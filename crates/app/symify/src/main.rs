//! symify CLI entry point.
// Under the nightly coverage job (cargo-llvm-cov sets `coverage_nightly`), enable
// the attribute used to exclude genuinely untestable glue from coverage. Inert on
// the stable toolchain used for normal builds and tests.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
#![warn(missing_docs)]

mod cli;
mod confirm;
mod output;

use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use symify_core::clock::SystemClock;
use symify_core::config::{ResolvedConfig, ResolvedMapping};
use symify_core::model::{LinkValue, Mode};
use symify_core::{
    Action, Error, FsOp, RunOptions, Verb, config, edit, entry_paths, execute, fs, plan, status,
};

use crate::cli::{
    AddArgs, Cli, Command, CompletionsArgs, ListArgs, QueryArgs, RemoveArgs, RunArgs,
};

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(output::EXIT_FAILURE)
        }
    }
}

fn run() -> symify_core::Result<u8> {
    let cli = Cli::parse_from(cli::normalize_args(std::env::args()));
    if cli.version {
        // `name x.y.z`, the near-universal CLI convention, so pasted output is
        // self-identifying in bug reports.
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(output::EXIT_OK);
    }
    let Some(command) = cli.command else {
        Cli::command().print_help().ok();
        return Ok(output::EXIT_OK);
    };
    if root_refused(is_mutating(&command), is_root(), cli.allow_root) {
        return Err(Error::config(
            "refusing to run as root; re-run with --allow-root if you really mean it",
        ));
    }
    match command {
        Command::Sync(args) => run_verb(Verb::Sync, args),
        Command::Deploy(args) => run_verb(Verb::Deploy, args),
        Command::Status(args) => run_status(args),
        Command::Add(args) => run_add(args),
        Command::Remove(args) => run_remove(args),
        Command::List(args) => run_list(args),
        Command::Completions(args) => run_completions(args),
        Command::Man => run_man(),
    }
}

/// Write a shell completion script for `shell` to stdout.
fn run_completions(args: CompletionsArgs) -> symify_core::Result<u8> {
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(args.shell, &mut cmd, name, &mut io::stdout());
    Ok(output::EXIT_OK)
}

/// Write a roff man page to stdout, for packaging into release archives.
fn run_man() -> symify_core::Result<u8> {
    clap_mangen::Man::new(Cli::command())
        .render(&mut io::stdout())
        .map_err(stdout_err)?;
    Ok(output::EXIT_OK)
}

/// Whether a command mutates the filesystem or config (so it should be refused
/// under root unless `--allow-root`). `status`/`list` are read-only.
fn is_mutating(command: &Command) -> bool {
    matches!(
        command,
        Command::Sync(_) | Command::Deploy(_) | Command::Add(_) | Command::Remove(_)
    )
}

/// Whether a mutating command should be refused: we're root and `--allow-root`
/// was not given. Kept as a pure decision, separate from the `geteuid` syscall,
/// so it is unit-testable without actually being root.
fn root_refused(mutating: bool, root: bool, allow_root: bool) -> bool {
    mutating && root && !allow_root
}

/// Map a stdout write failure into a `symify` error so render helpers can use `?`.
fn stdout_err(e: io::Error) -> Error {
    Error::io(Path::new("<stdout>"), e)
}

/// True when the process is running with an effective uid of 0 (root). We declare
/// `geteuid` directly rather than depend on `libc`; it is always linked on Unix.
/// The raw syscall wrapper is excluded from coverage: it can't be exercised
/// without actually running as root. The pure decision lives in `root_refused`.
#[cfg(unix)]
#[cfg_attr(coverage_nightly, coverage(off))]
fn is_root() -> bool {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() == 0 }
}

/// Off Unix there is no euid concept here; never refuse. (Windows admin detection
/// is a future-milestone concern.)
#[cfg(not(unix))]
#[cfg_attr(coverage_nightly, coverage(off))]
fn is_root() -> bool {
    false
}

/// Discover (auto-initing a default config if needed), then load + resolve.
/// Returns the config file set and the resolved config.
fn load_set(config: &[PathBuf]) -> symify_core::Result<(Vec<PathBuf>, ResolvedConfig)> {
    let discovered = config::ensure_config(config)?;
    if let Some(created) = &discovered.created {
        // To stderr so it never pollutes `--json` output on stdout.
        eprintln!("Created {} (defaults).", created.display());
    }
    let resolved = config::resolve(config::load(&discovered.paths)?)?;
    Ok((discovered.paths, resolved))
}

