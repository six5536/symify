//! The pure planner.
//!
//! [`plan`] is a function of *(resolved config + current filesystem state)*. It
//! reads the filesystem but never mutates it, emitting an ordered list of
//! [`Planned`] entries. Each entry's [`Action`] either records a no-op/blocked
//! state or carries the exact ordered [`FsOp`]s the executor will run. The
//! per-entry state machine is documented in `knowledge/architecture.md`.

use std::path::{Path, PathBuf};

use crate::Result;
use crate::clock::Clock;
use crate::config::{ResolvedConfig, ResolvedMapping};
use crate::fs::{self, NodeKind};
use crate::model::{Conflict, LinkKind, LinkValue, Mode};

/// Direction of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// `live` → `store` (capture / adopt).
    Sync,
    /// `store` → `live` (install).
    Deploy,
}

/// Per-run options that influence planning but are not config: the `--checksum`
/// and `--modify-window` CLI flags. Shared by [`plan()`] and
/// [`status()`](crate::status::status) so a status report matches what a run
/// would decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RunOptions {
    /// Force an exact content compare instead of the size+mtime quick-check.
    pub checksum: bool,
    /// mtime tolerance in seconds for the quick-check (`0` = exact).
    pub modify_window: u64,
}

impl RunOptions {
    /// Compare two `sync`-mode paths for equality under these options: the exact
    /// [`fs::checksum_equal`] when `--checksum` is set, else the fast
    /// [`fs::quick_equal`] with the configured modify-window.
    pub(crate) fn equal(&self, a: &Path, b: &Path) -> Result<bool> {
        if self.checksum {
            fs::checksum_equal(a, b)
        } else {
            fs::quick_equal(a, b, self.modify_window)
        }
    }
}

/// A primitive filesystem operation, applied in order by the executor. The
/// executor creates parent directories of any written path automatically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsOp {
    /// Rename the path to `<name>.<timestamp>.bak`.
    Backup(PathBuf),
    /// Delete the path (file or directory).
    Remove(PathBuf),
    /// Rename `from` to `to` (used to move a live file into the store).
    Move {
        /// Source path.
        from: PathBuf,
        /// Destination path.
        to: PathBuf,
    },
    /// Create a symlink at `link` pointing to `target`.
    Symlink {
        /// The link location.
        link: PathBuf,
        /// The link target.
        target: PathBuf,
    },
    /// Copy `from` to `to` (recursive for directories).
    Copy {
        /// Source path.
        from: PathBuf,
        /// Destination path.
        to: PathBuf,
    },
}

/// A short semantic label for an applied action, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    /// `sync` link modes: capture a live file into the store, then link.
    Adopt,
    /// Convert an already-matching live file into a link (no backup needed).
    Relink,
    /// `deploy` link modes: create a link in the live location.
    Link,
    /// `sync` copy mode: copy live → store.
    Push,
    /// `deploy` copy mode: copy store → live.
    Pull,
}

/// The decided outcome for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Already in the desired state — nothing to do.
    AlreadyOk,
    /// Entry disabled (`value = false`).
    Disabled,
    /// Nothing to do for this verb and state, with a human-readable reason.
    Skip(&'static str),
    /// A real difference left unresolved because `conflict = skip`.
    Conflict,
    /// The planner determined the entry cannot be applied.
    Failed(String),
    /// Apply the ordered operations.
    Apply {
        /// Semantic label for reporting.
        kind: ActionKind,
        /// Operations in execution order.
        ops: Vec<FsOp>,
    },
    /// Apply the ordered operations, but a same-path difference was left
    /// unresolved by `conflict = skip`. The entry both changes files (the pure
    /// adds and any resolved conflicts) **and** reports drift, so a second run is
    /// not all-clean until the skipped difference is resolved.
    ApplyDrift {
        /// Semantic label for reporting.
        kind: ActionKind,
        /// Operations in execution order.
        ops: Vec<FsOp>,
    },
}

/// One planned entry: the resolved paths plus the decided [`Action`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    /// Mapping name.
    pub mapping: String,
    /// The entry key as written in config.
    pub key: String,
    /// Absolute live-side path (`S`).
    pub live: PathBuf,
    /// Absolute store-side path (`D`).
    pub store: PathBuf,
    /// Effective mode for this entry.
    pub mode: Mode,
    /// Effective conflict policy for this entry.
    pub conflict: Conflict,
    /// What the planner decided.
    pub action: Action,
}

/// Plan a run over the whole resolved config.
///
/// ```no_run
/// use symify_core::{config, plan, RunOptions, Verb};
/// let resolved = config::load_config(&[])?;
/// let planned = plan::plan(&resolved, Verb::Deploy, RunOptions::default())?;
/// # Ok::<(), symify_core::Error>(())
/// ```
pub fn plan(config: &ResolvedConfig, verb: Verb, opts: RunOptions) -> Result<Vec<Planned>> {
    let mut out = Vec::new();
    for mapping in &config.mappings {
        for (key, value) in &mapping.links {
            out.push(plan_entry(mapping, key, value, verb, opts)?);
        }
    }
    Ok(out)
}

