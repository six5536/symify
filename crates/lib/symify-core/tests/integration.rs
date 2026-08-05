//! End-to-end tests over real temp directories: plan → execute → status through
//! the public API, asserting actual filesystem state.

use std::path::{Path, PathBuf};

use symify_core::clock::FixedClock;
use symify_core::config::{ResolvedConfig, ResolvedMapping};
use symify_core::model::{Conflict, LinkValue, Mode};
use symify_core::status::StatusLabel;
use symify_core::{Outcome, RunOptions, Verb, execute, plan, status};

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
    fn write(&self, p: &Path, c: &[u8]) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, c).unwrap();
    }
    fn cfg(&self, mode: Mode, conflict: Conflict, links: &[(&str, LinkValue)]) -> ResolvedConfig {
        ResolvedConfig {
            mappings: vec![ResolvedMapping {
                name: "m".into(),
                live: self.live.clone(),
                store: self.store.clone(),
                mode,
                conflict,
                links: links
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
                inactive: None,
            }],
        }
    }
}

fn clock() -> FixedClock {
    FixedClock("20260101000000".into())
}

fn run(cfg: &ResolvedConfig, verb: Verb) -> Vec<Outcome> {
    let planned = plan(cfg, verb, RunOptions::default()).unwrap();
    execute(&planned, &clock(), false)
}

fn symlink_target(p: &Path) -> PathBuf {
    std::fs::read_link(p).unwrap()
}

#[test]
fn sync_adopts_then_status_is_ok_and_idempotent() {
    let fx = Fx::new();
    fx.write(&fx.lp(".bashrc"), b"hello");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Backup,
        &[(".bashrc", LinkValue::Boolean(true))],
    );

    let out = run(&cfg, Verb::Sync);
    assert!(matches!(out[0], Outcome::Applied(_)));

    // Real content now lives in the store; live is a symlink to it.
    assert_eq!(std::fs::read(fx.sp(".bashrc")).unwrap(), b"hello");
    assert_eq!(symlink_target(&fx.lp(".bashrc")), fx.sp(".bashrc"));

    // status reports Ok.
    let st = status(&cfg, RunOptions::default()).unwrap();
    assert_eq!(st[0].label, StatusLabel::Ok);

    // Idempotent: a second sync is a no-op.
    let again = run(&cfg, Verb::Sync);
    assert_eq!(again, vec![Outcome::AlreadyOk]);
}

#[test]
fn deploy_creates_link_on_fresh_machine() {
    let fx = Fx::new();
    fx.write(&fx.sp(".vimrc"), b"set nocompatible");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Backup,
        &[(".vimrc", LinkValue::Boolean(true))],
    );

    let out = run(&cfg, Verb::Deploy);
    assert!(matches!(out[0], Outcome::Applied(_)));
    assert_eq!(symlink_target(&fx.lp(".vimrc")), fx.sp(".vimrc"));

    // Idempotent.
    assert_eq!(run(&cfg, Verb::Deploy), vec![Outcome::AlreadyOk]);
}

#[test]
fn sync_conflict_backup_writes_bak_and_overwrites_store() {
    let fx = Fx::new();
    fx.write(&fx.lp("x"), b"live-wins");
    fx.write(&fx.sp("x"), b"old-store");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Backup,
        &[("x", LinkValue::Boolean(true))],
    );

    run(&cfg, Verb::Sync);

    // Old store content preserved in a timestamped backup.
    assert_eq!(
        std::fs::read(fx.sp("x.20260101000000.bak")).unwrap(),
        b"old-store"
    );
    // Store now holds the live content; live is a symlink.
    assert_eq!(std::fs::read(fx.sp("x")).unwrap(), b"live-wins");
    assert_eq!(symlink_target(&fx.lp("x")), fx.sp("x"));
}