fn run_verb(verb: Verb, args: RunArgs) -> symify_core::Result<u8> {
    let (_, resolved) = load_set(&args.config)?;
    let cfg = config::select(resolved, &args.mapping)?;
    let opts = RunOptions {
        checksum: args.checksum,
        modify_window: args.modify_window,
    };
    let planned = plan(&cfg, verb, opts)?;
    if let confirm::Gate::Aborted = confirm::gate(&planned, args.yes, args.json, args.dry_run)? {
        eprintln!("Aborted.");
        return Ok(output::EXIT_OK);
    }
    let outcomes = execute(&planned, &SystemClock, args.dry_run);
    output::render_run(
        &mut io::stdout().lock(),
        verb,
        args.dry_run,
        &planned,
        &outcomes,
        args.json,
    )
    .map_err(stdout_err)
}

fn run_status(args: QueryArgs) -> symify_core::Result<u8> {
    let (_, resolved) = load_set(&args.config)?;
    let cfg = config::select(resolved, &args.mapping)?;
    let opts = RunOptions {
        checksum: args.checksum,
        modify_window: args.modify_window,
    };
    output::render_status(&mut io::stdout().lock(), &status(&cfg, opts)?, args.json)
        .map_err(stdout_err)
}

fn run_list(args: ListArgs) -> symify_core::Result<u8> {
    let (_, resolved) = load_set(&args.config)?;
    let cfg = config::select(resolved, &args.mapping)?;
    output::render_list(&mut io::stdout().lock(), &cfg, args.entries, args.json).map_err(stdout_err)
}

fn run_add(args: AddArgs) -> symify_core::Result<u8> {
    let (files, resolved) = load_set(&args.config)?;
    let primary = files
        .first()
        .cloned()
        .ok_or_else(|| Error::config("no config file to edit"))?;
    let raw = config::load(&files)?;

    let mapping_name = match &args.mapping {
        Some(n) => n.clone(),
        None => sole_mapping(&resolved)?,
    };

    // The mapping's live root (existing mapping, or [settings] for a new one).
    let live = match resolved.mappings.iter().find(|m| m.name == mapping_name) {
        Some(m) => m.live.clone(),
        None => {
            let s = raw
                .settings
                .as_ref()
                .and_then(|s| s.live.clone())
                .ok_or_else(|| {
                    Error::config(format!(
                        "cannot create mapping `{mapping_name}`: [settings].live is not set"
                    ))
                })?;
            config::expand_root(&s)?
        }
    };

    let abs = config::expand_root(&args.path.to_string_lossy())?;
    if abs.symlink_metadata().is_err() {
        return Err(Error::config(format!("no such file: {}", abs.display())));
    }
    let key = derive_key(&abs, &live);
    let value = match &args.store_path {
        Some(p) => LinkValue::String(p.clone()),
        None => LinkValue::Boolean(true),
    };

    // Build the would-be config in memory to plan the single-entry adoption.
    let mut raw2 = raw.clone();
    raw2.mappings
        .entry(mapping_name.clone())
        .or_default()
        .links
        .insert(key.clone(), value.clone());
    let resolved2 = config::resolve(raw2)?;
    let m2 = resolved2
        .mappings
        .iter()
        .find(|m| m.name == mapping_name)
        .expect("mapping present after insert");
    let single = ResolvedConfig {
        mappings: vec![ResolvedMapping {
            links: m2
                .links
                .iter()
                .filter(|(k, _)| *k == key)
                .cloned()
                .collect(),
            ..m2.clone()
        }],
    };
    let planned = plan(&single, Verb::Sync, RunOptions::default())?;

    // A guard failure (protected root, out-of-root directory, …) must abort
    // before we touch the config — don't record a link we'll refuse to adopt.
    if let Action::Failed(msg) = &planned[0].action {
        return Err(Error::config(msg.clone()));
    }
    if let confirm::Gate::Aborted = confirm::gate(&planned, args.yes, args.json, args.dry_run)? {
        eprintln!("Aborted.");
        return Ok(output::EXIT_OK);
    }

    if args.dry_run {
        let outcomes = execute(&planned, &SystemClock, true);
        return output::render_add(
            &mut io::stdout().lock(),
            &mapping_name,
            &key,
            None,
            "would-add",
            &outcomes[0],
            true,
            args.json,
        )
        .map_err(stdout_err);
    }

    let report = edit::add_link(&files, &primary, &mapping_name, &key, value, args.force)?;
    let outcomes = execute(&planned, &SystemClock, false);
    let status = match report.status {
        edit::AddStatus::Added => "added",
        edit::AddStatus::Replaced => "replaced",
        edit::AddStatus::Unchanged => "unchanged",
    };
    output::render_add(
        &mut io::stdout().lock(),
        &mapping_name,
        &key,
        Some(&report.file),
        status,
        &outcomes[0],
        false,
        args.json,
    )
    .map_err(stdout_err)
}