/// Outcome of executing one planned entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The action was applied (or, under `--dry-run`, would be applied).
    Applied(ActionKind),
    /// Applied (or would apply) changes, but a residual `skip`-conflict drift
    /// remains — the entry is not fully in sync.
    AppliedDrift(ActionKind),
    /// Already in the desired state.
    AlreadyOk,
    /// Entry disabled.
    Disabled,
    /// Nothing to do, with a reason.
    Skipped(&'static str),
    /// Unresolved conflict (`conflict = skip`).
    Conflict,
    /// The entry could not be applied.
    Failed(String),
}

impl Outcome {
    /// True when the entry represents drift (a conflict, or a change applied with
    /// a residual `skip`-difference still unresolved).
    pub fn is_drift(&self) -> bool {
        matches!(self, Outcome::Conflict | Outcome::AppliedDrift(_))
    }

    /// True when the entry failed.
    pub fn is_failure(&self) -> bool {
        matches!(self, Outcome::Failed(_))
    }
}

/// Execute planned entries in order, continuing past failures (each entry is
/// independent — there is no rollback). Under `dry_run` nothing is mutated and
/// `Apply` entries report `Applied` as if they had run.
///
/// ```no_run
/// use symify_core::{config, plan, RunOptions, Verb};
/// use symify_core::clock::SystemClock;
/// let resolved = config::load_config(&[])?;
/// let planned = plan::plan(&resolved, Verb::Sync, RunOptions::default())?;
/// let outcomes = plan::execute(&planned, &SystemClock, false);
/// # Ok::<(), symify_core::Error>(())
/// ```
pub fn execute(planned: &[Planned], clock: &dyn Clock, dry_run: bool) -> Vec<Outcome> {
    planned
        .iter()
        .map(|p| execute_one(p, clock, dry_run))
        .collect()
}

fn execute_one(p: &Planned, clock: &dyn Clock, dry_run: bool) -> Outcome {
    match &p.action {
        Action::AlreadyOk => Outcome::AlreadyOk,
        Action::Disabled => Outcome::Disabled,
        Action::Skip(reason) => Outcome::Skipped(reason),
        Action::Conflict => Outcome::Conflict,
        Action::Failed(msg) => Outcome::Failed(msg.clone()),
        Action::Apply { kind, ops } => run_ops(ops, Outcome::Applied(*kind), clock, dry_run),
        Action::ApplyDrift { kind, ops } => {
            run_ops(ops, Outcome::AppliedDrift(*kind), clock, dry_run)
        }
    }
}

/// Run an action's ops (unless `dry_run`), returning `applied` on success.
fn run_ops(ops: &[FsOp], applied: Outcome, clock: &dyn Clock, dry_run: bool) -> Outcome {
    if dry_run {
        return applied;
    }
    for op in ops {
        if let Err(e) = fs::apply_op(op, clock) {
            return Outcome::Failed(e.to_string());
        }
    }
    applied
}

fn plan_entry(
    m: &ResolvedMapping,
    key: &str,
    value: &LinkValue,
    verb: Verb,
    opts: RunOptions,
) -> Result<Planned> {
    let kind = value.kind();
    let make = |live: PathBuf, store: PathBuf, action: Action| Planned {
        mapping: m.name.clone(),
        key: key.to_string(),
        live,
        store,
        mode: m.mode,
        conflict: m.conflict,
        action,
    };

    if let LinkKind::Disabled = kind {
        let (s, d) = resolve_paths(m, key, LinkKind::Mirror); // paths still meaningful for display
        return Ok(make(s, d, Action::Disabled));
    }

    let (s, d) = resolve_paths(m, key, kind);

    // Safety guards: refuse entries that could swallow a root or operate on a
    // directory outside the live root. See `knowledge/architectural-rules.md`.
    if let Some(reason) = guard_reason(m, &s, &d)? {
        return Ok(make(s, d, Action::Failed(reason)));
    }

    let action = match verb {
        Verb::Sync => plan_sync(&s, &d, m, opts)?,
        Verb::Deploy => plan_deploy(&s, &d, m, opts)?,
    };
    Ok(make(s, d, action))
}

