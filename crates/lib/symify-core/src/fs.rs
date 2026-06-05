//! Filesystem interaction.
//!
//! This module holds both the read-side inspection used by the (pure) planner —
//! [`inspect`], [`symlink_points_to`], [`content_equal`] — and the write-side
//! executor that applies planned operations. OS-specific bits (permission bits,
//! symlink creation) are isolated here so Windows support can slot in without
//! touching the planner.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::clock::Clock;
use crate::error::{Error, Result};
use crate::plan::FsOp;

/// What a path is, without following symlinks (`lstat` semantics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// The path does not exist.
    Missing,
    /// A symbolic link; carries the raw link target as stored on disk.
    Symlink(PathBuf),
    /// A regular file (or anything that isn't a dir/symlink).
    File,
    /// A directory.
    Dir,
}

impl NodeKind {
    /// True when the path does not exist.
    pub fn is_missing(&self) -> bool {
        matches!(self, NodeKind::Missing)
    }
}

/// True when `path` is a real directory (not a symlink to one) that contains at
/// least one entry. Used to flag unrecoverable recursive deletes for confirmation.
pub fn is_nonempty_dir(path: &Path) -> bool {
    match std::fs::symlink_metadata(path) {
        Ok(md) if md.is_dir() => std::fs::read_dir(path)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false),
        _ => false,
    }
}

/// Inspect a path without following symlinks.
pub fn inspect(path: &Path) -> Result<NodeKind> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NodeKind::Missing),
        Err(e) => Err(Error::io(path, e)),
        Ok(md) => {
            let ft = md.file_type();
            if ft.is_symlink() {
                let target = std::fs::read_link(path).map_err(|e| Error::io(path, e))?;
                Ok(NodeKind::Symlink(target))
            } else if ft.is_dir() {
                Ok(NodeKind::Dir)
            } else {
                Ok(NodeKind::File)
            }
        }
    }
}

/// True when `link` is a symlink whose (resolved) target is `target`.
///
/// symify writes absolute targets; we compare lexically-normalized absolute
/// paths so an equivalent spelling still counts as correct.
pub fn symlink_points_to(link: &Path, target: &Path) -> Result<bool> {
    match inspect(link)? {
        NodeKind::Symlink(raw) => {
            let abs = if raw.is_absolute() {
                raw
            } else {
                link.parent().unwrap_or(Path::new("")).join(raw)
            };
            Ok(normalize(&abs) == normalize(target))
        }
        _ => Ok(false),
    }
}

/// True when a directory-entry name is one of symify's own artifacts (`*.bak`
/// backups, `*.symify-tmp.*` in-progress copies). These are invisible to the
/// `sync` diff on both sides — never copied as a source add, never pruned as
/// extraneous — so backups don't churn and idempotency holds.
pub fn is_artifact(name: &OsStr) -> bool {
    let n = name.to_string_lossy();
    n.ends_with(".bak") || n.contains(".symify-tmp.")
}

/// Sorted set of a directory's entry names, with symify artifacts filtered out.
/// Shared by the identity checks and the planner's `sync` diff so both agree on
/// which entries exist.
pub fn dir_entries(path: &Path) -> Result<BTreeSet<OsString>> {
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(path).map_err(|e| Error::io(path, e))? {
        let name = entry.map_err(|e| Error::io(path, e))?.file_name();
        if !is_artifact(&name) {
            names.insert(name);
        }
    }
    Ok(names)
}

