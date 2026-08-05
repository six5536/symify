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
    /// A single `mode = "copy"` mapping with the given conflict policy
    /// and link lines.
    fn copy(conflict: &str, links: &[&str]) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let live = tmp.path().join("live");
        let store = tmp.path().join("store");
        std::fs::create_dir_all(&live).unwrap();
        std::fs::create_dir_all(&store).unwrap();
        let config = tmp.path().join("symify.toml");
        std::fs::write(
            &config,
            format!(
                "[settings]\nlive = \"{}\"\nstore = \"{}\"\nmode = \"copy\"\nconflict = \"{conflict}\"\n\n[mappings.dotfiles.links]\n{}\n",
                live.display(),
                store.display(),
                links.join("\n"),
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

#[cfg(unix)]
#[test]
fn add_relative_path_relativizes_through_a_symlinked_root() {
    // Regression: a relative argument is made absolute against the CWD, which
    // the OS reports already resolved through symlinks, while the configured
    // live root is raw config text. macOS hits this on every temp dir, since
    // /var is a symlink to /private/var — the shape is built explicitly here so
    // the case is covered everywhere rather than only on one runner.
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real");
    let live = real.join("live");
    std::fs::create_dir_all(&live).unwrap();
    std::fs::create_dir_all(real.join("store")).unwrap();

    let link = tmp.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let config = tmp.path().join("symify.toml");
    std::fs::write(
        &config,
        format!(
            "[settings]\nlive = \"{}\"\nstore = \"{}\"\nmode = \"symlink\"\nconflict = \"backup\"\n\n[mappings.dotfiles.links]\n",
            link.join("live").display(),
            link.join("store").display(),
        ),
    )
    .unwrap();
    std::fs::write(live.join(".zshrc"), b"z").unwrap();

    Command::cargo_bin("symify")
        .unwrap()
        .args(["add", ".zshrc", "-c"])
        .arg(&config)
        .current_dir(link.join("live"))
        .assert()
        .success();

    let text = std::fs::read_to_string(&config).unwrap();
    assert!(
        text.contains("\".zshrc\""),
        "key should be relative to the live root, got: {text}"
    );
    assert!(is_symlink(&live.join(".zshrc")));
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

#[test]
fn bare_path_is_shorthand_for_add() {
    let fx = Fx::new("backup", &[]);
    fx.write(&fx.lp(".zshrc"), b"z");

    // `symify <PATH>` with no verb == `symify add <PATH>`.
    Command::cargo_bin("symify")
        .unwrap()
        .arg(fx.lp(".zshrc"))
        .arg("-c")
        .arg(&fx.config)
        .assert()
        .success();

    assert_eq!(std::fs::read(fx.sp(".zshrc")).unwrap(), b"z");
    assert!(is_symlink(&fx.lp(".zshrc")));
    assert!(fx.config_text().contains("\".zshrc\""));
}

#[test]
fn bare_invocation_prints_help() {
    let out = Command::cargo_bin("symify").unwrap().assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("Usage:"), "got: {stdout}");
}

// ----- version ----------------------------------------------------------

#[test]
fn version_flag_prints_name_and_number() {
    let expected = format!("symify {}", env!("CARGO_PKG_VERSION"));
    for flag in ["-V", "--version"] {
        let out = Command::cargo_bin("symify")
            .unwrap()
            .arg(flag)
            .assert()
            .success();
        let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
        assert_eq!(stdout.trim(), expected, "flag {flag}");
    }
}

#[test]
fn version_flag_is_global() {
    // `-V` is declared global, so it works after a subcommand too.
    let out = Command::cargo_bin("symify")
        .unwrap()
        .args(["sync", "-V"])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert_eq!(
        stdout.trim(),
        format!("symify {}", env!("CARGO_PKG_VERSION"))
    );
}

// ----- completions / man ------------------------------------------------

#[test]
fn completions_generates_for_every_supported_shell() {
    for (shell, needle) in [
        ("bash", "_symify()"),
        ("zsh", "#compdef symify"),
        ("fish", "complete -c symify"),
        ("powershell", "Register-ArgumentCompleter"),
        ("elvish", "set edit:completion:arg-completer[symify]"),
    ] {
        let out = Command::cargo_bin("symify")
            .unwrap()
            .args(["completions", shell])
            .assert()
            .success();
        let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
        assert!(stdout.contains(needle), "shell {shell} got: {stdout}");
    }
}

#[test]
fn completions_rejects_unknown_shell() {
    Command::cargo_bin("symify")
        .unwrap()
        .args(["completions", "nonsuch"])
        .assert()
        .failure();
}

#[test]
fn man_renders_roff_with_version() {
    let out = Command::cargo_bin("symify")
        .unwrap()
        .arg("man")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains(".TH symify 1"), "got: {stdout}");
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "man page should carry the version, got: {stdout}"
    );
    assert!(stdout.contains(".SH NAME"), "got: {stdout}");
}

