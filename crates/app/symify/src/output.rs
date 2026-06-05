//! Rendering of run and status results — one data model, two renderers (human
//! and `--json`) — plus the exit-code policy.
//!
//! Exit codes: `0` clean / success, `1` drift (unresolved conflicts, or any
//! out-of-sync entry for `status`), `2` one or more failures.

use std::path::{Path, PathBuf};

use serde::Serialize;
use symify_core::config::ResolvedConfig;
use symify_core::model::Mode;
use symify_core::status::{StatusEntry, StatusLabel};
use symify_core::{Action, ActionKind, FsOp, Outcome, Planned, Verb, entry_paths};

/// Exit code: success / clean.
pub const EXIT_OK: u8 = 0;
/// Exit code: drift detected.
pub const EXIT_DRIFT: u8 = 1;
/// Exit code: one or more failures.
pub const EXIT_FAILURE: u8 = 2;

fn mode_str(mode: Mode) -> &'static str {
    match mode {
        Mode::Symlink => "symlink",
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
    /// Files copied (`sync` mode).
    copied: usize,
    /// Files backed up before overwrite/prune.
    backed_up: usize,
    /// Files/dirs removed (overwrite or mirror prune).
    pruned: usize,
    /// A residual `skip`-difference remains after applying.
    drift: bool,
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
        let (outcome, action, mut detail) = classify(o, &mut summary);
        let (copied, backed_up, pruned) = count_ops(&p.action);
        let drift = matches!(o, Outcome::AppliedDrift(_));
        // For applied entries, surface the per-file counts (and prune visibility)
        // instead of a bare action word.
        if matches!(o, Outcome::Applied(_) | Outcome::AppliedDrift(_)) {
            detail = count_detail(copied, backed_up, pruned, drift);
        }
        entries.push(RunEntryJson {
            mapping: p.mapping.clone(),
            key: p.key.clone(),
            live: p.live.display().to_string(),
            store: p.store.display().to_string(),
            mode: mode_str(p.mode),
            outcome,
            action,
            detail,
            copied,
            backed_up,
            pruned,
            drift,
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
        Outcome::AppliedDrift(kind) => {
            // Both a change and drift: counts toward changed and conflicts so the
            // run exits with the drift code while still reporting the work done.
            s.changed += 1;
            s.conflicts += 1;
            ("applied-drift", Some(action_word(*kind)), None)
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
        "applied-drift" => '!',
        "ok" => '=',
        "disabled" => '·',
        "skipped" => '-',
        "conflict" => '!',
        "failed" => 'x',
        _ => '?',
    }
}

/// Tally an entry's ops into (copied, backed_up, pruned) for reporting.
fn count_ops(action: &Action) -> (usize, usize, usize) {
    let ops = match action {
        Action::Apply { ops, .. } | Action::ApplyDrift { ops, .. } => ops.as_slice(),
        _ => &[][..],
    };
    let (mut copied, mut backed_up, mut pruned) = (0, 0, 0);
    for op in ops {
        match op {
            FsOp::Copy { .. } => copied += 1,
            FsOp::Backup(_) => backed_up += 1,
            FsOp::Remove(_) => pruned += 1,
            _ => {}
        }
    }
    (copied, backed_up, pruned)
}

/// Build the human detail string for an applied entry, e.g. `+2 ~1 -3, drift`.
/// Returns `None` when there is nothing noteworthy (e.g. a plain link adopt).
fn count_detail(copied: usize, backed_up: usize, pruned: usize, drift: bool) -> Option<String> {
    let mut parts = Vec::new();
    if copied > 0 {
        parts.push(format!("+{copied}"));
    }
    if backed_up > 0 {
        parts.push(format!("~{backed_up}"));
    }
    if pruned > 0 {
        parts.push(format!("-{pruned}"));
    }
    let mut detail = parts.join(" ");
    if drift {
        if !detail.is_empty() {
            detail.push_str(", ");
        }
        detail.push_str("drift");
    }
    if detail.is_empty() {
        None
    } else {
        Some(detail)
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

// ----- add / remove / list ----------------------------------------------

/// Map a single adopt/restore outcome to (symbol, word) and an exit code.
fn outcome_word(o: &Outcome) -> &'static str {
    match o {
        Outcome::Applied(k) | Outcome::AppliedDrift(k) => action_word(*k),
        Outcome::AlreadyOk => "ok",
        Outcome::Disabled => "disabled",
        Outcome::Skipped(_) => "skipped",
        Outcome::Conflict => "conflict",
        Outcome::Failed(_) => "failed",
    }
}

fn outcome_exit(o: &Outcome) -> u8 {
    match o {
        Outcome::Failed(_) => EXIT_FAILURE,
        Outcome::Conflict | Outcome::AppliedDrift(_) => EXIT_DRIFT,
        _ => EXIT_OK,
    }
}

#[derive(Serialize)]
struct AddJson<'a> {
    action: &'static str,
    mapping: &'a str,
    key: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    status: &'a str,
    adopt: &'static str,
    dry_run: bool,
}

/// Render the result of `add`; returns the exit code.
#[allow(clippy::too_many_arguments)]
pub fn render_add(
    mapping: &str,
    key: &str,
    file: Option<&Path>,
    status: &str,
    adopt: &Outcome,
    dry_run: bool,
    json: bool,
) -> u8 {
    if json {
        let doc = AddJson {
            action: "add",
            mapping,
            key,
            file: file.map(|p| p.display().to_string()),
            status,
            adopt: outcome_word(adopt),
            dry_run,
        };
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    } else {
        let prefix = if dry_run { "would add" } else { "added" };
        match file {
            Some(f) => println!("{prefix} {key} to {mapping} ({})", f.display()),
            None => println!("{prefix} {key} to {mapping}"),
        }
        if status == "unchanged" {
            println!("  (already present)");
        }
        let done = if dry_run { "would adopt" } else { "adopted" };
        match adopt {
            Outcome::Applied(_) => println!("  {done}"),
            Outcome::AppliedDrift(_) => println!("  {done} (drift remains — resolve, then sync)"),
            Outcome::AlreadyOk => println!("  already in sync"),
            Outcome::Conflict => println!("  conflict — not adopted (resolve, then sync)"),
            Outcome::Failed(m) => println!("  failed to adopt: {m}"),
            Outcome::Skipped(r) => println!("  not adopted: {r}"),
            Outcome::Disabled => {}
        }
    }
    outcome_exit(adopt)
}

#[derive(Serialize)]
struct RemoveJson<'a> {
    action: &'static str,
    mapping: &'a str,
    key: &'a str,
    files: Vec<String>,
    restored: bool,
    dry_run: bool,
}

/// Render the result of `remove`; returns the exit code.
pub fn render_remove(
    mapping: &str,
    key: &str,
    files: &[PathBuf],
    restored: bool,
    dry_run: bool,
    json: bool,
) -> u8 {
    if json {
        let doc = RemoveJson {
            action: "remove",
            mapping,
            key,
            files: files.iter().map(|p| p.display().to_string()).collect(),
            restored,
            dry_run,
        };
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    } else {
        let prefix = if dry_run { "would remove" } else { "removed" };
        if files.is_empty() {
            println!("{prefix} {key} from {mapping}");
        } else {
            let list: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
            println!("{prefix} {key} from {mapping} ({})", list.join(", "));
        }
        if restored {
            let r = if dry_run { "would restore" } else { "restored" };
            println!("  {r}: standalone copy at the live path");
        }
    }
    EXIT_OK
}

#[derive(Serialize)]
struct ListJson {
    mappings: Vec<ListMappingJson>,
}

#[derive(Serialize)]
struct ListMappingJson {
    name: String,
    live: String,
    store: String,
    mode: &'static str,
    conflict: &'static str,
    entries: Vec<ListEntryJson>,
}

#[derive(Serialize)]
struct ListEntryJson {
    key: String,
    live: String,
    store: String,
}

fn conflict_str(c: symify_core::model::Conflict) -> &'static str {
    use symify_core::model::Conflict;
    match c {
        Conflict::Skip => "skip",
        Conflict::Replace => "replace",
        Conflict::Backup => "backup",
    }
}