#[test]
fn deploy_conflict_skip_leaves_live_and_reports_conflict() {
    let fx = Fx::new();
    fx.write(&fx.sp("x"), b"store");
    fx.write(&fx.lp("x"), b"live");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Skip,
        &[("x", LinkValue::Boolean(true))],
    );

    assert_eq!(run(&cfg, Verb::Deploy), vec![Outcome::Conflict]);
    // Untouched.
    assert_eq!(std::fs::read(fx.lp("x")).unwrap(), b"live");
}

#[test]
fn dry_run_mutates_nothing() {
    let fx = Fx::new();
    fx.write(&fx.lp(".bashrc"), b"hello");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Backup,
        &[(".bashrc", LinkValue::Boolean(true))],
    );

    let planned = plan(&cfg, Verb::Sync, RunOptions::default()).unwrap();
    let out = execute(&planned, &clock(), true); // dry_run
    assert!(matches!(out[0], Outcome::Applied(_)));

    // Still a real file in live, nothing created in store.
    assert!(fx.lp(".bashrc").is_file());
    assert!(!fx.sp(".bashrc").exists());
}

#[test]
fn continue_on_error_processes_all_entries() {
    let fx = Fx::new();
    // An entry resolving to the live root itself is refused by the safety guard;
    // a sibling file still succeeds. ("" sorts before "f".)
    fx.write(&fx.lp("f"), b"file");
    let cfg = fx.cfg(
        Mode::Symlink,
        Conflict::Backup,
        &[
            ("", LinkValue::Boolean(true)),
            ("f", LinkValue::Boolean(true)),
        ],
    );

    let out = run(&cfg, Verb::Sync); // links sorted: "", f
    assert!(
        matches!(out[0], Outcome::Failed(_)),
        "live-root entry should fail the guard"
    );
    assert!(
        matches!(out[1], Outcome::Applied(_)),
        "file should still apply"
    );

    // The file was adopted despite the earlier failure.
    assert_eq!(std::fs::read(fx.sp("f")).unwrap(), b"file");
}

#[test]
fn sync_copy_mode_preserves_and_tracks_permission_bits() {
    use std::os::unix::fs::PermissionsExt;
    let fx = Fx::new();
    fx.write(&fx.lp("script.sh"), b"#!/bin/sh\necho hi\n");
    std::fs::set_permissions(fx.lp("script.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    let cfg = fx.cfg(
        Mode::Copy,
        Conflict::Backup,
        &[("script.sh", LinkValue::Boolean(true))],
    );

    // Push: the store copy keeps the executable bit, and it's then in sync.
    run(&cfg, Verb::Sync);
    let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&fx.sp("script.sh")), 0o755);
    assert_eq!(run(&cfg, Verb::Sync), vec![Outcome::AlreadyOk]);

    // Change only the live file's mode → detected as drift and re-synced.
    std::fs::set_permissions(fx.lp("script.sh"), std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        status(&cfg, RunOptions::default()).unwrap()[0].label,
        StatusLabel::Differs
    );
    run(&cfg, Verb::Sync);
    assert_eq!(mode(&fx.sp("script.sh")), 0o644);
    assert_eq!(
        status(&cfg, RunOptions::default()).unwrap()[0].label,
        StatusLabel::Ok
    );
}

#[test]
fn sync_copy_mode_roundtrips_directory() {
    let fx = Fx::new();
    fx.write(&fx.lp("conf/a.toml"), b"a");
    fx.write(&fx.lp("conf/sub/b.toml"), b"b");
    let cfg = fx.cfg(
        Mode::Copy,
        Conflict::Backup,
        &[("conf", LinkValue::Boolean(true))],
    );

    run(&cfg, Verb::Sync);
    // Tree copied into the store; live stays a real directory (no link).
    assert_eq!(std::fs::read(fx.sp("conf/sub/b.toml")).unwrap(), b"b");
    assert!(fx.lp("conf").is_dir());
    assert!(
        !fx.lp("conf")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );

    // Now in sync.
    assert_eq!(
        status(&cfg, RunOptions::default()).unwrap()[0].label,
        StatusLabel::Ok
    );
}
