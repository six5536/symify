//! The pure planner.
//!
//! [`plan`] is a function of *(resolved config + current filesystem state)*. It
//! reads the filesystem but never mutates it, emitting an ordered list of
//! [`Planned`] entries. Each entry's [`Action`] either records a no-op/blocked
//! state or carries the exact ordered [`FsOp`]s the executor will run. The
//! per-entry state machine is documented in `specs/ARCHITECTURE.md`.

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
    /// Create a hardlink at `link` sharing `target`'s inode.
    Hardlink {
        /// The link location.
        link: PathBuf,
        /// The existing file to share.
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
pub fn plan(config: &ResolvedConfig, verb: Verb) -> Result<Vec<Planned>> {
    let mut out = Vec::new();
    for mapping in &config.mappings {
        for (key, value) in &mapping.links {
            out.push(plan_entry(mapping, key, value, verb)?);
        }
    }
    Ok(out)
}

/// Outcome of executing one planned entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The action was applied (or, under `--dry-run`, would be applied).
    Applied(ActionKind),
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
    /// True when the entry represents drift (a conflict, or a pending change).
    pub fn is_drift(&self) -> bool {
        matches!(self, Outcome::Conflict)
    }

    /// True when the entry failed.
    pub fn is_failure(&self) -> bool {
        matches!(self, Outcome::Failed(_))
    }
}

/// Execute planned entries in order, continuing past failures (each entry is
/// independent — there is no rollback). Under `dry_run` nothing is mutated and
/// `Apply` entries report `Applied` as if they had run.
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
        Action::Apply { kind, ops } => {
            if dry_run {
                return Outcome::Applied(*kind);
            }
            for op in ops {
                if let Err(e) = fs::apply_op(op, clock) {
                    return Outcome::Failed(e.to_string());
                }
            }
            Outcome::Applied(*kind)
        }
    }
}

fn plan_entry(m: &ResolvedMapping, key: &str, value: &LinkValue, verb: Verb) -> Result<Planned> {
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
    let action = match verb {
        Verb::Sync => plan_sync(&s, &d, m.mode, m.conflict)?,
        Verb::Deploy => plan_deploy(&s, &d, m.mode, m.conflict)?,
    };
    Ok(make(s, d, action))
}

/// Resolve an entry's absolute `(live, store)` paths from its key and value.
/// Public wrapper over [`resolve_paths`] for callers like `list`.
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

fn link_op(s: &Path, d: &Path, mode: Mode) -> FsOp {
    match mode {
        Mode::Hardlink => FsOp::Hardlink {
            link: s.to_path_buf(),
            target: d.to_path_buf(),
        },
        _ => FsOp::Symlink {
            link: s.to_path_buf(),
            target: d.to_path_buf(),
        },
    }
}

// ----- sync (live -> store) ---------------------------------------------

fn plan_sync(s: &Path, d: &Path, mode: Mode, conflict: Conflict) -> Result<Action> {
    let s_state = fs::inspect(s)?;
    match mode {
        Mode::Symlink | Mode::Hardlink => plan_sync_link(s, d, mode, conflict, s_state),
        Mode::Sync => plan_sync_copy(s, d, conflict, s_state),
    }
}

fn plan_sync_link(
    s: &Path,
    d: &Path,
    mode: Mode,
    conflict: Conflict,
    s_state: NodeKind,
) -> Result<Action> {
    match s_state {
        NodeKind::Missing => Ok(Action::Skip("live path missing — nothing to capture")),
        NodeKind::Symlink(_) => {
            // A link carries no independent content to capture.
            if mode == Mode::Symlink
                && fs::symlink_points_to(s, d)?
                && !fs::inspect(d)?.is_missing()
            {
                Ok(Action::AlreadyOk)
            } else {
                Ok(Action::Skip(
                    "live path is a symlink — no content to capture",
                ))
            }
        }
        NodeKind::File | NodeKind::Dir => {
            if mode == Mode::Hardlink {
                if fs::same_inode(s, d)? {
                    return Ok(Action::AlreadyOk);
                }
                if s_state == NodeKind::Dir {
                    return Ok(Action::Failed(
                        "hardlink mode cannot link a directory".into(),
                    ));
                }
            }
            match fs::inspect(d)? {
                NodeKind::Missing => Ok(Action::Apply {
                    kind: ActionKind::Adopt,
                    ops: vec![mv(s, d), link_op(s, d, mode)],
                }),
                _ if fs::content_equal(s, d)? => Ok(Action::Apply {
                    kind: ActionKind::Relink,
                    ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d, mode)],
                }),
                _ => match conflict {
                    Conflict::Skip => Ok(Action::Conflict),
                    Conflict::Backup => Ok(Action::Apply {
                        kind: ActionKind::Adopt,
                        ops: vec![FsOp::Backup(d.to_path_buf()), mv(s, d), link_op(s, d, mode)],
                    }),
                    Conflict::Replace => Ok(Action::Apply {
                        kind: ActionKind::Adopt,
                        ops: vec![FsOp::Remove(d.to_path_buf()), mv(s, d), link_op(s, d, mode)],
                    }),
                },
            }
        }
    }
}

