//! Filesystem interaction.
//!
//! This module holds both the read-side inspection used by the (pure) planner —
//! [`inspect`], [`symlink_points_to`], [`same_inode`], [`content_equal`] — and,
//! from milestone M4, the write-side executor that applies planned operations.
//! OS-specific bits (inode comparison) are isolated here so Windows support can
//! slot in without touching the planner.

use std::path::{Component, Path, PathBuf};

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

/// True when both paths exist and share the same inode (a hardlink pair).
#[cfg(unix)]
pub fn same_inode(a: &Path, b: &Path) -> Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let (ma, mb) = match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => (ma, mb),
        _ => return Ok(false),
    };
    Ok(ma.dev() == mb.dev() && ma.ino() == mb.ino())
}

/// Inode comparison is not yet implemented off Unix; treat as "not linked" so the
/// planner re-establishes the hardlink. (Windows support is a future milestone.)
#[cfg(not(unix))]
pub fn same_inode(_a: &Path, _b: &Path) -> Result<bool> {
    Ok(false)
}

/// True when two paths have identical content (files: byte-equal; directories:
/// same tree), **ignoring** permission bits. Used by link-mode relink decisions,
/// where the live file is about to become a link and its mode is irrelevant.
pub fn content_equal(a: &Path, b: &Path) -> Result<bool> {
    equal(a, b, false)
}

/// Like [`content_equal`] but also requires identical permission bits at every
/// node. Used for `sync`-mode (copy) entries, where both sides are independent
/// real files and the mode is part of the file's identity.
pub fn synced_equal(a: &Path, b: &Path) -> Result<bool> {
    equal(a, b, true)
}

fn equal(a: &Path, b: &Path, include_mode: bool) -> Result<bool> {
    let (ma, mb) = (
        std::fs::metadata(a).map_err(|e| Error::io(a, e))?,
        std::fs::metadata(b).map_err(|e| Error::io(b, e))?,
    );
    if ma.is_dir() != mb.is_dir() {
        return Ok(false);
    }
    if ma.is_file() && mb.is_file() && ma.len() != mb.len() {
        return Ok(false); // fast path: different sizes can't be equal
    }
    Ok(digest(a, include_mode)? == digest(b, include_mode)?)
}

/// Content digest of a file or directory tree (following symlinks). Directory
/// digests fold in sorted entry names so they are order-independent. When
/// `include_mode` is set, each node's permission bits are folded in too.
fn digest(path: &Path, include_mode: bool) -> Result<blake3::Hash> {
    let md = std::fs::metadata(path).map_err(|e| Error::io(path, e))?;
    let mut hasher = blake3::Hasher::new();
    if include_mode {
        hasher.update(b"mode\0");
        hasher.update(&perm_bits(&md).to_le_bytes());
    }
    if md.is_dir() {
        hasher.update(b"dir\0");
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| Error::io(path, e))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for entry in entries {
            let name = entry.file_name().unwrap_or_default().to_string_lossy();
            hasher.update(name.as_bytes());
            hasher.update(b"\0");
            hasher.update(digest(&entry, include_mode)?.as_bytes());
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
        FsOp::Hardlink { link, target } => {
            mkparent(link)?;
            std::fs::hard_link(target, link).map_err(|e| Error::io(link, e))
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
        std::fs::copy(from, to)
            .map(|_| ())
            .map_err(|e| Error::io(to, e))
    }
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
fn normalize(p: &Path) -> PathBuf {
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
    fn synced_equal_is_permission_sensitive_but_content_equal_is_not() {
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
        assert!(synced_equal(&a, &b).unwrap());

        // Flip only the mode bits.
        std::fs::set_permissions(&b, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(content_equal(&a, &b).unwrap()); // content-only: still equal
        assert!(!synced_equal(&a, &b).unwrap()); // mode-sensitive: now differs
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

    #[test]
    fn same_inode_detects_hardlinks() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let a = base.join("a");
        let b = base.join("b");
        let c = base.join("c");
        std::fs::write(&a, b"x").unwrap();
        std::fs::hard_link(&a, &b).unwrap();
        std::fs::write(&c, b"x").unwrap();
        assert!(same_inode(&a, &b).unwrap());
        assert!(!same_inode(&a, &c).unwrap()); // same content, different inode
    }
}
