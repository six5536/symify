//! symify CLI entry point.

mod cli;
mod confirm;
mod output;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use symify_core::clock::SystemClock;
use symify_core::config::{ResolvedConfig, ResolvedMapping};
use symify_core::model::{LinkValue, Mode};
use symify_core::{
    Action, Error, FsOp, RunOptions, Verb, config, edit, entry_paths, execute, fs, plan, status,
};

use crate::cli::{AddArgs, Cli, Command, ListArgs, QueryArgs, RemoveArgs, RunArgs};

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
        // Bare version number — easier to consume from scripts than `name x.y.z`.
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(output::EXIT_OK);
    }
    let Some(command) = cli.command else {
        Cli::command().print_help().ok();
        return Ok(output::EXIT_OK);
    };
    if is_mutating(&command) && is_root() && !cli.allow_root {
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
    }
}

/// Whether a command mutates the filesystem or config (so it should be refused
/// under root unless `--allow-root`). `status`/`list` are read-only.
fn is_mutating(command: &Command) -> bool {
    matches!(
        command,
        Command::Sync(_) | Command::Deploy(_) | Command::Add(_) | Command::Remove(_)
    )
}

/// True when the process is running with an effective uid of 0 (root). We declare
/// `geteuid` directly rather than depend on `libc`; it is always linked on Unix.
#[cfg(unix)]
fn is_root() -> bool {
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() == 0 }
}

/// Off Unix there is no euid concept here; never refuse. (Windows admin detection
/// is a future-milestone concern.)
#[cfg(not(unix))]
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
    let mut cfg = config::select(resolved, &args.mapping)?;
    // `--delete` is a per-run config override: force mirror on for the selected
    // mappings before planning, so the planner reads only ResolvedMapping.mirror.
    if args.delete {
        for m in &mut cfg.mappings {
            m.mirror = true;
        }
    }
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
    Ok(output::render_run(
        verb,
        args.dry_run,
        &planned,
        &outcomes,
        args.json,
    ))
}

fn run_status(args: QueryArgs) -> symify_core::Result<u8> {
    let (_, resolved) = load_set(&args.config)?;
    let cfg = config::select(resolved, &args.mapping)?;
    let opts = RunOptions {
        checksum: args.checksum,
        modify_window: args.modify_window,
    };
    Ok(output::render_status(&status(&cfg, opts)?, args.json))
}

fn run_list(args: ListArgs) -> symify_core::Result<u8> {
    let (_, resolved) = load_set(&args.config)?;
    let cfg = config::select(resolved, &args.mapping)?;
    Ok(output::render_list(&cfg, args.entries, args.json))
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
        return Ok(output::render_add(
            &mapping_name,
            &key,
            None,
            "would-add",
            &outcomes[0],
            true,
            args.json,
        ));
    }

    let report = edit::add_link(&files, &primary, &mapping_name, &key, value, args.force)?;
    let outcomes = execute(&planned, &SystemClock, false);
    let status = match report.status {
        edit::AddStatus::Added => "added",
        edit::AddStatus::Replaced => "replaced",
        edit::AddStatus::Unchanged => "unchanged",
    };
    Ok(output::render_add(
        &mapping_name,
        &key,
        Some(&report.file),
        status,
        &outcomes[0],
        false,
        args.json,
    ))
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
        return Ok(output::render_remove(
            &mapping_name,
            &key,
            &[],
            restored,
            true,
            args.json,
        ));
    }

    if restored {
        do_restore(&s, &d)?;
    }
    let edited = edit::remove_link(&files, &mapping_name, &key)?;
    Ok(output::render_remove(
        &mapping_name,
        &key,
        &edited,
        restored,
        false,
        args.json,
    ))
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
fn derive_key(abs: &Path, live: &Path) -> String {
    match abs.strip_prefix(live) {
        Ok(rel) => rel.to_string_lossy().into_owned(),
        Err(_) => abs.to_string_lossy().into_owned(),
    }
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