/// Safety check shared by [`plan`] and `status`. Returns a refusal reason when an
/// entry's resolved `(live, store)` is dangerous, or `None` when it is safe:
///
/// - **A (sentinels):** live or store resolves to a protected root — `/`,
///   `$HOME`, or the mapping's own `live`/`store` root.
/// - **store-containment:** live equals or contains the store root (adopting it
///   would pull the store into itself).
/// - **B (out-of-root ⇒ file-only):** anything resolving outside the live root
///   must be a single file, not a directory, on either side (blocks adopting or
///   deploying whole trees such as `/etc`).
pub(crate) fn guard_reason(m: &ResolvedMapping, s: &Path, d: &Path) -> Result<Option<String>> {
    let ns = fs::normalize(s);
    let nd = fs::normalize(d);
    let nlive = fs::normalize(&m.live);
    let nstore = fs::normalize(&m.store);

    let mut sentinels = vec![nlive.clone(), nstore.clone(), PathBuf::from("/")];
    if let Ok(home) = crate::config::home_dir() {
        sentinels.push(fs::normalize(&home));
    }
    for (side, p) in [("live", &ns), ("store", &nd)] {
        if sentinels.iter().any(|sent| sent == p) {
            return Ok(Some(format!(
                "refusing to operate on protected root: {side} path resolves to {}",
                p.display()
            )));
        }
    }

    if nstore.starts_with(&ns) {
        return Ok(Some(format!(
            "refusing: live path {} contains the store root {}",
            ns.display(),
            nstore.display()
        )));
    }

    if !ns.starts_with(&nlive)
        && (fs::inspect(s)? == NodeKind::Dir || fs::inspect(d)? == NodeKind::Dir)
    {
        return Ok(Some(format!(
            "refusing: {} is outside the live root, so it must be a file, not a directory",
            s.display()
        )));
    }
    Ok(None)
}

/// Resolve an entry's absolute `(live, store)` paths from its key and value.
/// Public wrapper over the internal `resolve_paths` for callers like `list`.
pub fn entry_paths(m: &ResolvedMapping, key: &str, value: &LinkValue) -> (PathBuf, PathBuf) {
    resolve_paths(m, key, value.kind())
}

/// Resolve an entry's `(live, store)` absolute paths from its key and value kind.
pub(crate) fn resolve_paths(m: &ResolvedMapping, key: &str, kind: LinkKind) -> (PathBuf, PathBuf) {
    let key_path = Path::new(key);
    let live = if key_path.is_absolute() {
        key_path.to_path_buf()
    } else {
        m.live.join(key)
    };

    let store = match kind {
        LinkKind::Mirror | LinkKind::Disabled => {
            if key_path.is_absolute() {
                m.store.join(key.trim_start_matches('/'))
            } else {
                m.store.join(key)
            }
        }
        LinkKind::Explicit(p) => {
            let pp = Path::new(p);
            if pp.is_absolute() {
                pp.to_path_buf()
            } else {
                m.store.join(p)
            }
        }
    };

    (live, store)
}

fn link_op(s: &Path, d: &Path) -> FsOp {
    FsOp::Symlink {
        link: s.to_path_buf(),
        target: d.to_path_buf(),
    }
}

// ----- sync (live -> store) ---------------------------------------------

fn plan_sync(s: &Path, d: &Path, m: &ResolvedMapping, opts: RunOptions) -> Result<Action> {
    let s_state = fs::inspect(s)?;
    match m.mode {
        Mode::Symlink => plan_sync_link(s, d, m.conflict, s_state),
        Mode::Sync => plan_sync_copy(s, d, m.conflict, s_state, opts),
    }
}

fn plan_sync_link(s: &Path, d: &Path, conflict: Conflict, s_state: NodeKind) -> Result<Action> {
    match s_state {
        NodeKind::Missing => Ok(Action::Skip("live path missing — nothing to capture")),
        NodeKind::Symlink(_) => {
            // A link carries no independent content to capture.
            if fs::symlink_points_to(s, d)? && !fs::inspect(d)?.is_missing() {
                Ok(Action::AlreadyOk)
            } else {
                Ok(Action::Skip(
                    "live path is a symlink — no content to capture",
                ))
            }
        }
        NodeKind::File | NodeKind::Dir => match fs::inspect(d)? {
            NodeKind::Missing => Ok(Action::Apply {
                kind: ActionKind::Adopt,
                ops: vec![mv(s, d), link_op(s, d)],
            }),
            _ if fs::content_equal(s, d)? => Ok(Action::Apply {
                kind: ActionKind::Relink,
                ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d)],
            }),
            _ => match conflict {
                Conflict::Skip => Ok(Action::Conflict),
                Conflict::Backup => Ok(Action::Apply {
                    kind: ActionKind::Adopt,
                    ops: vec![FsOp::Backup(d.to_path_buf()), mv(s, d), link_op(s, d)],
                }),
                Conflict::Replace => Ok(Action::Apply {
                    kind: ActionKind::Adopt,
                    ops: vec![FsOp::Remove(d.to_path_buf()), mv(s, d), link_op(s, d)],
                }),
            },
        },
    }
}

fn plan_sync_copy(
    s: &Path,
    d: &Path,
    conflict: Conflict,
    s_state: NodeKind,
    opts: RunOptions,
) -> Result<Action> {
    if s_state.is_missing() {
        return Ok(Action::Skip("live path missing — nothing to capture"));
    }
    // sync copies live (s) → store (d), adding and updating (never pruning).
    diff_copy(s, d, conflict, opts, ActionKind::Push)
}

// ----- deploy (store -> live) -------------------------------------------

