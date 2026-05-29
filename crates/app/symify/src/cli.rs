//! Command-line interface (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// symify — keep a working location in sync with a managed backing repository.
#[derive(Debug, Parser)]
#[command(name = "symify", version, about, long_about = None)]
pub struct Cli {
    /// The verb to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level verbs.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Capture live files into the store (live → store), adopting as needed.
    Sync(RunArgs),
    /// Install the store into the live location (store → live).
    Deploy(RunArgs),
    /// Report per-entry status (read-only).
    Status(StatusArgs),
}

/// Shared arguments for the mutating verbs.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Plan and report without making any changes.
    #[arg(long)]
    pub dry_run: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the read-only `status` verb.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}
