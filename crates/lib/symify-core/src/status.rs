//! The `status` verb: a read-only, direction-neutral report of each entry's
//! state. It never claims which way to sync — it just describes what is.

use std::path::{Path, PathBuf};

use crate::Result;
use crate::config::ResolvedConfig;
use crate::fs::{self, NodeKind};
use crate::model::{LinkKind, Mode};

/// Direction-neutral state of a single entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusLabel {
    /// Entry disabled (`value = false`).
    Disabled,
    /// Link modes: a correct link. Copy mode: content is in sync.
    Ok,
    /// Link modes: the live path is a real file, not yet a link.
    Unadopted,
    /// Link modes: the live path is a symlink to the wrong target.
    WrongTarget,
    /// The live side is absent.
    LiveMissing,
    /// The store side is absent.
    StoreMissing,
    /// Both sides are absent.
    Missing,
    /// Copy mode: both sides exist but differ.
    Differs,
    /// The entry is in an unusable state (e.g. a directory in hardlink mode).
    Failed(String),
}

impl StatusLabel {
    /// True when the entry is fully in sync (or intentionally disabled).
    pub fn is_clean(&self) -> bool {
        matches!(self, StatusLabel::Ok | StatusLabel::Disabled)
    }

    /// True when the entry is in an unusable/error state.
    pub fn is_failure(&self) -> bool {
        matches!(self, StatusLabel::Failed(_))
    }
}

/// A status report for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusEntry {
    /// Mapping name.
    pub mapping: String,
    /// Entry key.
    pub key: String,
    /// Absolute live-side path.
    pub live: PathBuf,
    /// Absolute store-side path.
    pub store: PathBuf,
    /// Effective mode.
    pub mode: Mode,
    /// The computed label.
    pub label: StatusLabel,
}

/// Report the status of every entry in the resolved config.
pub fn status(config: &ResolvedConfig) -> Result<Vec<StatusEntry>> {
    let mut out = Vec::new();
    for m in &config.mappings {
        for (key, value) in &m.links {
            let kind = value.kind();
            let (live, store) = crate::plan::resolve_paths(m, key, kind);
            let label = if matches!(kind, LinkKind::Disabled) {
                StatusLabel::Disabled
            } else {
                label_for(&live, &store, m.mode)?
            };
            out.push(StatusEntry {
                mapping: m.name.clone(),
                key: key.clone(),
                live,
                store,
                mode: m.mode,
                label,
            });
        }
    }
    Ok(out)
}

fn label_for(s: &Path, d: &Path, mode: Mode) -> Result<StatusLabel> {
    let s_state = fs::inspect(s)?;
    let d_state = fs::inspect(d)?;

    match mode {
        Mode::Symlink | Mode::Hardlink => {
            if mode == Mode::Hardlink && (s_state == NodeKind::Dir || d_state == NodeKind::Dir) {
                return Ok(StatusLabel::Failed(
                    "hardlink mode cannot link a directory".into(),
                ));
            }

            let correct = match mode {
                Mode::Hardlink => fs::same_inode(s, d)?,
                _ => fs::symlink_points_to(s, d)? && !d_state.is_missing(),
            };
            if correct {
                return Ok(StatusLabel::Ok);
            }

            Ok(match (&s_state, d_state.is_missing()) {
                (NodeKind::Missing, false) => StatusLabel::LiveMissing,
                (NodeKind::Missing, true) => StatusLabel::Missing,
                (_, true) => StatusLabel::StoreMissing,
                (NodeKind::Symlink(_), false) => StatusLabel::WrongTarget,
                (_, false) => StatusLabel::Unadopted,
            })
        }
        Mode::Sync => Ok(match (s_state.is_missing(), d_state.is_missing()) {
            (true, true) => StatusLabel::Missing,
            (false, true) => StatusLabel::StoreMissing,
            (true, false) => StatusLabel::LiveMissing,
            (false, false) => {
                if fs::synced_equal(s, d)? {
                    StatusLabel::Ok
                } else {
                    StatusLabel::Differs
                }
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ResolvedConfig, ResolvedMapping};
    use crate::model::{Conflict, LinkValue};
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;

    struct Fx {
        _tmp: tempfile::TempDir,
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
        fn lp(&self, r: &str) -> PathBuf {
            self.live.join(r)
        }
        fn sp(&self, r: &str) -> PathBuf {
            self.store.join(r)
        }
        fn cfg(&self, mode: Mode, links: Vec<(&str, LinkValue)>) -> ResolvedConfig {
            ResolvedConfig {
                mappings: vec![ResolvedMapping {
                    name: "m".into(),
                    live: self.live.clone(),
                    store: self.store.clone(),
                    mode,
                    conflict: Conflict::Backup,
                    links: links.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                }],
            }
        }
    }
    fn t() -> LinkValue {
        LinkValue::Boolean(true)
    }
    fn label(cfg: &ResolvedConfig) -> StatusLabel {
        status(cfg).unwrap().remove(0).label
    }

    #[test]
    fn symlink_states() {
        let fx = Fx::new();
        // correct link
        std::fs::write(fx.sp("a"), b"x").unwrap();
        symlink(fx.sp("a"), fx.lp("a")).unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("a", t())])),
            StatusLabel::Ok
        );

        // unadopted: live is a real file
        std::fs::write(fx.lp("b"), b"x").unwrap();
        std::fs::write(fx.sp("b"), b"x").unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("b", t())])),
            StatusLabel::Unadopted
        );

        // live missing, store present
        std::fs::write(fx.sp("c"), b"x").unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("c", t())])),
            StatusLabel::LiveMissing
        );

        // wrong target
        std::fs::write(fx.sp("d"), b"x").unwrap();
        symlink(fx.sp("elsewhere"), fx.lp("d")).unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("d", t())])),
            StatusLabel::WrongTarget
        );

        // both missing
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("gone", t())])),
            StatusLabel::Missing
        );
    }

    #[test]
    fn copy_states() {
        let fx = Fx::new();
        std::fs::write(fx.lp("a"), b"x").unwrap();
        std::fs::write(fx.sp("a"), b"x").unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Sync, vec![("a", t())])),
            StatusLabel::Ok
        );

        std::fs::write(fx.lp("b"), b"one").unwrap();
        std::fs::write(fx.sp("b"), b"two").unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Sync, vec![("b", t())])),
            StatusLabel::Differs
        );

        std::fs::write(fx.lp("c"), b"x").unwrap();
        assert_eq!(
            label(&fx.cfg(Mode::Sync, vec![("c", t())])),
            StatusLabel::StoreMissing
        );
    }

    #[test]
    fn disabled_and_hardlink_dir() {
        let fx = Fx::new();
        assert_eq!(
            label(&fx.cfg(Mode::Symlink, vec![("x", LinkValue::Boolean(false))])),
            StatusLabel::Disabled
        );
        std::fs::create_dir_all(fx.sp("d")).unwrap();
        assert!(matches!(
            label(&fx.cfg(Mode::Hardlink, vec![("d", t())])),
            StatusLabel::Failed(_)
        ));
    }
}