#[test]
fn man_is_hidden_from_help_but_completions_is_not() {
    let out = Command::cargo_bin("symify")
        .unwrap()
        .arg("--help")
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("completions"), "got: {stdout}");
    assert!(
        !stdout.contains("Print a roff man page"),
        "man should be hidden, got: {stdout}"
    );
}

/// A reader that stops early (`symify … | head`) must not turn into a crash or a
/// spurious error. `clap_complete` `expect()`s its writes, so before this was
/// fixed `completions fish` hit the closed pipe and died with SIGABRT under
/// `panic = "abort"`; the config-reading verbs printed
/// `error: I/O error at <stdout>: Broken pipe` and exited 2.
#[test]
fn closed_stdout_exits_cleanly_and_quietly() {
    let fx = Fx::new("backup", &[r#""a" = true"#, r#""b" = true"#]);
    let bin = assert_cmd::cargo::cargo_bin("symify");
    let cfg = fx.config.to_string_lossy().into_owned();

    for args in [
        vec!["completions", "fish"],
        vec!["completions", "bash"],
        vec!["man"],
        vec!["status", "-c", &cfg],
        vec!["list", "--entries", "-c", &cfg],
    ] {
        let mut child = std::process::Command::new(&bin)
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // Close the read end before the child gets going, so its writes see EPIPE.
        drop(child.stdout.take());
        let out = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);

        assert_eq!(
            out.status.code(),
            Some(0),
            "{args:?} on a closed pipe: expected exit 0, got {:?} (stderr: {stderr})",
            out.status
        );
        assert!(
            stderr.is_empty(),
            "{args:?} on a closed pipe should print nothing, got: {stderr}"
        );
    }
}

// ----- safety guards ----------------------------------------------------

#[test]
fn add_refuses_directory_outside_live_root() {
    let fx = Fx::new("backup", &[]);
    // A directory that is a sibling of `live`, i.e. outside the mapping's root.
    let outside = fx.live.parent().unwrap().join("outside_dir");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("f"), b"x").unwrap();

    fx.cmd("add").arg(&outside).assert().code(2);
    // The guard fires before any config edit.
    assert!(!fx.config_text().contains("outside_dir"));
    assert!(!is_symlink(&outside));
}

