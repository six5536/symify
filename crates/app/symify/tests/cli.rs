//! End-to-end CLI tests: invoke the real binary against temp trees and assert
//! output, JSON, and exit codes.

use std::path::{Path, PathBuf};

use assert_cmd::Command;

struct Fx {
    _tmp: tempfile::TempDir,
    live: PathBuf,
    store: PathBuf,
    config: PathBuf,
}

impl Fx {
    /// Build a fixture with one symlink-mode mapping and the given conflict
    /// policy plus link entries (`key = value` TOML lines).
    fn new(conflict: &str, links: &[&str]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let live = tmp.path().join("live");
        let store = tmp.path().join("store");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&store).unwrap();
        let config = tmp.path().join("symify.toml");
        let links = links.join("\n");
        std::fs::write(
            &config,
            format!(
                "[settings]\nlive = \"{}\"\nstore = \"{}\"\nmode = \"symlink\"\nconflict = \"{conflict}\"\n\n[mappings.dotfiles.links]\n{links}\n",
                live.display(),
                store.display(),
            ),
        )
        .unwrap();
        Fx {
            _tmp: tmp,
            live,
            store,
            config,
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
    fn cmd(&self, verb: &str) -> Command {
        let mut c = Command::cargo_bin("symify").unwrap();
        c.arg(verb).arg("-c").arg(&self.config);
        c
    }
}

#[test]
fn sync_adopts_and_status_reports_ok_human_output() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");

    let out = fx.cmd("sync").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    insta::assert_snapshot!(stdout, @r"
      + adopt    dotfiles/.bashrc

    sync: 1 changed, 0 ok, 0 skipped, 0 disabled, 0 conflicts, 0 failed
    ");

    // The file was adopted into the store and replaced by a symlink.
    assert_eq!(std::fs::read(fx.sp(".bashrc")).unwrap(), b"hi");
    assert!(
        fx.lp(".bashrc")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );

    let st = fx.cmd("status").assert().success();
    let st_out = String::from_utf8_lossy(&st.get_output().stdout).into_owned();
    insta::assert_snapshot!(st_out, @r"
      ok            dotfiles/.bashrc

    status: 1 ok, 0 drift, 0 failed
    ");
}

#[test]
fn status_reports_drift_with_exit_1() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi"); // unadopted: live real file, store missing
    fx.cmd("status").assert().code(1);
}

#[test]
fn sync_is_idempotent() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");
    fx.cmd("sync").assert().success();
    // Second run: nothing changed, still success.
    let out = fx.cmd("sync").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("0 changed, 1 ok"));
}

#[test]
fn deploy_conflict_skip_exits_1_and_keeps_live() {
    let fx = Fx::new("skip", &["\"x\" = true"]);
    fx.write(&fx.sp("x"), b"store");
    fx.write(&fx.lp("x"), b"live");
    fx.cmd("deploy").assert().code(1);
    assert_eq!(std::fs::read(fx.lp("x")).unwrap(), b"live");
}

#[test]
fn dry_run_makes_no_changes() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");
    let out = fx.cmd("sync").arg("--dry-run").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("dry run"));
    assert!(!fx.sp(".bashrc").exists());
    assert!(fx.lp(".bashrc").is_file());
}

#[test]
fn json_output_is_machine_readable() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");
    let out = fx.cmd("sync").arg("--json").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["verb"], "sync");
    assert_eq!(doc["summary"]["changed"], 1);
    assert_eq!(doc["entries"][0]["mapping"], "dotfiles");
    assert_eq!(doc["entries"][0]["action"], "adopt");
}

#[test]
fn failure_exits_2() {
    // A directory entry in hardlink mode fails to plan/apply.
    let tmp = tempfile::tempdir().unwrap();
    let live = tmp.path().join("live");
    let store = tmp.path().join("store");
    std::fs::create_dir_all(live.join("d")).unwrap();
    std::fs::write(live.join("d/f"), b"x").unwrap();
    std::fs::create_dir_all(&store).unwrap();
    let config = tmp.path().join("symify.toml");
    std::fs::write(
        &config,
        format!(
            "[settings]\nlive = \"{}\"\nstore = \"{}\"\nmode = \"hardlink\"\nconflict = \"backup\"\n\n[mappings.m.links]\n\"d\" = true\n",
            live.display(),
            store.display()
        ),
    )
    .unwrap();
    Command::cargo_bin("symify")
        .unwrap()
        .arg("sync")
        .arg("-c")
        .arg(&config)
        .assert()
        .code(2);
}
