//! symify CLI entry point.

mod cli;
mod output;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use symify_core::clock::SystemClock;
use symify_core::config::{ResolvedConfig, ResolvedMapping};
use symify_core::model::{LinkValue, Mode};
use symify_core::{Error, FsOp, Verb, config, edit, entry_paths, execute, fs, plan, status};

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
    match Cli::parse().command {
        Command::Sync(args) => run_verb(Verb::Sync, args),
        Command::Deploy(args) => run_verb(Verb::Deploy, args),
        Command::Status(args) => run_status(args),
        Command::Add(args) => run_add(args),
        Command::Remove(args) => run_remove(args),
        Command::List(args) => run_list(args),
    }
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
    let planned = plan(&cfg, verb)?;
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
    Ok(output::render_status(&status(&cfg)?, args.json))
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
    let planned = plan(&single, Verb::Sync)?;

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
        Mode::Hardlink => fs::same_inode(s, d)?,
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
