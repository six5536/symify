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
    /// One symlink-mode `dotfiles` mapping with the given conflict policy and
    /// link lines (`key = value` TOML).
    fn new(conflict: &str, links: &[&str]) -> Self {
        Self::with_body(
            &format!("[mappings.dotfiles.links]\n{}\n", links.join("\n")),
            conflict,
        )
    }

    /// Like `new`, but lets the test supply the `[mappings.*]` body verbatim
    /// (settings are prepended).
    fn with_body(body: &str, conflict: &str) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let live = tmp.path().join("live");
        let store = tmp.path().join("store");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&store).unwrap();
        let config = tmp.path().join("symify.toml");
        std::fs::write(
            &config,
            format!(
                "[settings]\nlive = \"{}\"\nstore = \"{}\"\nmode = \"symlink\"\nconflict = \"{conflict}\"\n\n{body}",
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
    fn config_text(&self) -> String {
        std::fs::read_to_string(&self.config).unwrap()
    }
    fn cmd(&self, verb: &str) -> Command {
        let mut c = Command::cargo_bin("symify").unwrap();
        c.arg(verb).arg("-c").arg(&self.config);
        c
    }
}

fn is_symlink(p: &Path) -> bool {
    p.symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
}

// ----- sync / deploy / status -------------------------------------------

#[test]
fn sync_adopts_and_status_reports_ok_human_output() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");

    let out = fx.cmd("sync").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("+ adopt"), "got: {stdout}");
    assert!(stdout.contains("dotfiles/.bashrc"));
    assert!(stdout.contains("sync: 1 changed"));

    assert_eq!(std::fs::read(fx.sp(".bashrc")).unwrap(), b"hi");
    assert!(is_symlink(&fx.lp(".bashrc")));

    let st = fx.cmd("status").assert().success();
    let st_out = String::from_utf8_lossy(&st.get_output().stdout).into_owned();
    assert!(st_out.contains("ok"), "got: {st_out}");
    assert!(st_out.contains("dotfiles/.bashrc"));
    assert!(st_out.contains("status: 1 ok, 0 drift, 0 failed"));
}

#[test]
fn status_reports_drift_with_exit_1() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi"); // unadopted: live real file, store missing
    fx.cmd("status").assert().code(1);
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
fn json_output_is_machine_readable() {
    let fx = Fx::new("backup", &["\".bashrc\" = true"]);
    fx.write(&fx.lp(".bashrc"), b"hi");
    let out = fx.cmd("sync").arg("--json").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    let doc: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(doc["verb"], "sync");
    assert_eq!(doc["summary"]["changed"], 1);
    assert_eq!(doc["entries"][0]["action"], "adopt");
}

// ----- add / remove / list ----------------------------------------------

#[test]
fn add_adopts_file_and_appears_in_list() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");

    fx.cmd("add").arg(fx.lp(".zshrc")).assert().success();

    // adopted: store has the content, live is a symlink
    assert_eq!(std::fs::read(fx.sp(".zshrc")).unwrap(), b"z");
    assert!(is_symlink(&fx.lp(".zshrc")));
    assert!(fx.config_text().contains("\".zshrc\""));

    let out = fx.cmd("list").arg("--entries").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("dotfiles"));
    assert!(stdout.contains(".zshrc"));
}

#[test]
fn add_preserves_comments_and_schema_line() {
    let fx = Fx::with_body(
        "# tracked files\n[mappings.dotfiles.links]\n# examples here\n",
        "backup",
    );
    // Prepend a schema line to exercise its preservation.
    let with_schema = format!(
        "#:schema https://example/symify.schema.json\n{}",
        fx.config_text()
    );
    std::fs::write(&fx.config, with_schema).unwrap();
    fx.write(&fx.lp(".vimrc"), b"v");

    fx.cmd("add").arg(fx.lp(".vimrc")).assert().success();

    let text = fx.config_text();
    assert!(text.contains("#:schema https://example/symify.schema.json"));
    assert!(text.contains("# tracked files"));
    assert!(text.contains("# examples here"));
    assert!(text.contains("\".vimrc\""));
}

#[test]
fn add_dry_run_changes_nothing() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");
    fx.cmd("add")
        .arg(fx.lp(".zshrc"))
        .arg("--dry-run")
        .assert()
        .success();
    assert!(!is_symlink(&fx.lp(".zshrc")));
    assert!(!fx.sp(".zshrc").exists());
    assert!(!fx.config_text().contains(".zshrc"));
}