/// Fast structural equality for `sync`-mode entries: a stat-only walk comparing
/// each node's `(kind, len for files, perm bits, mtime within `modify_window`)`,
/// short-circuiting on the first difference. Directories are compared by their
/// (artifact-filtered) entry-name set, then recursively. Symlinks are compared
/// **as symlinks** (by target string), never followed. O(files) stat calls, not
/// O(bytes) — this is the headline win over the digest path.
pub fn quick_equal(a: &Path, b: &Path, modify_window: u64) -> Result<bool> {
    let (ma, mb) = match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(ma), Ok(mb)) => (ma, mb),
        _ => return Ok(false), // a side is missing or unreadable
    };
    let (ta, tb) = (ma.file_type(), mb.file_type());

    if ta.is_symlink() || tb.is_symlink() {
        // Compare links by target; mismatched kinds are unequal.
        if !(ta.is_symlink() && tb.is_symlink()) {
            return Ok(false);
        }
        return Ok(read_link_raw(a)? == read_link_raw(b)?);
    }
    if ta.is_dir() != tb.is_dir() {
        return Ok(false);
    }
    if ta.is_dir() {
        let (na, nb) = (dir_entries(a)?, dir_entries(b)?);
        if na != nb {
            return Ok(false);
        }
        for name in &na {
            if !quick_equal(&a.join(name), &b.join(name), modify_window)? {
                return Ok(false);
            }
        }
        Ok(true)
    } else {
        if ma.len() != mb.len() || perm_bits(&ma) != perm_bits(&mb) {
            return Ok(false);
        }
        Ok(mtime_within(&ma, &mb, modify_window))
    }
}

/// True when two regular files' modification times agree within `window` seconds.
/// `window == 0` requires an exact match (including sub-second precision).
fn mtime_within(a: &std::fs::Metadata, b: &std::fs::Metadata, window: u64) -> bool {
    let (ta, tb) = match (a.modified(), b.modified()) {
        (Ok(ta), Ok(tb)) => (ta, tb),
        _ => return false,
    };
    let diff = ta.duration_since(tb).unwrap_or_else(|e| e.duration());
    diff <= Duration::from_secs(window)
}

/// True when two paths have identical content (files: byte-equal; directories:
/// same tree), **ignoring** permission bits. Used by link-mode relink decisions,
/// where the live file is about to become a link and its mode is irrelevant.
pub fn content_equal(a: &Path, b: &Path) -> Result<bool> {
    equal(a, b, false)
}

/// Exact content compare for `sync`-mode entries (the `--checksum` path): like
/// [`content_equal`] but also requires identical permission bits at every node.
/// Both sides are independent real files and the mode is part of their identity.
pub fn checksum_equal(a: &Path, b: &Path) -> Result<bool> {
    equal(a, b, true)
}

fn equal(a: &Path, b: &Path, include_mode: bool) -> Result<bool> {
    let (ma, mb) = (
        std::fs::symlink_metadata(a).map_err(|e| Error::io(a, e))?,
        std::fs::symlink_metadata(b).map_err(|e| Error::io(b, e))?,
    );
    let (ta, tb) = (ma.file_type(), mb.file_type());
    if ta.is_symlink() || tb.is_symlink() {
        if !(ta.is_symlink() && tb.is_symlink()) {
            return Ok(false);
        }
        return Ok(read_link_raw(a)? == read_link_raw(b)?);
    }
    if ta.is_dir() != tb.is_dir() {
        return Ok(false);
    }
    if !ta.is_dir() && ma.len() != mb.len() {
        return Ok(false); // fast path: different sizes can't be equal
    }
    Ok(digest(a, include_mode)? == digest(b, include_mode)?)
}

/// Read a symlink's raw target, mapping IO errors.
fn read_link_raw(path: &Path) -> Result<PathBuf> {
    std::fs::read_link(path).map_err(|e| Error::io(path, e))
}

/// Content digest of a file or directory tree, treating symlinks **as symlinks**
/// (the target string is hashed; the link is never followed). Directory digests
/// fold in artifact-filtered, sorted entry names so they are order-independent.
/// When `include_mode` is set, each node's permission bits are folded in too.
fn digest(path: &Path, include_mode: bool) -> Result<blake3::Hash> {
    let md = std::fs::symlink_metadata(path).map_err(|e| Error::io(path, e))?;
    let ft = md.file_type();
    let mut hasher = blake3::Hasher::new();
    if include_mode {
        hasher.update(b"mode\0");
        hasher.update(&perm_bits(&md).to_le_bytes());
    }
    if ft.is_symlink() {
        hasher.update(b"link\0");
        hasher.update(read_link_raw(path)?.as_os_str().as_encoded_bytes());
    } else if ft.is_dir() {
        hasher.update(b"dir\0");
        for name in dir_entries(path)? {
            hasher.update(name.as_encoded_bytes());
            hasher.update(b"\0");
            hasher.update(digest(&path.join(&name), include_mode)?.as_bytes());
        }
    } else {
        hasher.update(b"file\0");
        let bytes = std::fs::read(path).map_err(|e| Error::io(path, e))?;
        hasher.update(&bytes);
    }
    Ok(hasher.finalize())
}

