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
    /// Capture live files into the store (live → store), adopting as needed.
    Sync(RunArgs),
    /// Set up your live location from the store, linking back to it.
    Deploy(RunArgs),
    /// Show what each file will do, without changing anything.
    Status(QueryArgs),
    /// Start tracking a file: add it to a mapping and adopt it.
    Add(AddArgs),
    /// Stop tracking a file, restoring a standalone copy by default.
    #[command(visible_alias = "rm")]
    Remove(RemoveArgs),
    /// List the mappings and where they point.
    #[command(visible_alias = "ls")]
    List(ListArgs),
}

/// Shared arguments for the mutating run verbs (`sync`/`deploy`).
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Limit to these mappings; repeatable. Omitted = all mappings.
    #[arg(short = 'm', long = "mapping", value_name = "MAPPING")]
    pub mapping: Vec<String>,

    /// Plan and report without making any changes.
    #[arg(long)]
    pub dry_run: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for the read-only `status` verb.
#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Limit to these mappings; repeatable. Omitted = all mappings.
    #[arg(short = 'm', long = "mapping", value_name = "MAPPING")]
    pub mapping: Vec<String>,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `list`.
#[derive(Debug, Args)]
pub struct ListArgs {
    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Limit to these mappings; repeatable. Omitted = all mappings.
    #[arg(short = 'm', long = "mapping", value_name = "MAPPING")]
    pub mapping: Vec<String>,

    /// Also list each mapping's entries (live → store).
    #[arg(long)]
    pub entries: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `add`.
#[derive(Debug, Args)]
pub struct AddArgs {
    /// The existing file (or directory) to track.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Mapping to add to; defaults to the sole mapping. Created if it doesn't exist.
    #[arg(short = 'm', long = "mapping", value_name = "MAPPING")]
    pub mapping: Option<String>,

    /// Explicit store-side path (relative to store, or absolute). Default: mirror.
    #[arg(long, value_name = "PATH")]
    pub store_path: Option<String>,

    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Overwrite an existing entry whose value differs.
    #[arg(long)]
    pub force: bool,

    /// Preview the config edit and adoption without changing anything.
    #[arg(long)]
    pub dry_run: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Arguments for `remove`.
#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// The tracked file (or directory) to stop tracking.
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Mapping to remove from; defaults to the sole mapping.
    #[arg(short = 'm', long = "mapping", value_name = "MAPPING")]
    pub mapping: Option<String>,

    /// Config file(s); repeatable. When given, replaces default discovery.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Vec<PathBuf>,

    /// Only edit config; leave the existing link/copy in place.
    #[arg(long)]
    pub no_restore: bool,

    /// Preview the config edit and restore without changing anything.
    #[arg(long)]
    pub dry_run: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}
