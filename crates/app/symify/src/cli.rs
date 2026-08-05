//! Command-line interface (clap derive).

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// Keep your files in sync with a backing repository — as symlinks or copies.
#[derive(Debug, Parser)]
// `version` feeds the man page header and `--help`; the built-in `-V` flag stays
// disabled because we render the version ourselves (see `main::run`).
#[command(name = "symify", about, long_about = None, version, disable_version_flag = true)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Allow mutating commands to run as root (refused by default).
    #[arg(long, global = true)]
    pub allow_root: bool,

    /// Print the version and exit.
    #[arg(short = 'V', long = "version", global = true)]
    pub version: bool,
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
    /// Print a shell completion script to stdout.
    Completions(CompletionsArgs),
    /// Print a roff man page to stdout. Hidden: it exists for packaging, not
    /// day-to-day use, and is generated into the release archives.
    #[command(hide = true)]
    Man,
}

/// Arguments for `completions`.
#[derive(Debug, Args)]
pub struct CompletionsArgs {
    /// Shell to generate a completion script for.
    #[arg(value_enum)]
    pub shell: clap_complete::Shell,
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

    /// Skip the confirmation prompt for unrecoverable recursive deletes.
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// In `copy` mode, compare file content exactly instead of by size+mtime.
    #[arg(long)]
    pub checksum: bool,

    /// In `copy` mode, treat mtimes within this many seconds as equal (for
    /// coarse-granularity filesystems). Default 0 (exact).
    #[arg(long, value_name = "SECONDS", default_value_t = 0)]
    pub modify_window: u64,

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

    /// In `copy` mode, compare file content exactly instead of by size+mtime.
    #[arg(long)]
    pub checksum: bool,

    /// In `copy` mode, treat mtimes within this many seconds as equal (for
    /// coarse-granularity filesystems). Default 0 (exact).
    #[arg(long, value_name = "SECONDS", default_value_t = 0)]
    pub modify_window: u64,

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

    /// Skip the confirmation prompt for unrecoverable recursive deletes.
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Emit machine-readable JSON instead of human output.
    #[arg(long)]
    pub json: bool,
}

/// Subcommand names and aliases that the bare-path shortcut must not shadow.
const SUBCOMMANDS: &[&str] = &[
    "sync",
    "deploy",
    "status",
    "add",
    "remove",
    "rm",
    "list",
    "ls",
    "help",
    "completions",
    "man",
];

/// Make `symify <PATH> …` an alias for `symify add <PATH> …`, since adding is the
/// common case. Bare `symify` (or only global flags like `-V`/`--help`) is left
/// untouched, so clap still prints help/version.
///
/// We insert `add` before the first non-flag token, unless it is already a known
/// subcommand. A file literally named like a subcommand (e.g. `status`) is still
/// read as that subcommand — use `symify add status` to disambiguate. A bare `--`
/// disables the shortcut, so `symify -- <PATH>` is left as-is.
pub fn normalize_args<I, T>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = T>,
    T: Into<String>,
{
    let mut args: Vec<String> = args.into_iter().map(Into::into).collect();
    // The command slot is the first token that isn't a leading global flag. Every
    // top-level flag (`--allow-root`, `-V`/`--version`) takes no value, so the
    // first non-`-` token (before any `--`) is where a subcommand would go.
    let mut slot = None;
    for (i, a) in args.iter().enumerate().skip(1) {
        if a == "--" {
            break;
        }
        if !a.starts_with('-') {
            slot = Some(i);
            break;
        }
    }
    if let Some(i) = slot
        && !SUBCOMMANDS.contains(&args[i].as_str())
    {
        args.insert(i, "add".to_string());
    }
    args
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

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(args: &[&str]) -> Vec<String> {
        normalize_args(std::iter::once("symify").chain(args.iter().copied()))
    }

    #[test]
    fn bare_path_becomes_add() {
        assert_eq!(norm(&["~/.bashrc"]), ["symify", "add", "~/.bashrc"]);
    }

    #[test]
    fn path_keeps_trailing_flags() {
        assert_eq!(
            norm(&["~/.bashrc", "-m", "dots", "--dry-run"]),
            ["symify", "add", "~/.bashrc", "-m", "dots", "--dry-run"]
        );
    }

    #[test]
    fn global_flag_before_path_still_adds() {
        assert_eq!(
            norm(&["--allow-root", "~/.bashrc"]),
            ["symify", "--allow-root", "add", "~/.bashrc"]
        );
    }

    #[test]
    fn known_subcommands_and_aliases_untouched() {
        for cmd in [
            "sync",
            "deploy",
            "status",
            "add",
            "remove",
            "rm",
            "list",
            "ls",
            // Without these in SUBCOMMANDS the bare-path shortcut would rewrite
            // `symify completions bash` into `symify add completions bash`.
            "completions",
            "man",
        ] {
            assert_eq!(norm(&[cmd]), ["symify", cmd]);
        }
    }

    #[test]
    fn completions_shell_argument_is_not_shadowed() {
        assert_eq!(
            norm(&["completions", "bash"]),
            ["symify", "completions", "bash"]
        );
    }

    #[test]
    fn bare_and_flag_only_invocations_untouched() {
        assert_eq!(norm(&[]), ["symify"]);
        assert_eq!(norm(&["-V"]), ["symify", "-V"]);
        assert_eq!(norm(&["--help"]), ["symify", "--help"]);
    }

    #[test]
    fn double_dash_disables_shortcut() {
        assert_eq!(norm(&["--", "weird"]), ["symify", "--", "weird"]);
    }
}