#[test]
fn destructive_replace_is_gated_then_proceeds_with_yes() {
    // sync + conflict=replace where live and store both hold a non-empty, differing
    // directory => the plan recursively deletes the store dir (unrecoverable).
    let fx = Fx::with_body("[mappings.dotfiles.links]\n\"d\" = true\n", "replace");
    fx.write(&fx.lp("d/file"), b"live");
    fx.write(&fx.sp("d/other"), b"store");

    // Non-interactive (piped stdin) + no --yes => refused, nothing executed.
    fx.cmd("sync").assert().code(2);
    assert!(fx.sp("d/other").exists());
    assert!(!is_symlink(&fx.lp("d")));

    // --yes pre-approves the delete: store dir is replaced, live becomes a link.
    fx.cmd("sync").arg("--yes").assert().success();
    assert!(is_symlink(&fx.lp("d")));
    assert_eq!(std::fs::read(fx.sp("d/file")).unwrap(), b"live");
    assert!(!fx.sp("d/other").exists());
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

// ----- copy mode: incremental, checksum ---------------------------------

#[test]
fn sync_copy_touches_only_changed_files() {
    let fx = Fx::copy("backup", &["\"conf\" = true"]);
    fx.write(&fx.lp("conf/a"), b"a");
    fx.write(&fx.lp("conf/b"), b"b");

    // First sync captures the whole dir; second is a clean no-op (idempotent,
    // mtime preserved).
    fx.cmd("sync").assert().success();
    let out = fx.cmd("sync").arg("--json").assert().success();
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.get_output().stdout)).unwrap();
    assert_eq!(doc["summary"]["ok"], 1, "second sync should be all-ok");

    // Change one file; only it is copied.
    let before_a = std::fs::metadata(fx.sp("conf/a"))
        .unwrap()
        .modified()
        .unwrap();
    fx.write(&fx.lp("conf/b"), b"b-bigger");
    let out = fx.cmd("sync").arg("--json").assert().success();
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.get_output().stdout)).unwrap();
    assert_eq!(doc["entries"][0]["copied"], 1, "only one file copied");
    assert_eq!(std::fs::read(fx.sp("conf/b")).unwrap(), b"b-bigger");
    // The unchanged file was not rewritten (its mtime is untouched).
    let after_a = std::fs::metadata(fx.sp("conf/a"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(before_a, after_a, "unchanged file must not be recopied");
}

#[test]
fn sync_checksum_skips_recopy_on_mtime_only_change() {
    let fx = Fx::copy("replace", &["\"f\" = true"]);
    fx.write(&fx.lp("f"), b"content");
    fx.cmd("sync").assert().success();

    // Bump only the live mtime (same content).
    let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
    let h = std::fs::OpenOptions::new()
        .write(true)
        .open(fx.lp("f"))
        .unwrap();
    h.set_times(std::fs::FileTimes::new().set_modified(later))
        .unwrap();

    // Default quick-check sees a difference and re-copies.
    let out = fx.cmd("sync").arg("--json").assert().success();
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.get_output().stdout)).unwrap();
    assert_eq!(doc["summary"]["changed"], 1);

    // Re-bump and use --checksum: content identical → no copy.
    let later2 = std::time::SystemTime::now() + std::time::Duration::from_secs(10);
    let h = std::fs::OpenOptions::new()
        .write(true)
        .open(fx.lp("f"))
        .unwrap();
    h.set_times(std::fs::FileTimes::new().set_modified(later2))
        .unwrap();
    let out = fx
        .cmd("sync")
        .arg("--checksum")
        .arg("--json")
        .assert()
        .success();
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.get_output().stdout)).unwrap();
    assert_eq!(
        doc["summary"]["ok"], 1,
        "checksum recognises identical content"
    );
}

#[test]
fn sync_skip_partial_apply_reports_drift_exit_1() {
    let fx = Fx::copy("skip", &["\"conf\" = true"]);
    fx.write(&fx.lp("conf/a"), b"a");
    fx.cmd("sync").assert().success();

    // One new file (additive) + one diverging file (skip → drift).
    fx.write(&fx.lp("conf/new"), b"new");
    fx.write(&fx.sp("conf/a"), b"store-diverged-bigger");
    let out = fx.cmd("sync").arg("--json").assert().code(1); // drift exit
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.get_output().stdout)).unwrap();
    assert_eq!(doc["entries"][0]["drift"], true);
    assert_eq!(doc["entries"][0]["copied"], 1, "new file still copied");
    assert!(
        fx.sp("conf/new").exists(),
        "additive copy applied under skip"
    );
}

// ----- per-machine mappings (os / host) ---------------------------------

/// A config with the active `dotfiles` mapping (os-gated to every CI OS) plus
/// an `other` mapping host-gated to a hostname that cannot exist here.
fn machine_fx() -> Fx {
    Fx::with_body(
        concat!(
            "[mappings.dotfiles]\n",
            "os = [\"linux\", \"macos\", \"windows\"]\n",
            "[mappings.dotfiles.links]\n",
            "\"a\" = true\n\n",
            "[mappings.other]\n",
            "host = \"no-such-host.invalid\"\n",
            "[mappings.other.links]\n",
            "\"b\" = true\n"
        ),
        "backup",
    )
}

#[test]
fn inactive_mapping_is_noted_and_skipped() {
    let fx = machine_fx();
    fx.write(&fx.lp("a"), b"x");
    fx.write(&fx.lp("b"), b"y");

    // The active mapping applies; the inactive one is a note, not entries, and
    // the mixed run still exits 0.
    let out = fx.cmd("sync").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        stdout.contains("mapping other: inactive (host)"),
        "got: {stdout}"
    );
    assert!(is_symlink(&fx.lp("a")), "active mapping must still adopt");
    assert!(
        !is_symlink(&fx.lp("b")) && fx.lp("b").exists(),
        "inactive mapping must not be touched"
    );

    // status --json: entries only for the active mapping; the inactive one is
    // a {mapping, inactive, reason} object.
    let out = fx.cmd("status").arg("--json").assert().success();
    let doc: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    assert!(
        doc["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["mapping"] == "dotfiles"),
        "got: {doc}"
    );
    assert_eq!(
        doc["inactive_mappings"],
        serde_json::json!([{ "mapping": "other", "inactive": true, "reason": "host" }]),
    );

    // Explicitly selecting the inactive mapping is a clean no-op with the note.
    let out = fx.cmd("status").arg("-m").arg("other").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(
        stdout.contains("mapping other: inactive (host)"),
        "got: {stdout}"
    );

    // list marks the inactive mapping.
    let out = fx.cmd("list").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("inactive (host)"), "got: {stdout}");
}