/// Permission bits used for `sync`-mode equality. On Unix these are the mode's
/// permission/setuid/sticky bits; elsewhere we fall back to the read-only flag.
#[cfg(unix)]
fn perm_bits(md: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    md.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn perm_bits(md: &std::fs::Metadata) -> u32 {
    md.permissions().readonly() as u32
}

// ----- write side (executor) --------------------------------------------

/// Apply a single primitive operation. Parent directories of any written path
/// are created automatically; directories are never removed implicitly.
pub fn apply_op(op: &FsOp, clock: &dyn Clock) -> Result<()> {
    match op {
        FsOp::Backup(path) => backup(path, clock),
        FsOp::Remove(path) => remove(path),
        FsOp::Move { from, to } => move_path(from, to),
        FsOp::Symlink { link, target } => {
            mkparent(link)?;
            make_symlink(target, link)
        }
        FsOp::Copy { from, to } => {
            mkparent(to)?;
            copy_tree(from, to)
        }
    }
}

/// Rename `path` to `<name>.<timestamp>.bak` beside it.
fn backup(path: &Path, clock: &dyn Clock) -> Result<()> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dest = path.with_file_name(format!("{name}.{}.bak", clock.timestamp()));
    std::fs::rename(path, &dest).map_err(|e| Error::io(path, e))
}

/// Remove a file, symlink, or directory (recursively). A missing path is a
/// no-op.
fn remove(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::io(path, e)),
        Ok(md) if md.file_type().is_dir() => {
            std::fs::remove_dir_all(path).map_err(|e| Error::io(path, e))
        }
        Ok(_) => std::fs::remove_file(path).map_err(|e| Error::io(path, e)),
    }
}

/// Rename `from` to `to`, falling back to copy + remove across filesystems.
fn move_path(from: &Path, to: &Path) -> Result<()> {
    mkparent(to)?;
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Likely a cross-device rename; copy then remove the original.
            copy_tree(from, to)?;
            remove(from)
        }
    }
}

/// Recursively copy a file, symlink, or directory tree to `to`.
fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    let md = std::fs::symlink_metadata(from).map_err(|e| Error::io(from, e))?;
    let ft = md.file_type();
    if ft.is_dir() {
        std::fs::create_dir_all(to).map_err(|e| Error::io(to, e))?;
        for entry in std::fs::read_dir(from).map_err(|e| Error::io(from, e))? {
            let entry = entry.map_err(|e| Error::io(from, e))?;
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        }
        // Preserve the source directory's permission bits (set last, after
        // children are written, so a read-only source dir doesn't block the copy).
        std::fs::set_permissions(to, md.permissions()).map_err(|e| Error::io(to, e))?;
        Ok(())
    } else if ft.is_symlink() {
        let target = std::fs::read_link(from).map_err(|e| Error::io(from, e))?;
        make_symlink(&target, to)
    } else {
        copy_file_atomic(from, to)
    }
}

/// Counter for unique temp names, so concurrent copies in the same directory
/// never collide. Combined with the pid it is unique across processes too.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A temp path beside `to`, in the same directory so the final `rename` is a
/// same-filesystem (atomic) operation. Recognised by [`is_artifact`].
fn temp_path(to: &Path) -> PathBuf {
    let n = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let name = to
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    to.with_file_name(format!(".{name}.symify-tmp.{pid}.{n}"))
}