fn run_remove(args: RemoveArgs) -> symify_core::Result<u8> {
    let (files, resolved) = load_set(&args.config)?;
    let mapping_name = match &args.mapping {
        Some(n) => n.clone(),
        None => sole_mapping(&resolved)?,
    };
    let m = resolved
        .mappings
        .iter()
        .find(|m| m.name == mapping_name)
        .ok_or_else(|| Error::config(format!("unknown mapping `{mapping_name}`")))?;

    let abs = config::expand_root(&args.path.to_string_lossy())?;
    let key = derive_key(&abs, &m.live);
    let entry =
        m.links.iter().find(|(k, _)| *k == key).ok_or_else(|| {
            Error::config(format!("no entry `{key}` in mapping `{mapping_name}`"))
        })?;

    let (s, d) = entry_paths(m, &entry.0, &entry.1);
    let restored = !args.no_restore && would_restore(&s, &d, m.mode)?;

    if args.dry_run {
        return output::render_remove(
            &mut io::stdout().lock(),
            &mapping_name,
            &key,
            &[],
            restored,
            true,
            args.json,
        )
        .map_err(stdout_err);
    }

    if restored {
        do_restore(&s, &d)?;
    }
    let edited = edit::remove_link(&files, &mapping_name, &key)?;
    output::render_remove(
        &mut io::stdout().lock(),
        &mapping_name,
        &key,
        &edited,
        restored,
        false,
        args.json,
    )
    .map_err(stdout_err)
}

// ----- helpers -----------------------------------------------------------

fn sole_mapping(resolved: &ResolvedConfig) -> symify_core::Result<String> {
    match resolved.mappings.as_slice() {
        [one] => Ok(one.name.clone()),
        [] => Err(Error::config("no mappings; pass -m <name>")),
        _ => Err(Error::config("multiple mappings; pass -m <name>")),
    }
}

/// Relativize an absolute path against the mapping's live root; fall back to an
/// absolute key when the path is outside it.
///
/// A plain prefix match is not enough. A relative argument is made absolute
/// against the process CWD, which the OS reports already resolved through any
/// symlinks, while the live root comes from config text and is not. So the two
/// can spell the same directory differently — on macOS always, since `/var` is a
/// symlink to `/private/var` and temp dirs live under it. When the textual match
/// fails we retry on canonical forms before giving up and writing an absolute
/// key.
fn derive_key(abs: &Path, live: &Path) -> String {
    if let Ok(rel) = abs.strip_prefix(live) {
        return rel.to_string_lossy().into_owned();
    }
    if let Some(canonical) = canonical_parent(abs)
        && let Ok(live) = live.canonicalize()
        && let Ok(rel) = canonical.strip_prefix(&live)
    {
        return rel.to_string_lossy().into_owned();
    }
    abs.to_string_lossy().into_owned()
}

/// `abs` with its parent directory canonicalized, keeping the final component
/// as written. Only the parent is resolved: canonicalizing the whole path would
/// follow the entry itself when it is a symlink, which is not what the key
/// should name. `None` when the parent does not exist or cannot be resolved.
fn canonical_parent(abs: &Path) -> Option<PathBuf> {
    let name = abs.file_name()?;
    Some(abs.parent()?.canonicalize().ok()?.join(name))
}

/// Whether `remove --restore` would replace a managed link at `live` with a
/// standalone copy (true only if `live` is currently linked to an existing `store`).
fn would_restore(s: &Path, d: &Path, mode: Mode) -> symify_core::Result<bool> {
    if fs::inspect(d)?.is_missing() {
        return Ok(false);
    }
    Ok(match mode {
        Mode::Symlink => fs::symlink_points_to(s, d)?,
        Mode::Sync => false,
    })
}