#[test]
fn add_missing_file_errors() {
    let fx = Fx::new("backup", &[]);
    fx.cmd("add").arg(fx.lp("nope")).assert().code(2);
}

#[test]
fn add_resolves_relative_path_against_cwd() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");
    // A bare relative path is resolved against the working directory (model B).
    fx.cmd("add")
        .arg(".zshrc")
        .current_dir(&fx.live)
        .assert()
        .success();
    assert!(is_symlink(&fx.lp(".zshrc")));
    assert!(fx.config_text().contains("\".zshrc\""));
}

#[test]
fn add_file_outside_live_uses_absolute_key() {
    let fx = Fx::new("backup", &[]);
    // A sibling of `live`, i.e. outside the mapping's live root.
    let outside = fx.live.parent().unwrap().join("outside.txt");
    std::fs::write(&outside, b"o").unwrap();

    fx.cmd("add").arg(&outside).assert().success();
    assert!(is_symlink(&outside)); // adopted in place via an absolute key
    assert!(fx.config_text().contains(&outside.display().to_string()));
}

#[test]
fn remove_restores_standalone_file_and_clears_entry() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");
    fx.cmd("add").arg(fx.lp(".zshrc")).assert().success();
    assert!(is_symlink(&fx.lp(".zshrc")));

    fx.cmd("remove").arg(fx.lp(".zshrc")).assert().success();
    assert!(!is_symlink(&fx.lp(".zshrc")));
    assert_eq!(std::fs::read(fx.lp(".zshrc")).unwrap(), b"z"); // standalone copy restored
    assert!(!fx.config_text().contains(".zshrc")); // entry gone
}

#[test]
fn remove_no_restore_leaves_link() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");
    fx.cmd("add").arg(fx.lp(".zshrc")).assert().success();
    fx.cmd("remove")
        .arg(fx.lp(".zshrc"))
        .arg("--no-restore")
        .assert()
        .success();
    assert!(is_symlink(&fx.lp(".zshrc"))); // link untouched
    assert!(!fx.config_text().contains(".zshrc"));
}

#[test]
fn rm_and_ls_aliases_work() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");
    fx.cmd("add").arg(fx.lp(".zshrc")).assert().success();
    fx.cmd("ls").assert().success();
    fx.cmd("rm").arg(fx.lp(".zshrc")).assert().success();
    assert!(!fx.config_text().contains(".zshrc"));
}

// ----- mapping scoping --------------------------------------------------

#[test]
fn sync_scopes_to_named_mapping_and_errors_on_unknown() {
    // Two mappings sharing roots; both have an unadopted real file.
    let fx = Fx::with_body(
        "[mappings.a.links]\n\"a.txt\" = true\n\n[mappings.b.links]\n\"b.txt\" = true\n",
        "backup",
    );
    fx.write(&fx.lp("a.txt"), b"a");
    fx.write(&fx.lp("b.txt"), b"b");

    fx.cmd("sync").arg("-m").arg("a").assert().success();
    assert!(is_symlink(&fx.lp("a.txt"))); // a adopted
    assert!(!is_symlink(&fx.lp("b.txt"))); // b untouched

    fx.cmd("sync").arg("-m").arg("nope").assert().code(2); // unknown mapping
}

// ----- auto-init --------------------------------------------------------

#[test]
fn auto_init_creates_default_config() {
    let home = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("symify")
        .unwrap()
        .arg("status")
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .assert()
        .success();
    // The notice goes to stderr, so it never pollutes stdout (e.g. --json).
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(
        stderr.contains("Created"),
        "expected auto-init notice on stderr, got: {stderr}"
    );
    assert!(xdg.path().join("symify").join("symify.toml").is_file());
}

#[test]
fn auto_init_with_json_keeps_stdout_clean() {
    let home = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let out = Command::cargo_bin("symify")
        .unwrap()
        .args(["status", "--json"])
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", xdg.path())
        .assert()
        .success();
    // stdout must be valid JSON even though auto-init fired.
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    serde_json::from_str::<serde_json::Value>(&stdout)
        .unwrap_or_else(|e| panic!("stdout not clean JSON ({e}): {stdout}"));
}