/// Copy a regular file atomically and preserving its mtime: write a temp beside
/// the destination, copy permission bits, set the modification time to match the
/// source, then `rename` over the destination. Readers and crashes never observe
/// a half-written file; the preserved mtime keeps the size+mtime quick-check
/// stable across runs. A leftover temp is best-effort removed on error.
fn copy_file_atomic(from: &Path, to: &Path) -> Result<()> {
    let tmp = temp_path(to);
    let result = (|| {
        std::fs::copy(from, &tmp).map_err(|e| Error::io(&tmp, e))?;
        let src = std::fs::metadata(from).map_err(|e| Error::io(from, e))?;
        std::fs::set_permissions(&tmp, src.permissions()).map_err(|e| Error::io(&tmp, e))?;
        let mtime = src.modified().map_err(|e| Error::io(from, e))?;
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(&tmp)
            .map_err(|e| Error::io(&tmp, e))?;
        f.set_times(std::fs::FileTimes::new().set_modified(mtime))
            .map_err(|e| Error::io(&tmp, e))?;
        std::fs::rename(&tmp, to).map_err(|e| Error::io(to, e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Create the parent directory of `path` if needed.
fn mkparent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    Ok(())
}

/// Create a symlink at `link` pointing to `target` (platform-aware).
#[cfg(unix)]
fn make_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link).map_err(|e| Error::io(link, e))
}

#[cfg(windows)]
fn make_symlink(target: &Path, link: &Path) -> Result<()> {
    // Choose the right Windows symlink kind based on the target.
    let is_dir = std::fs::metadata(target)
        .map(|m| m.is_dir())
        .unwrap_or(false);
    let res = if is_dir {
        std::os::windows::fs::symlink_dir(target, link)
    } else {
        std::os::windows::fs::symlink_file(target, link)
    };
    res.map_err(|e| Error::io(link, e))
}

/// Lexically normalize a path (resolve `.` and `..` without touching the
/// filesystem).
pub(crate) fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspect_distinguishes_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let file = base.join("f");
        std::fs::write(&file, b"hi").unwrap();
        let subdir = base.join("d");
        std::fs::create_dir(&subdir).unwrap();
        let link = base.join("l");
        std::os::unix::fs::symlink(&file, &link).unwrap();

        assert_eq!(inspect(&base.join("nope")).unwrap(), NodeKind::Missing);
        assert_eq!(inspect(&file).unwrap(), NodeKind::File);
        assert_eq!(inspect(&subdir).unwrap(), NodeKind::Dir);
        assert_eq!(inspect(&link).unwrap(), NodeKind::Symlink(file));
    }

    #[test]
    fn symlink_points_to_compares_resolved_target() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let target = base.join("real");
        std::fs::write(&target, b"x").unwrap();
        let link = base.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(symlink_points_to(&link, &target).unwrap());
        assert!(!symlink_points_to(&link, &base.join("other")).unwrap());
        assert!(!symlink_points_to(&target, &target).unwrap()); // not a symlink
    }

    #[test]
    fn content_equal_for_files_and_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        assert!(content_equal(&a, &b).unwrap());
        std::fs::write(&b, b"different").unwrap();
        assert!(!content_equal(&a, &b).unwrap());

        // directory trees
        let da = base.join("da");
        let db = base.join("db");
        std::fs::create_dir_all(da.join("sub")).unwrap();
        std::fs::create_dir_all(db.join("sub")).unwrap();
        std::fs::write(da.join("sub/x"), b"1").unwrap();
        std::fs::write(db.join("sub/x"), b"1").unwrap();
        assert!(content_equal(&da, &db).unwrap());
        std::fs::write(db.join("sub/x"), b"2").unwrap();
        assert!(!content_equal(&da, &db).unwrap());
    }

    #[test]
    fn checksum_equal_is_permission_sensitive_but_content_equal_is_not() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        std::fs::set_permissions(&a, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(content_equal(&a, &b).unwrap());
        assert!(checksum_equal(&a, &b).unwrap());

        // Flip only the mode bits.
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(content_equal(&a, &b).unwrap()); // content-only: still equal
        assert!(!checksum_equal(&a, &b).unwrap()); // mode-sensitive: now differs
    }

    #[test]
    fn copy_tree_preserves_directory_permissions() {
        use crate::clock::SystemClock;
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let src = base.join("src");
        std::fs::create_dir(&src).unwrap();
        std::fs::write(src.join("f"), b"x").unwrap();
        std::fs::set_permissions(&src, std::fs::Permissions::from_mode(0o700)).unwrap();

        let dst = base.join("dst");
        apply_op(
            &FsOp::Copy {
                from: src.clone(),
                to: dst.clone(),
            },
            &SystemClock,
        )
        .unwrap();

        let mode = std::fs::metadata(&dst).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }

    fn match_mtime(src: &Path, dst: &Path) {
        let t = std::fs::metadata(src).unwrap().modified().unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(dst).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(t))
            .unwrap();
    }

    #[test]
    fn quick_equal_detects_size_mode_and_mtime_drift() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::write(&a, b"hello").unwrap();
        std::fs::write(&b, b"hello").unwrap();
        match_mtime(&a, &b);
        assert!(quick_equal(&a, &b, 0).unwrap()); // same size, mode, mtime

        // Size drift.
        std::fs::write(&b, b"hello world").unwrap();
        match_mtime(&a, &b);
        assert!(!quick_equal(&a, &b, 0).unwrap());

        // Mode drift only.
        std::fs::write(&b, b"hello").unwrap();
        match_mtime(&a, &b);
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&a, std::fs::Permissions::from_mode(0o644)).unwrap();
        match_mtime(&a, &b); // chmod can bump mtime; realign
        assert!(!quick_equal(&a, &b, 0).unwrap());
    }

    #[test]
    fn quick_equal_honours_modify_window() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::write(&a, b"data").unwrap();
        std::fs::write(&b, b"data").unwrap();
        let t = std::fs::metadata(&a).unwrap().modified().unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(&b).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(t + Duration::from_secs(1)))
            .unwrap();
        assert!(!quick_equal(&a, &b, 0).unwrap()); // 1s skew, exact
        assert!(quick_equal(&a, &b, 1).unwrap()); // within window
    }

    #[test]
    fn symlinks_compared_as_links_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let la = base.join("la");
        let lb = base.join("lb");
        let lc = base.join("lc");
        std::os::unix::fs::symlink("target/x", &la).unwrap();
        std::os::unix::fs::symlink("target/x", &lb).unwrap(); // same (dangling) target
        std::os::unix::fs::symlink("target/y", &lc).unwrap(); // different target

        // Equal by target string, even though the targets don't exist.
        assert!(quick_equal(&la, &lb, 0).unwrap());
        assert!(content_equal(&la, &lb).unwrap());
        assert!(checksum_equal(&la, &lb).unwrap());
        // Different target → unequal, and no error from the dangling link.
        assert!(!quick_equal(&la, &lc, 0).unwrap());
        assert!(!content_equal(&la, &lc).unwrap());
    }

    #[test]
    fn artifacts_are_invisible_to_dir_compare() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        std::fs::write(a.join("f"), b"x").unwrap();
        std::fs::write(b.join("f"), b"x").unwrap();
        match_mtime(&a.join("f"), &b.join("f"));
        // An extra `.bak` / temp on one side does not make the trees differ.
        std::fs::write(b.join("f.20260101.bak"), b"backup").unwrap();
        std::fs::write(a.join(".f.symify-tmp.1.2"), b"partial").unwrap();
        assert!(quick_equal(&a, &b, 0).unwrap());
        assert!(content_equal(&a, &b).unwrap());
    }

    #[test]
    fn atomic_copy_preserves_content_and_mtime() {
        use crate::clock::SystemClock;
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let from = base.join("from");
        let to = base.join("to");
        std::fs::write(&from, b"payload").unwrap();
        let want = std::fs::metadata(&from).unwrap().modified().unwrap();

        apply_op(
            &FsOp::Copy {
                from: from.clone(),
                to: to.clone(),
            },
            &SystemClock,
        )
        .unwrap();

        assert_eq!(std::fs::read(&to).unwrap(), b"payload");
        assert_eq!(std::fs::metadata(&to).unwrap().modified().unwrap(), want);
        // No leftover temp file beside the destination.
        let leftovers: Vec<_> = std::fs::read_dir(base)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| is_artifact(&e.file_name()))
            .collect();
        assert!(leftovers.is_empty(), "temp left behind: {leftovers:?}");
    }
}