fn do_restore(s: &Path, d: &Path) -> symify_core::Result<()> {
    fs::apply_op(&FsOp::Remove(s.to_path_buf()), &SystemClock)?;
    fs::apply_op(
        &FsOp::Copy {
            from: d.to_path_buf(),
            to: s.to_path_buf(),
        },
        &SystemClock,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use symify_core::model::Conflict;

    fn cmd(args: &[&str]) -> Command {
        Cli::try_parse_from(args).unwrap().command.unwrap()
    }

    #[test]
    fn root_refused_truth_table() {
        // Only refuse when mutating AND root AND not allow_root.
        assert!(root_refused(true, true, false));
        assert!(!root_refused(true, true, true)); // --allow-root overrides
        assert!(!root_refused(true, false, false)); // not root
        assert!(!root_refused(false, true, false)); // read-only verb
        // Exhaustive: only the (T,T,F) corner is true.
        for m in [false, true] {
            for r in [false, true] {
                for a in [false, true] {
                    assert_eq!(root_refused(m, r, a), m && r && !a);
                }
            }
        }
    }

    #[test]
    fn is_mutating_classifies_every_verb() {
        assert!(is_mutating(&cmd(&["symify", "sync"])));
        assert!(is_mutating(&cmd(&["symify", "deploy"])));
        assert!(is_mutating(&cmd(&["symify", "add", "/tmp/x"])));
        assert!(is_mutating(&cmd(&["symify", "remove", "/tmp/x"])));
        assert!(!is_mutating(&cmd(&["symify", "status"])));
        assert!(!is_mutating(&cmd(&["symify", "list"])));
    }

    #[test]
    fn derive_key_relativizes_in_root_else_absolute() {
        let live = Path::new("/home/user");
        // In-root path becomes a relative key.
        assert_eq!(derive_key(Path::new("/home/user/.bashrc"), live), ".bashrc");
        assert_eq!(
            derive_key(Path::new("/home/user/.config/nvim"), live),
            ".config/nvim"
        );
        // Outside the live root keeps an absolute key.
        assert_eq!(derive_key(Path::new("/etc/hosts"), live), "/etc/hosts");
    }

    #[cfg(unix)]
    #[test]
    fn derive_key_relativizes_through_a_symlinked_live_root() {
        // The resolved path and the configured root spell the same directory
        // differently (macOS /var -> /private/var). The textual prefix match
        // fails; the canonical retry must still produce a relative key.
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        let link = tmp.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        assert_eq!(derive_key(&real.join(".bashrc"), &link), ".bashrc");
        // Genuinely outside the root still falls back to an absolute key.
        let outside = tmp.path().join("elsewhere.txt");
        assert_eq!(
            derive_key(&outside, &link),
            outside.to_string_lossy().into_owned()
        );
    }

    fn mapping(name: &str) -> ResolvedMapping {
        ResolvedMapping {
            name: name.into(),
            live: "/live".into(),
            store: "/store".into(),
            mode: Mode::Symlink,
            conflict: Conflict::Backup,
            links: vec![],
        }
    }

    #[test]
    fn would_restore_and_do_restore_replace_link_with_copy() {
        // entry_paths yields (s, d) = (live, store); mirror these names here.
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("store_f");
        let live = dir.path().join("live_f");
        std::fs::write(&store, b"content").unwrap();
        std::os::unix::fs::symlink(&store, &live).unwrap();

        // Symlink mode: a live link pointing at store would be restored.
        assert!(would_restore(&live, &store, Mode::Symlink).unwrap());
        // Sync mode keeps independent copies — never "restores".
        assert!(!would_restore(&live, &store, Mode::Sync).unwrap());
        // A missing store side short-circuits to false.
        let missing_store = dir.path().join("gone");
        assert!(!would_restore(&live, &missing_store, Mode::Symlink).unwrap());

        do_restore(&live, &store).unwrap();
        // Live is now a standalone copy; the store content is left in place.
        assert!(
            std::fs::symlink_metadata(&live)
                .unwrap()
                .file_type()
                .is_file()
        );
        assert_eq!(std::fs::read(&live).unwrap(), b"content");
        assert!(store.exists(), "do_restore must not remove the store side");
    }

    #[test]
    fn sole_mapping_defaults_and_errors() {
        // Exactly one -> its name.
        let one = ResolvedConfig {
            mappings: vec![mapping("dots")],
        };
        assert_eq!(sole_mapping(&one).unwrap(), "dots");

        // None -> error asking for -m.
        let none = ResolvedConfig { mappings: vec![] };
        assert!(matches!(sole_mapping(&none), Err(Error::Config(_))));

        // Many -> error asking for -m.
        let many = ResolvedConfig {
            mappings: vec![mapping("a"), mapping("b")],
        };
        assert!(matches!(sole_mapping(&many), Err(Error::Config(_))));
    }
}
