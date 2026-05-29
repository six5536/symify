//! symify CLI entry point.

mod cli;
mod output;

use std::process::ExitCode;

use clap::Parser;
use symify_core::clock::SystemClock;
use symify_core::{Verb, config, execute, plan, status};

use crate::cli::{Cli, Command, RunArgs, StatusArgs};

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
    }
}

fn run_verb(verb: Verb, args: RunArgs) -> symify_core::Result<u8> {
    let cfg = config::load_config(&args.config)?;
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

fn run_status(args: StatusArgs) -> symify_core::Result<u8> {
    let cfg = config::load_config(&args.config)?;
    let entries = status(&cfg)?;
    Ok(output::render_status(&entries, args.json))
}