/// Render `list`; returns the exit code.
pub fn render_list(config: &ResolvedConfig, entries: bool, json: bool) -> u8 {
    if json {
        let mappings = config
            .mappings
            .iter()
            .map(|m| ListMappingJson {
                name: m.name.clone(),
                live: m.live.display().to_string(),
                store: m.store.display().to_string(),
                mode: mode_str(m.mode),
                conflict: conflict_str(m.conflict),
                entries: m
                    .links
                    .iter()
                    .map(|(k, v)| {
                        let (live, store) = entry_paths(m, k, v);
                        ListEntryJson {
                            key: k.clone(),
                            live: live.display().to_string(),
                            store: store.display().to_string(),
                        }
                    })
                    .collect(),
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&ListJson { mappings }).unwrap()
        );
        return EXIT_OK;
    }

    if config.mappings.is_empty() {
        println!("no mappings configured");
        return EXIT_OK;
    }
    for m in &config.mappings {
        println!(
            "{}  {} → {}  {}  {}  ({} entries)",
            m.name,
            m.live.display(),
            m.store.display(),
            mode_str(m.mode),
            conflict_str(m.conflict),
            m.links.len()
        );
        if entries {
            for (k, v) in &m.links {
                let (live, store) = entry_paths(m, k, v);
                println!("  {}  {} → {}", k, live.display(), store.display());
            }
        }
    }
    EXIT_OK
}
