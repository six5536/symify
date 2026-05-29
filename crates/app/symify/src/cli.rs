//! Command-line interface (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Keep your files in sync with a backing repository — as symlinks, hardlinks, or copies.
#[derive(Debug, Parser)]
#[command(name = "symify", version, about, long_about = None)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Move your live files into the store and replace them with links.
    Sync(RunArgs),
    /// Set up your live location from the store, linking back to it.
    Deploy(RunArgs),
    /// Show what each file will do, without changing anything.
    Status(StatusArgs),
}

/// Shared arguments for the commands that change files.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Read config from these files instead of the usual locations. Repeatable.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Preview the changes without touching any files.
    #[arg(long)]
    pub dry_run: bool,

    /// Print JSON for scripts instead of human-friendly output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the read-only `status` command.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Read config from these files instead of the usual locations. Repeatable.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Print JSON for scripts instead of human-friendly output.
    #[arg(long)]
    pub json: bool,
}
