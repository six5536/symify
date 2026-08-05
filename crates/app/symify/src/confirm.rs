//! Confirmation gate for unrecoverable actions.
//!
//! The only unrecoverable filesystem op symify emits is a recursive delete of a
//! non-empty directory, produced by `conflict = replace`. Before executing a
//! plan that contains one, we require confirmation: an interactive `[y/N]` prompt
//! on a TTY, or `--yes`. Non-interactive runs (piped, `--json`, CI) are refused
//! unless `--yes` is given, so a script can never silently trigger a recursive
//! delete. Everything else (links, single-file adopts, backups) proceeds freely.

use std::io::{BufRead, IsTerminal, Write};
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
        let (kind, ops) = match &p.action {
            Action::Apply { kind, ops } | Action::ApplyDrift { kind, ops } => (kind, ops),
            _ => continue,
        };
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
    out
}

/// Decide whether to proceed with a plan, prompting if it contains an
/// unrecoverable recursive delete. Never prompts under `dry_run`. Wires the real
/// stdin/stderr and TTY detection into the testable [`decide`].
pub fn gate(
    planned: &[Planned],
    yes: bool,
    json: bool,
    dry_run: bool,
) -> symify_core::Result<Gate> {
    let interactive = std::io::stdin().is_terminal();
    let mut input = std::io::stdin().lock();
    let mut prompt = std::io::stderr();
    decide(
        planned,
        yes,
        json,
        dry_run,
        interactive,
        &mut input,
        &mut prompt,
    )
}

/// The confirmation decision, with all IO injected so it is unit-testable: the
/// prompt is written to `prompt`, the answer read from `input`, and `interactive`
/// stands in for "stdin is a TTY". Behaviour is identical to the real `gate`.
fn decide<R: BufRead, W: Write>(
    planned: &[Planned],
    yes: bool,
    json: bool,
    dry_run: bool,
    interactive: bool,
    input: &mut R,
    prompt: &mut W,
) -> symify_core::Result<Gate> {
    if dry_run {
        return Ok(Gate::Proceed);
    }
    let deletes = destructive_deletes(planned);
    if deletes.is_empty() || yes {
        return Ok(Gate::Proceed);
    }

    // Destructive and not pre-approved: a TTY can confirm; anything else refuses.
    if json || !interactive {
        return Err(Error::config(
            "refusing to recursively delete a directory without confirmation; re-run with --yes",
        ));
    }

    let w = |e: std::io::Error| Error::io(Path::new("<stderr>"), e);
    writeln!(
        prompt,
        "The following directories will be permanently deleted:"
    )
    .map_err(w)?;
    for d in &deletes {
        writeln!(prompt, "  {}  ({}/{})", d.path.display(), d.mapping, d.key).map_err(w)?;
    }
    write!(prompt, "Proceed? [y/N] ").map_err(w)?;
    prompt.flush().ok();

    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|e| Error::io(Path::new("<stdin>"), e))?;
    let answer = line.trim().to_ascii_lowercase();
    if answer == "y" || answer == "yes" {
        Ok(Gate::Proceed)
    } else {
        Ok(Gate::Aborted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symify_core::model::{Conflict, Mode};

    /// A planned entry that removes a path via the given action kind.
    fn planned_remove(kind: ActionKind, path: &str) -> Planned {
        Planned {
            mapping: "dots".into(),
            key: "dir".into(),
            live: "/live/dir".into(),
            store: "/store/dir".into(),
            mode: Mode::Copy,
            conflict: Conflict::Replace,
            action: Action::Apply {
                kind,
                ops: vec![FsOp::Remove(PathBuf::from(path))],
            },
        }
    }

    fn run(
        planned: &[Planned],
        yes: bool,
        json: bool,
        dry_run: bool,
        interactive: bool,
        answer: &str,
    ) -> (symify_core::Result<Gate>, String) {
        let mut input = answer.as_bytes();
        let mut prompt = Vec::new();
        let res = decide(
            planned,
            yes,
            json,
            dry_run,
            interactive,
            &mut input,
            &mut prompt,
        );
        (res, String::from_utf8(prompt).unwrap())
    }

    fn is_proceed(r: &symify_core::Result<Gate>) -> bool {
        matches!(r, Ok(Gate::Proceed))
    }
    fn is_aborted(r: &symify_core::Result<Gate>) -> bool {
        matches!(r, Ok(Gate::Aborted))
    }

    #[test]
    fn no_destructive_ops_proceeds_without_prompt() {
        // A plain copy carries no recursive delete.
        let p = vec![Planned {
            action: Action::Apply {
                kind: ActionKind::Push,
                ops: vec![FsOp::Copy {
                    from: "/live/f".into(),
                    to: "/store/f".into(),
                }],
            },
            ..planned_remove(ActionKind::Push, "/x")
        }];
        let (r, out) = run(&p, false, false, false, true, "");
        assert!(is_proceed(&r));
        assert!(out.is_empty(), "must not prompt when nothing destructive");
    }

    #[test]
    fn dry_run_proceeds_even_with_destructive_ops() {
        // Use a real nonempty dir so destructive_deletes would otherwise fire.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Push,
            dir.path().to_str().unwrap(),
        )];
        let (r, out) = run(&p, false, false, true, true, "");
        assert!(is_proceed(&r));
        assert!(out.is_empty());
    }

    #[test]
    fn yes_flag_skips_prompt() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Push,
            dir.path().to_str().unwrap(),
        )];
        let (r, out) = run(&p, true, false, false, true, "");
        assert!(is_proceed(&r));
        assert!(out.is_empty());
    }

    #[test]
    fn relink_delete_is_not_destructive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Relink,
            dir.path().to_str().unwrap(),
        )];
        let (r, out) = run(&p, false, false, false, true, "");
        assert!(is_proceed(&r));
        assert!(out.is_empty());
    }

    #[test]
    fn non_interactive_refuses() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Push,
            dir.path().to_str().unwrap(),
        )];
        let (r, _) = run(&p, false, false, false, false, "");
        assert!(matches!(r, Err(Error::Config(_))));
    }

    #[test]
    fn json_refuses_even_on_a_tty() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Push,
            dir.path().to_str().unwrap(),
        )];
        let (r, _) = run(&p, false, true, false, true, "y");
        assert!(matches!(r, Err(Error::Config(_))));
    }

    #[test]
    fn interactive_yes_proceeds_and_prompt_lists_the_delete() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let target = dir.path().to_str().unwrap();
        let p = vec![planned_remove(ActionKind::Push, target)];
        for ans in ["y\n", "yes\n", "Y\n", "YES\n"] {
            let (r, out) = run(&p, false, false, false, true, ans);
            assert!(is_proceed(&r), "answer {ans:?} should proceed");
            assert!(out.contains("permanently deleted"));
            assert!(out.contains(target), "prompt should name the directory");
            assert!(out.contains("dots/dir"), "prompt should name mapping/key");
            assert!(out.ends_with("Proceed? [y/N] "));
        }
    }

    #[test]
    fn interactive_decline_and_eof_abort() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("f"), b"x").unwrap();
        let p = vec![planned_remove(
            ActionKind::Push,
            dir.path().to_str().unwrap(),
        )];
        for ans in ["n\n", "no\n", "\n", "", "garbage\n"] {
            let (r, _) = run(&p, false, false, false, true, ans);
            assert!(is_aborted(&r), "answer {ans:?} should abort");
        }
    }
}
