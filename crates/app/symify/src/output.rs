//! Rendering of run and status results — one data model, two renderers (human
//! and `--json`) — plus the exit-code policy.
//!
//! Exit codes: `0` clean / success, `1` drift (unresolved conflicts, or any
//! out-of-sync entry for `status`), `2` one or more failures.

use serde::Serialize;
use symify_core::model::Mode;
use symify_core::status::{StatusEntry, StatusLabel};
use symify_core::{ActionKind, Outcome, Planned, Verb};

/// Exit code: success / clean.
pub const EXIT_OK: u8 = 0;
/// Exit code: drift detected.
pub const EXIT_DRIFT: u8 = 1;
/// Exit code: one or more failures.
pub const EXIT_FAILURE: u8 = 2;

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Symlink => "symlink",
        Mode::Hardlink => "hardlink",
        Mode::Sync => "sync",
    }
}

fn verb_str(verb: Verb) -> &'static str {
    match verb {
        Verb::Sync => "sync",
        Verb::Deploy => "deploy",
    }
}

fn action_word(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::Adopt => "adopt",
        ActionKind::Relink => "relink",
        ActionKind::Link => "link",
        ActionKind::Push => "push",
        ActionKind::Pull => "pull",
    }
}

// ----- run (sync / deploy) ----------------------------------------------

#[derive(Serialize)]
struct RunJson<'a> {
    verb: &'a str,
    dry_run: bool,
    entries: Vec<RunEntryJson>,
    summary: RunSummary,
}

#[derive(Serialize)]
struct RunEntryJson {
    mapping: String,
    key: String,
    live: String,
    store: String,
    mode: &'static str,
    outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Default, Serialize)]
struct RunSummary {
    changed: usize,
    ok: usize,
    skipped: usize,
    disabled: usize,
    conflicts: usize,
    failed: usize,
}

/// Render the result of a `sync`/`deploy` run; returns the process exit code.
pub fn render_run(
    verb: Verb,
    dry_run: bool,
    planned: &[Planned],
    outcomes: &[Outcome],
    json: bool,
) -> u8 {
    let mut summary = RunSummary::default();
    let mut entries = Vec::with_capacity(planned.len());

    for (p, o) in planned.iter().zip(outcomes) {
        let (outcome, action, detail) = classify(o, &mut summary);
        entries.push(RunEntryJson {
            mapping: p.mapping.clone(),
            key: p.key.clone(),
            live: p.live.display().to_string(),
            store: p.store.display().to_string(),
            mode: mode_str(p.mode),
            outcome,
            action,
            detail,
        });
    }

    let (failed, conflicts) = (summary.failed, summary.conflicts);

    if json {
        let doc = RunJson {
            verb: verb_str(verb),
            dry_run,
            entries,
            summary,
        };
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    } else {
        if dry_run {
            println!("dry run — no changes applied\n");
        }
        for e in &entries {
            print_line(e);
        }
        print_run_summary(verb, dry_run, &summary);
    }

    if failed > 0 {
        EXIT_FAILURE
    } else if conflicts > 0 {
        EXIT_DRIFT
    } else {
        EXIT_OK
    }
}

fn classify(
    o: &Outcome,
    s: &mut RunSummary,
) -> (&'static str, Option<&'static str>, Option<String>) {
    match o {
        Outcome::Applied(kind) => {
            s.changed += 1;
            ("applied", Some(action_word(*kind)), None)
        }
        Outcome::AlreadyOk => {
            s.ok += 1;
            ("ok", None, None)
        }
        Outcome::Disabled => {
            s.disabled += 1;
            ("disabled", None, None)
        }
        Outcome::Skipped(reason) => {
            s.skipped += 1;
            ("skipped", None, Some(reason.to_string()))
        }
        Outcome::Conflict => {
            s.conflicts += 1;
            ("conflict", None, None)
        }
        Outcome::Failed(msg) => {
            s.failed += 1;
            ("failed", None, Some(msg.clone()))
        }
    }
}

fn symbol(outcome: &str) -> char {
    match outcome {
        "applied" => '+',
        "ok" => '=',
        "disabled" => '·',
        "skipped" => '-',
        "conflict" => '!',
        "failed" => 'x',
        _ => '?',
    }
}

fn print_line(e: &RunEntryJson) {
    let label = e.action.unwrap_or(e.outcome);
    let detail = match &e.detail {
        Some(d) => format!(" ({d})"),
        None => String::new(),
    };
    println!(
        "  {} {:<8} {}/{}{}",
        symbol(e.outcome),
        label,
        e.mapping,
        e.key,
        detail
    );
}

fn print_run_summary(verb: Verb, dry_run: bool, s: &RunSummary) {
    let verbed = if dry_run { "would change" } else { "changed" };
    println!(
        "\n{}: {} {}, {} ok, {} skipped, {} disabled, {} conflicts, {} failed",
        verb_str(verb),
        s.changed,
        verbed,
        s.ok,
        s.skipped,
        s.disabled,
        s.conflicts,
        s.failed
    );
}

// ----- status -----------------------------------------------------------

#[derive(Serialize)]
struct StatusJson {
    entries: Vec<StatusEntryJson>,
    summary: StatusSummary,
}

#[derive(Serialize)]
struct StatusEntryJson {
    mapping: String,
    key: String,
    live: String,
    store: String,
    mode: &'static str,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

#[derive(Default, Serialize)]
struct StatusSummary {
    clean: usize,
    drift: usize,
    failed: usize,
}

fn status_str(label: &StatusLabel) -> &'static str {
    match label {
        StatusLabel::Disabled => "disabled",
        StatusLabel::Ok => "ok",
        StatusLabel::Unadopted => "unadopted",
        StatusLabel::WrongTarget => "wrong-target",
        StatusLabel::LiveMissing => "live-missing",
        StatusLabel::StoreMissing => "store-missing",
        StatusLabel::Missing => "missing",
        StatusLabel::Differs => "differs",
        StatusLabel::Failed(_) => "failed",
    }
}

/// Render `status` results; returns the process exit code.
pub fn render_status(entries: &[StatusEntry], json: bool) -> u8 {
    let mut summary = StatusSummary::default();
    let mut out = Vec::with_capacity(entries.len());

    for e in entries {
        if e.label.is_failure() {
            summary.failed += 1;
        } else if e.label.is_clean() {
            summary.clean += 1;
        } else {
            summary.drift += 1;
        }
        let detail = match &e.label {
            StatusLabel::Failed(m) => Some(m.clone()),
            _ => None,
        };
        out.push(StatusEntryJson {
            mapping: e.mapping.clone(),
            key: e.key.clone(),
            live: e.live.display().to_string(),
            store: e.store.display().to_string(),
            mode: mode_str(e.mode),
            status: status_str(&e.label),
            detail,
        });
    }

    let (failed, drift) = (summary.failed, summary.drift);

    if json {
        let doc = StatusJson {
            entries: out,
            summary,
        };
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    } else {
        for e in &out {
            let detail = match &e.detail {
                Some(d) => format!(" ({d})"),
                None => String::new(),
            };
            println!("  {:<13} {}/{}{}", e.status, e.mapping, e.key, detail);
        }
        println!(
            "\nstatus: {} ok, {} drift, {} failed",
            summary.clean, drift, failed
        );
    }

    if failed > 0 {
        EXIT_FAILURE
    } else if drift > 0 {
        EXIT_DRIFT
    } else {
        EXIT_OK
    }
}