fn plan_sync_copy(s: &Path, d: &Path, conflict: Conflict, s_state: NodeKind) -> Result<Action> {
    if s_state.is_missing() {
        return Ok(Action::Skip("live path missing — nothing to capture"));
    }
    match fs::inspect(d)? {
        NodeKind::Missing => Ok(Action::Apply {
            kind: ActionKind::Push,
            ops: vec![cp(s, d)],
        }),
        _ if fs::synced_equal(s, d)? => Ok(Action::AlreadyOk),
        _ => match conflict {
            Conflict::Skip => Ok(Action::Conflict),
            Conflict::Backup => Ok(Action::Apply {
                kind: ActionKind::Push,
                ops: vec![FsOp::Backup(d.to_path_buf()), cp(s, d)],
            }),
            Conflict::Replace => Ok(Action::Apply {
                kind: ActionKind::Push,
                ops: vec![FsOp::Remove(d.to_path_buf()), cp(s, d)],
            }),
        },
    }
}

// ----- deploy (store -> live) -------------------------------------------

fn plan_deploy(s: &Path, d: &Path, mode: Mode, conflict: Conflict) -> Result<Action> {
    if fs::inspect(d)?.is_missing() {
        return Ok(Action::Skip("store path missing — nothing to deploy"));
    }
    match mode {
        Mode::Symlink | Mode::Hardlink => plan_deploy_link(s, d, mode, conflict),
        Mode::Sync => plan_deploy_copy(s, d, conflict),
    }
}

fn plan_deploy_link(s: &Path, d: &Path, mode: Mode, conflict: Conflict) -> Result<Action> {
    if mode == Mode::Hardlink && fs::inspect(d)? == NodeKind::Dir {
        return Ok(Action::Failed(
            "hardlink mode cannot link a directory".into(),
        ));
    }

    // Already in desired state?
    match mode {
        Mode::Symlink if fs::symlink_points_to(s, d)? => return Ok(Action::AlreadyOk),
        Mode::Hardlink if fs::same_inode(s, d)? => return Ok(Action::AlreadyOk),
        _ => {}
    }

    let s_state = fs::inspect(s)?;
    if s_state.is_missing() {
        return Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![link_op(s, d, mode)],
        });
    }

    // S exists but is wrong. If it's a real file/dir already matching D, relink
    // without a backup; otherwise apply the conflict policy.
    let can_relink = matches!(s_state, NodeKind::File | NodeKind::Dir) && fs::content_equal(s, d)?;
    if can_relink {
        return Ok(Action::Apply {
            kind: ActionKind::Relink,
            ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d, mode)],
        });
    }
    match conflict {
        Conflict::Skip => Ok(Action::Conflict),
        Conflict::Backup => Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![FsOp::Backup(s.to_path_buf()), link_op(s, d, mode)],
        }),
        Conflict::Replace => Ok(Action::Apply {
            kind: ActionKind::Link,
            ops: vec![FsOp::Remove(s.to_path_buf()), link_op(s, d, mode)],
        }),
    }
}

fn plan_deploy_copy(s: &Path, d: &Path, conflict: Conflict) -> Result<Action> {
    let s_state = fs::inspect(s)?;
    if s_state.is_missing() {
        return Ok(Action::Apply {
            kind: ActionKind::Pull,
            ops: vec![cp(d, s)],
        });
    }
    if fs::synced_equal(s, d)? {
        return Ok(Action::AlreadyOk);
    }
    match conflict {
        Conflict::Skip => Ok(Action::Conflict),
        Conflict::Backup => Ok(Action::Apply {
            kind: ActionKind::Pull,
            ops: vec![FsOp::Backup(s.to_path_buf()), cp(d, s)],
        }),
        Conflict::Replace => Ok(Action::Apply {
            kind: ActionKind::Pull,
            ops: vec![FsOp::Remove(s.to_path_buf()), cp(d, s)],
        }),
    }
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
        fn mkdir(&self, p: &Path) {
            std::fs::create_dir_all(p).unwrap();
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
        let mut planned = plan(cfg, verb).unwrap();
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
        )
        .unwrap();
        assert_eq!(p[0].live, fx.lp(".bashrc"));
        assert_eq!(p[0].store, fx.sp(".bashrc"));
        // absolute key mirror -> store strips leading /
        let p = plan(
            &fx.cfg(Mode::Symlink, Conflict::Backup, vec![("/etc/x.conf", t())]),
            Verb::Deploy,
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

    #[test]
    fn sync_hardlink_dir_fails() {
        let fx = Fx::new();
        fx.mkdir(&fx.lp("d"));
        assert!(matches!(
            act(
                &fx.cfg(Mode::Hardlink, Conflict::Backup, vec![("d", t())]),
                Verb::Sync
            ),
            Action::Failed(_)
        ));
    }

    #[test]
    fn sync_hardlink_already_ok_when_same_inode() {
        let fx = Fx::new();
        fx.write(&fx.sp("x"), b"x");
        std::fs::hard_link(fx.sp("x"), fx.lp("x")).unwrap();
        assert_eq!(
            act(
                &fx.cfg(Mode::Hardlink, Conflict::Backup, vec![("x", t())]),
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

    #[test]
    fn sync_copy_already_ok_when_equal() {
        let fx = Fx::new();
        fx.write(&fx.lp("x"), b"d");
        fx.write(&fx.sp("x"), b"d");
        assert_eq!(
            act(
                &fx.cfg(Mode::Sync, Conflict::Backup, vec![("x", t())]),
                Verb::Sync
            ),
            Action::AlreadyOk
        );
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
    fn deploy_hardlink_dir_fails() {
        let fx = Fx::new();
        fx.mkdir(&fx.sp("d"));
        assert!(matches!(
            act(
                &fx.cfg(Mode::Hardlink, Conflict::Backup, vec![("d", t())]),
                Verb::Deploy
            ),
            Action::Failed(_)
        ));
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
}