fn plan_deploy(s: &Path, d: &Path, m: &ResolvedMapping, opts: RunOptions) -> Result<Action> {
    if fs::inspect(d)?.is_missing() {
        return Ok(Action::Skip("store path missing — nothing to deploy"));
    }
    match m.mode {
        Mode::Symlink => plan_deploy_link(s, d, m.conflict),
        Mode::Sync => plan_deploy_copy(s, d, m.conflict, opts),
    }
}

fn plan_deploy_link(s: &Path, d: &Path, conflict: Conflict) -> Result<Action> {
    // Already in desired state?
    if fs::symlink_points_to(s, d)? {
        return Ok(Action::AlreadyOk);
    }

    let s_state = fs::inspect(s)?;
    if s_state.is_missing() {
        return Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![link_op(s, d)],
        });
    }

    // S exists but is wrong. If it's a real file/dir already matching D, relink
    // without a backup; otherwise apply the conflict policy.
    let can_relink = matches!(s_state, NodeKind::File | NodeKind::Dir) && fs::content_equal(s, d)?;
    if can_relink {
        return Ok(Action::Apply {
            kind: ActionKind::Relink,
            ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d)],
        });
    }
    match conflict {
        Conflict::Skip => Ok(Action::Conflict),
        Conflict::Backup => Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![FsOp::Backup(s.to_path_buf()), link_op(s, d)],
        }),
        Conflict::Replace => Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d)],
        }),
    }
}

fn plan_deploy_copy(s: &Path, d: &Path, conflict: Conflict, opts: RunOptions) -> Result<Action> {
    // deploy copies store (d) → live (s), adding and updating (never pruning).
    diff_copy(d, s, conflict, opts, ActionKind::Pull)
}

/// Diff a `sync`-mode entry's `src` against `dst` and decide its [`Action`]:
/// walk per-file, emitting `Copy`/`Backup`/`Remove` ops only where they differ
/// (additive — destination-only entries are left untouched). Aggregates an
/// unresolved `skip`-difference into the drift-bearing outcome.
fn diff_copy(
    src: &Path,
    dst: &Path,
    conflict: Conflict,
    opts: RunOptions,
    kind: ActionKind,
) -> Result<Action> {
    let mut ops = Vec::new();
    let mut drift = false;
    walk_copy(src, dst, conflict, opts, &mut ops, &mut drift)?;
    Ok(if ops.is_empty() {
        if drift {
            Action::Conflict
        } else {
            Action::AlreadyOk
        }
    } else if drift {
        Action::ApplyDrift { kind, ops }
    } else {
        Action::Apply { kind, ops }
    })
}

/// Recursive worker for [`diff_copy`]. `src` is known to exist. Emits copy/backup
/// ops for changed source entries; additive, so destination-only entries are
/// left untouched. Sets `drift` when a `conflict = skip` difference is left
/// unresolved.
fn walk_copy(
    src: &Path,
    dst: &Path,
    conflict: Conflict,
    opts: RunOptions,
    ops: &mut Vec<FsOp>,
    drift: &mut bool,
) -> Result<()> {
    if fs::inspect(dst)?.is_missing() {
        // Nothing on the destination — copy the whole (sub)tree or file.
        ops.push(cp(src, dst));
        return Ok(());
    }

    let both_dirs = fs::inspect(src)? == NodeKind::Dir && fs::inspect(dst)? == NodeKind::Dir;
    if both_dirs {
        // Source entries: add or update (recursing — additive, never prunes).
        for name in &fs::dir_entries(src)? {
            walk_copy(&src.join(name), &dst.join(name), conflict, opts, ops, drift)?;
        }
        return Ok(());
    }

    // At least one side is a file/symlink (or the kinds differ). Compare; if they
    // already match there is nothing to do, otherwise apply the conflict policy.
    if opts.equal(src, dst)? {
        return Ok(());
    }
    match conflict {
        Conflict::Skip => *drift = true,
        Conflict::Backup => {
            ops.push(FsOp::Backup(dst.to_path_buf()));
            ops.push(cp(src, dst));
        }
        Conflict::Replace => {
            ops.push(FsOp::Remove(dst.to_path_buf()));
            ops.push(cp(src, dst));
        }
    }
    Ok(())
}

fn mv(from: &Path, to: &Path) -> FsOp {
    FsOp::Move {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
    }
}