#[test]
fn add_and_remove_refuse_an_inactive_mapping() {
    let fx = machine_fx();
    fx.write(&fx.lp("c"), b"z");

    let out = fx
        .cmd("add")
        .arg(fx.lp("c"))
        .args(["-m", "other"])
        .assert()
        .code(2);
    let stderr = String::from_utf8_lossy(&out.get_output().stderr).into_owned();
    assert!(
        stderr.contains("inactive on this machine") && stderr.contains("`host`"),
        "got: {stderr}"
    );
    assert!(
        !fx.config_text().contains("\"c\""),
        "refused add must not edit the config"
    );

    fx.cmd("remove")
        .arg(fx.lp("b"))
        .args(["-m", "other"])
        .assert()
        .code(2);
}

// ----- diff --------------------------------------------------------------

#[test]
fn diff_renders_content_and_per_file_states() {
    let fx = Fx::copy("backup", &["\"conf\" = true"]);
    fx.write(&fx.lp("conf/a.txt"), b"line1\nline2\nline3\n");
    fx.write(&fx.sp("conf/a.txt"), b"line1\nCHANGED\nline3\n");
    fx.write(&fx.lp("conf/new.txt"), b"new\n");
    fx.write(&fx.sp("conf/stale.txt"), b"stale\n");
    fx.write(&fx.sp("conf/bin.dat"), b"\x00\x01\x02");
    fx.write(&fx.lp("conf/bin.dat"), b"\x00\x01\x02\x03");

    let out = fx.cmd("diff").assert().code(1); // drift exit…
    fx.cmd("status").assert().code(1); // …matching status on the same tree
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    // Unified diff, store as old (-), live as new (+).
    assert!(stdout.contains("-CHANGED"), "got: {stdout}");
    assert!(stdout.contains("+line2"), "got: {stdout}");
    assert!(stdout.contains("only in live:"), "got: {stdout}");
    assert!(stdout.contains("only in store:"), "got: {stdout}");
    assert!(stdout.contains("binary files differ"), "got: {stdout}");

    // JSON: per-file paths and states, no content hunks.
    let out = fx.cmd("diff").arg("--json").assert().code(1);
    let doc: serde_json::Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
    let files = doc["entries"][0]["files"].as_array().unwrap();
    let states: Vec<&str> = files.iter().map(|f| f["state"].as_str().unwrap()).collect();
    assert!(states.contains(&"differs"), "got: {doc}");
    assert!(states.contains(&"live-only"), "got: {doc}");
    assert!(states.contains(&"store-only"), "got: {doc}");
}

#[test]
fn diff_symlink_mode_states_and_clean_exit() {
    let fx = Fx::new("backup", &["\"a\" = true", "\"b\" = true"]);
    // a: unadopted with content that differs -> a real diff.
    fx.write(&fx.lp("a"), b"live\n");
    fx.write(&fx.sp("a"), b"store\n");
    // b: wrong target.
    fx.write(&fx.sp("b"), b"x");
    std::os::unix::fs::symlink(fx.sp("a"), fx.lp("b")).unwrap();

    let out = fx.cmd("diff").assert().code(1);
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert!(stdout.contains("unadopted"), "got: {stdout}");
    assert!(stdout.contains("-store"), "got: {stdout}");
    assert!(stdout.contains("+live"), "got: {stdout}");
    assert!(stdout.contains("wrong-target"), "got: {stdout}");
    assert!(stdout.contains("expected"), "got: {stdout}");

    // sync adopts a (b is a link — nothing to capture); deploy repairs b's
    // wrong target. Diff then goes silent and exits 0.
    fx.cmd("sync").assert().success();
    fx.cmd("deploy").assert().success();
    let out = fx.cmd("diff").assert().success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    assert_eq!(
        stdout.trim(),
        "diff: 2 ok, 0 drift, 0 failed",
        "got: {stdout}"
    );
}

#[test]
fn diff_is_not_captured_by_the_bare_path_shortcut() {
    // `symify diff` must stay the verb, not become `add diff`.
    let fx = Fx::new("backup", &[]);
    fx.cmd("diff").assert().success();
}

