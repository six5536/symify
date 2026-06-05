//! Confirmation gate for unrecoverable actions.
//!
//! The only unrecoverable filesystem op symify emits is a recursive delete of a
//! non-empty directory, produced by `conflict = replace`. Before executing a
//! plan that contains one, we require confirmation: an interactive `[y/N]` prompt
//! on a TTY, or `--yes`. Non-interactive runs (piped, `--json`, CI) are refused
//! unless `--yes` is given, so a script can never silently trigger a recursive
//! delete. Everything else (links, single-file adopts, backups) proceeds freely.

use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

use symify_core::{Action, ActionKind, Error, FsOp, Planned, fs};

/// Whether the caller should proceed with execution.
pub enum Gate {
    /// Proceed (no destructive delete, confirmed, or `--yes`/dry-run).
    Proceed,
    /// The user declined the prompt.
    Aborted,
}

/// A pending unrecoverable delete, for the confirmation summary.
struct PendingDelete {
    path: PathBuf,
    mapping: String,
    key: String,
}

/// Scan a plan for unrecoverable recursive directory deletes. A `relink` removes
/// content that is byte-identical to the other side, so it is safe and excluded;
/// only `replace`-style removes of a non-empty directory count.
fn destructive_deletes(planned: &[Planned]) -> Vec<PendingDelete> {
    let mut out = Vec::new();
    for p in planned {
        if let Action::Apply { kind, ops } = &p.action {
            if *kind == ActionKind::Relink {
                continue;
            }
            for op in ops {
                if let FsOp::Remove(path) = op
                    && fs::is_nonempty_dir(path)
                {
                    out.push(PendingDelete {
                        path: path.clone(),
                        mapping: p.mapping.clone(),
                        key: p.key.clone(),
                    });
                }
            }
        }
    }
    out
}

/// Decide whether to proceed with a plan, prompting if it contains an
/// unrecoverable recursive delete. Never prompts under `dry_run`.
pub fn gate(
    planned: &[Planned],
    yes: bool,
    json: bool,
    dry_run: bool,
) -> symify_core::Result<Gate> {
    if dry_run {
        return Ok(Gate::Proceed);
    }
    let deletes = destructive_deletes(planned);
    if deletes.is_empty() || yes {
        return Ok(Gate::Proceed);
    }

    // Destructive and not pre-approved: a TTY can confirm; anything else refuses.
    if json || !std::io::stdin().is_terminal() {
        return Err(Error::config(
            "refusing to recursively delete a directory without confirmation; re-run with --yes",
        ));
    }

    eprintln!("The following directories will be permanently deleted:");
    for d in &deletes {
        eprintln!("  {}  ({}/{})", d.path.display(), d.mapping, d.key);
    }
    eprint!("Proceed? [y/N] ");
    std::io::stderr().flush().ok();

    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| Error::io(Path::new("<stdin>"), e))?;
    let answer = line.trim().to_ascii_lowercase();
    if answer == "y" || answer == "yes" {
        Ok(Gate::Proceed)
    } else {
        Ok(Gate::Aborted)
    }
}