fn cp(from: &Path, to: &Path) -> FsOp {
    FsOp::Copy {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ResolvedConfig, ResolvedMapping};
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Test fixture: a tempdir with `live/` and `store/` roots and helpers to
    /// lay down files, dirs, and symlinks, then build a one-mapping config.
    struct Fx {
        _tmp: TempDir,
        live: PathBuf,
        store: PathBuf,
    }

    impl Fx {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let live = tmp.path().join("live");
            let store = tmp.path().join("store");
            std::fs::create_dir_all(&live).unwrap();
            std::fs::create_dir_all(&store).unwrap();
            Fx {
                _tmp: tmp,
                live,
                store,
            }
        }
        fn lp(&self, rel: &str) -> PathBuf {
            self.live.join(rel)
        }
        fn sp(&self, rel: &str) -> PathBuf {
            self.store.join(rel)
        }
        fn write(&self, p: &Path, c: &[u8]) {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, c).unwrap();
        }
        fn cfg(
            &self,
            mode: Mode,
            conflict: Conflict,
            links: Vec<(&str, LinkValue)>,
        ) -> ResolvedConfig {
            ResolvedConfig {
                mappings: vec![ResolvedMapping {
                    name: "m".into(),
                    live: self.live.clone(),
                    store: self.store.clone(),
                    mode,
                    conflict,
                    links: links.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                }],
            }
        }
    }

    fn t() -> LinkValue {
        LinkValue::Boolean(true)
    }
    fn off() -> LinkValue {
        LinkValue::Boolean(false)
    }

    fn act(cfg: &ResolvedConfig, verb: Verb) -> Action {
        let mut planned = plan(cfg, verb, RunOptions::default()).unwrap();
        assert_eq!(planned.len(), 1, "expected exactly one entry");
        planned.remove(0).action
    }

    /// Like [`act`] but with explicit run options (checksum / modify-window).
    fn act_opts(cfg: &ResolvedConfig, verb: Verb, opts: RunOptions) -> Action {
        let mut planned = plan(cfg, verb, opts).unwrap();
        assert_eq!(planned.len(), 1, "expected exactly one entry");
        planned.remove(0).action
    }

    // ---- link resolution ----

    #[test]
    fn resolves_mirror_relative_and_absolute_and_explicit() {
        let fx = Fx::new();
        // relative mirror
        let p = plan(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![(".bashrc", t())]),
            Verb::Deploy,
            RunOptions::default(),
        )
        .unwrap();
        assert_eq!(p[0].live, fx.lp(".bashrc"));
        assert_eq!(p[0].store, fx.sp(".bashrc"));
        // absolute key mirror -> store strips leading /
        let p = plan(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("/etc/x.conf", t())]),
            Verb::Deploy,
            RunOptions::default(),
        )
        .unwrap();
        assert_eq!(p[0].live, PathBuf::from("/etc/x.conf"));
        assert_eq!(p[0].store, fx.sp("etc/x.conf"));
        // explicit relative value -> under store
        let p = plan(
            &fx.cfg(
                Mode::Symlink,
                Conflict::Backup,
                vec![("profile", LinkValue::String("fixed/p".into()))],
            ),
            Verb::Deploy,
            RunOptions::default(),
        )
        .unwrap();
        assert_eq!(p[0].store, fx.sp("fixed/p"));
    }

    #[test]
    fn disabled_entry_is_disabled() {
        let fx = Fx::new();
        assert_eq!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", off())]),
                Verb::Sync
            ),
            Action::Disabled
        );
    }

    // ---- sync, link modes ----

    #[test]
    fn sync_skips_when_live_missing() {
        let fx = Fx::new();
        assert!(matches!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
                Verb::Sync
            ),
            Action::Skip(_)
        ));
    }

    #[test]
    fn sync_adopts_when_store_missing() {
        let fx = Fx::new();
        fx.write(&fx.lp(".bashrc"), b"hi");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![(".bashrc", t())]),
            Verb::Sync,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Adopt,
                ops: vec![
                    FsOp::Move {
                        from: fx.lp(".bashrc"),
                        to: fx.sp(".bashrc")
                    },
                    FsOp::Symlink {
                        link: fx.lp(".bashrc"),
                        target: fx.sp(".bashrc")
                    },
                ],
            }
        );
    }

    #[test]
    fn sync_conflict_backup_backs_up_store() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"live");
        fx.write(&fx.sp("x"), b"store");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
            Verb::Sync,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Adopt,
                ops: vec![
                    FsOp::Backup(fx.sp("x")),
                    FsOp::Move {
                        from: fx.lp("x"),
                        to: fx.sp("x")
                    },
                    FsOp::Symlink {
                        link: fx.lp("x"),
                        target: fx.sp("x")
                    },
                ],
            }
        );
    }

    #[test]
    fn sync_conflict_skip_is_conflict() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"live");
        fx.write(&fx.sp("x"), b"store");
        assert_eq!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Skip, vec![("x", t())]),
                Verb::Sync
            ),
            Action::Conflict
        );
    }

    #[test]
    fn sync_relinks_when_content_equal() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"same");
        fx.write(&fx.sp("x"), b"same");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
            Verb::Sync,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Relink,
                ops: vec![
                    FsOp::Remove(fx.lp("x")),
                    FsOp::Symlink {
                        link: fx.lp("x"),
                        target: fx.sp("x")
                    }
                ],
            }
        );
    }

    #[test]
    fn sync_already_ok_when_live_is_correct_symlink() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"content");
        symlink(fx.sp("x"), fx.lp("x")).unwrap();
        assert_eq!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
    }

    // ---- sync, copy mode ----

    #[test]
    fn sync_copy_pushes_when_store_missing() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"data");
        let a = act(
            &fx.cfg(Mode::Sync, Conflict::Backup, vec![("x", t())]),
            Verb::Sync,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Push,
                ops: vec![FsOp::Copy {
                    from: fx.lp("x"),
                    to: fx.sp("x")
                }]
            }
        );
    }

    /// Set `dst`'s mtime equal to `src`'s, modelling a `sync` copy (which
    /// preserves mtime) so the size+mtime quick-check sees the pair as in sync.
    fn match_mtime(src: &Path, dst: &Path) {
        let t = std::fs::metadata(src).unwrap().modified().unwrap();
        let f = std::fs::OpenOptions::new().write(true).open(dst).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(t))
            .unwrap();
    }

    fn mtime(p: &Path) -> std::time::SystemTime {
        std::fs::metadata(p).unwrap().modified().unwrap()
    }

    fn set_mtime(p: &Path, t: std::time::SystemTime) {
        let f = std::fs::OpenOptions::new().write(true).open(p).unwrap();
        f.set_times(std::fs::FileTimes::new().set_modified(t))
            .unwrap();
    }

    /// Build an in-sync `store` copy of two live files, with matching mtimes,
    /// so a follow-up `sync` sees the tree as already captured.
    fn synced_dir(fx: &Fx) {
        for f in ["dir/a", "dir/b"] {
            fx.write(&fx.lp(f), f.as_bytes());
            fx.write(&fx.sp(f), f.as_bytes());
            match_mtime(&fx.lp(f), &fx.sp(f));
        }
    }

    #[test]
    fn sync_copy_already_ok_when_equal() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"d");
        fx.write(&fx.sp("x"), b"d");
        match_mtime(&fx.lp("x"), &fx.sp("x")); // in sync: same content, size, mtime
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Backup, vec![("x", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
    }

    #[test]
    fn sync_copy_unchanged_tree_is_already_ok() {
        let fx = Fx::new();
        synced_dir(&fx);
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Backup, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
    }

    #[test]
    fn sync_copy_one_changed_file_does_not_recopy_tree() {
        // The headline win: changing one file in a dir yields ops for *only* that
        // file, never a whole-tree recopy.
        let fx = Fx::new();
        synced_dir(&fx);
        fx.write(&fx.lp("dir/b"), b"b-changed-bigger"); // different size → detected
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Replace, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::Apply {
                kind: ActionKind::Push,
                ops: vec![
                    FsOp::Remove(fx.sp("dir/b")),
                    cp(&fx.lp("dir/b"), &fx.sp("dir/b")),
                ],
            }
        );
    }

    #[test]
    fn sync_copy_new_file_is_added() {
        let fx = Fx::new();
        synced_dir(&fx);
        fx.write(&fx.lp("dir/c"), b"c"); // brand-new, absent in store
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Backup, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::Apply {
                kind: ActionKind::Push,
                ops: vec![cp(&fx.lp("dir/c"), &fx.sp("dir/c"))],
            }
        );
    }

    #[test]
    fn sync_copy_partial_apply_with_skip_drift() {
        // Decision #1: a new file is copied even under `skip`, while a same-path
        // difference is left as drift — surfaced via ApplyDrift.
        let fx = Fx::new();
        synced_dir(&fx);
        fx.write(&fx.lp("dir/c"), b"c"); // new → copied
        fx.write(&fx.lp("dir/b"), b"b-changed-bigger"); // differs → skipped (drift)
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Skip, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::ApplyDrift {
                kind: ActionKind::Push,
                ops: vec![cp(&fx.lp("dir/c"), &fx.sp("dir/c"))],
            }
        );
    }

    #[test]
    fn sync_leaves_extraneous_store_file_untouched() {
        let fx = Fx::new();
        synced_dir(&fx);
        fx.write(&fx.sp("dir/extra"), b"orphan"); // store-only, no live counterpart
        // Additive: store-only files are never pruned — nothing to do.
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Replace, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
    }

    #[test]
    fn sync_ignores_own_bak_artifacts() {
        // A `.bak` artifact is invisible to the walk — never copied as a source add.
        let fx = Fx::new();
        synced_dir(&fx);
        fx.write(&fx.lp("dir/old.20260101.bak"), b"backup");
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Backup, vec![("dir", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
    }

    #[test]
    fn sync_checksum_ignores_mtime_only_change() {
        // Decision #4/#6: bump mtime without changing content. Quick-check sees a
        // difference; --checksum recognises the content is identical.
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"same");
        fx.write(&fx.sp("x"), b"same");
        match_mtime(&fx.lp("x"), &fx.sp("x"));
        let later = mtime(&fx.lp("x")) + std::time::Duration::from_secs(5);
        set_mtime(&fx.lp("x"), later);

        let cfg = fx.cfg(Mode::Sync, Conflict::Replace, vec![("x", t())]);
        // Default quick-check: mtime drift → re-sync.
        assert!(matches!(act(&cfg, Verb::Sync), Action::Apply { .. }));
        // --checksum: content identical → already ok.
        let checksum = RunOptions {
            checksum: true,
            modify_window: 0,
        };
        assert_eq!(act_opts(&cfg, Verb::Sync, checksum), Action::AlreadyOk);
    }

    #[test]
    fn sync_modify_window_tolerates_mtime_skew() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"same");
        fx.write(&fx.sp("x"), b"same");
        match_mtime(&fx.lp("x"), &fx.sp("x"));
        let skewed = mtime(&fx.sp("x")) + std::time::Duration::from_secs(1);
        set_mtime(&fx.sp("x"), skewed);

        let cfg = fx.cfg(Mode::Sync, Conflict::Replace, vec![("x", t())]);
        // Exact (window 0): 1s skew counts as a difference.
        assert!(matches!(act(&cfg, Verb::Sync), Action::Apply { .. }));
        // window 1: tolerated as equal.
        let windowed = RunOptions {
            checksum: false,
            modify_window: 1,
        };
        assert_eq!(act_opts(&cfg, Verb::Sync, windowed), Action::AlreadyOk);
    }

    // ---- deploy ----

    #[test]
    fn deploy_skips_when_store_missing() {
        let fx = Fx::new();
        assert!(matches!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
                Verb::Deploy
            ),
            Action::Skip(_)
        ));
    }

    #[test]
    fn deploy_links_when_live_missing() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"content");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
            Verb::Deploy,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Link,
                ops: vec![FsOp::Symlink {
                    link: fx.lp("x"),
                    target: fx.sp("x")
                }]
            }
        );
    }

    #[test]
    fn deploy_already_ok_when_correct() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"c");
        symlink(fx.sp("x"), fx.lp("x")).unwrap();
        assert_eq!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
                Verb::Deploy
            ),
            Action::AlreadyOk
        );
    }

    #[test]
    fn deploy_conflict_backup_backs_up_live() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"store");
        fx.write(&fx.lp("x"), b"live");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
            Verb::Deploy,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Link,
                ops: vec![
                    FsOp::Backup(fx.lp("x")),
                    FsOp::Symlink {
                        link: fx.lp("x"),
                        target: fx.sp("x")
                    }
                ],
            }
        );
    }

    #[test]
    fn deploy_relinks_when_live_matches() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"same");
        fx.write(&fx.lp("x"), b"same");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("x", t())]),
            Verb::Deploy,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Relink,
                ops: vec![
                    FsOp::Remove(fx.lp("x")),
                    FsOp::Symlink {
                        link: fx.lp("x"),
                        target: fx.sp("x")
                    }
                ],
            }
        );
    }

    #[test]
    fn deploy_copy_pulls_when_live_missing() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"data");
        let a = act(
            &fx.cfg(Mode::Sync, Conflict::Backup, vec![("x", t())]),
            Verb::Deploy,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Pull,
                ops: vec![FsOp::Copy {
                    from: fx.sp("x"),
                    to: fx.lp("x")
                }]
            }
        );
    }

    #[test]
    fn deploy_copy_conflict_skip() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"store");
        fx.write(&fx.lp("x"), b"live");
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Skip, vec![("x", t())]),
                Verb::Deploy
            ),
            Action::Conflict
        );
    }

    // ---- safety guards ----

    #[test]
    fn guard_refuses_entry_at_live_root() {
        let fx = Fx::new();
        // An empty key resolves to the live root itself.
        assert!(matches!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("", t())]),
                Verb::Sync
            ),
            Action::Failed(_)
        ));
    }

    #[test]
    fn guard_refuses_directory_outside_live_root() {
        let fx = Fx::new();
        let outside = fx._tmp.path().join("outside_dir");
        std::fs::create_dir_all(&outside).unwrap();
        let key = outside.to_string_lossy().into_owned();
        assert!(matches!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![(key.as_str(), t())]),
                Verb::Sync
            ),
            Action::Failed(_)
        ));
    }

    #[test]
    fn guard_allows_file_outside_live_root() {
        let fx = Fx::new();
        let outside = fx._tmp.path().join("outside.txt");
        std::fs::write(&outside, b"x").unwrap();
        let key = outside.to_string_lossy().into_owned();
        // A single file outside the live root is adopted in place.
        assert!(matches!(
            act(
                &fx.cfg(Mode::Symlink, Conflict::Backup, vec![(key.as_str(), t())]),
                Verb::Sync
            ),
            Action::Apply {
                kind: ActionKind::Adopt,
                ..
            }
        ));
    }

    #[test]
    fn guard_refuses_live_path_containing_store() {
        // Store nested under live; an entry whose live path is an ancestor of the
        // store root would pull the store into itself.
        let tmp = tempfile::tempdir().unwrap();
        let live = tmp.path().join("live");
        let cfg = ResolvedConfig {
            mappings: vec![ResolvedMapping {
                name: "m".into(),
                store: live.join("sub/store"),
                live,
                mode: Mode::Symlink,
                conflict: Conflict::Backup,
                links: vec![("sub".to_string(), t())],
            }],
        };
        assert!(matches!(act(&cfg, Verb::Sync), Action::Failed(_)));
    }

    #[test]
    fn sync_adopts_directory_in_symlink_mode() {
        let fx = Fx::new();
        fx.write(&fx.lp("dir/file"), b"x");
        let a = act(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("dir", t())]),
            Verb::Sync,
        );
        assert_eq!(
            a,
            Action::Apply {
                kind: ActionKind::Adopt,
                ops: vec![
                    FsOp::Move {
                        from: fx.lp("dir"),
                        to: fx.sp("dir")
                    },
                    FsOp::Symlink {
                        link: fx.lp("dir"),
                        target: fx.sp("dir")
                    },
                ],
            }
        );
    }

    // ---- helpers exercised directly (not just via plan) ----

    /// Borrow the single mapping out of a fixture config.
    fn one_mapping(fx: &Fx) -> ResolvedMapping {
        fx.cfg(Mode::Symlink, Conflict::Backup, vec![])
            .mappings
            .remove(0)
    }

    #[test]
    fn resolve_paths_relative_absolute_and_explicit() {
        let fx = Fx::new();
        let m = one_mapping(&fx);
        // Relative mirror key -> under live / under store.
        let (l, s) = resolve_paths(&m, ".bashrc", LinkKind::Mirror);
        assert_eq!(l, fx.lp(".bashrc"));
        assert_eq!(s, fx.sp(".bashrc"));
        // Absolute key -> literal live, store strips the leading slash.
        let (l, s) = resolve_paths(&m, "/etc/x", LinkKind::Mirror);
        assert_eq!(l, PathBuf::from("/etc/x"));
        assert_eq!(s, fx.sp("etc/x"));
        // Explicit value redirects the store side.
        let (_, s) = resolve_paths(&m, "profile", LinkKind::Explicit("fixed/p"));
        assert_eq!(s, fx.sp("fixed/p"));
    }

    #[test]
    fn guard_reason_flags_protected_roots_store_containment_and_traversal() {
        let fx = Fx::new();
        let m = one_mapping(&fx);
        // The live root itself is protected.
        assert!(
            guard_reason(&m, &fx.live, &fx.sp("x"))
                .unwrap()
                .unwrap()
                .contains("protected root")
        );
        // The store root itself is protected.
        assert!(guard_reason(&m, &fx.lp("x"), &fx.store).unwrap().is_some());
        // A live path that contains the store root is refused.
        let containing = fx.live.parent().unwrap().to_path_buf();
        assert!(
            guard_reason(&m, &containing, &fx.sp("x"))
                .unwrap()
                .unwrap()
                .contains("contains the store root")
        );
        // A clean in-root file passes.
        assert!(
            guard_reason(&m, &fx.lp("ok"), &fx.sp("ok"))
                .unwrap()
                .is_none()
        );
        // A directory outside the live root (e.g. via `..`) is refused.
        let outside = fx.live.parent().unwrap().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        assert!(
            guard_reason(&m, &outside, &fx.sp("outside"))
                .unwrap()
                .unwrap()
                .contains("outside the live root")
        );
        // A *file* outside the live root is allowed (only dirs are refused).
        let file_outside = fx.live.parent().unwrap().join("loose.txt");
        std::fs::write(&file_outside, b"x").unwrap();
        assert!(
            guard_reason(&m, &file_outside, &fx.sp("loose"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn traversal_key_is_guarded_through_plan() {
        let fx = Fx::new();
        // A `..` key that resolves to a directory outside the live root must be
        // refused by the planner, not silently operated on.
        std::fs::create_dir_all(fx.live.join("../escape")).unwrap();
        let cfg = fx.cfg(Mode::Symlink, Conflict::Backup, vec![("../escape", t())]);
        assert!(matches!(act(&cfg, Verb::Deploy), Action::Failed(_)));
    }

    #[test]
    fn empty_mapping_is_a_clean_no_op() {
        let fx = Fx::new();
        let cfg = fx.cfg(Mode::Symlink, Conflict::Backup, vec![]);
        assert!(
            plan(&cfg, Verb::Sync, RunOptions::default())
                .unwrap()
                .is_empty()
        );
        assert!(
            plan(&cfg, Verb::Deploy, RunOptions::default())
                .unwrap()
                .is_empty()
        );
        assert!(
            crate::status::status(&cfg, RunOptions::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn execute_continues_past_failures() {
        use crate::clock::SystemClock;
        let fx = Fx::new();
        // Entry 1 is a guarded failure (a dir outside the live root); entry 2 is
        // a normal deploy link. Execution must report both, not stop at the first.
        std::fs::create_dir_all(fx.live.join("../esc")).unwrap();
        fx.write(&fx.sp("ok"), b"v");
        let cfg = fx.cfg(
            Mode::Symlink,
            Conflict::Backup,
            vec![("../esc", t()), ("ok", t())],
        );
        let planned = plan(&cfg, Verb::Deploy, RunOptions::default()).unwrap();
        let outcomes = execute(&planned, &SystemClock, false);
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes.iter().any(|o| matches!(o, Outcome::Failed(_))));
        assert!(outcomes.iter().any(|o| matches!(o, Outcome::Applied(_))));
    }
}
