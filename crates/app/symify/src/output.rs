//! Rendering of run and status results — one data model, two renderers (human
//! and `--json`) — plus the exit-code policy.
//!
//! Exit codes: `0` clean / success, `1` drift (unresolved conflicts, or any
//! out-of-sync entry for `status`), `2` one or more failures.

use std::io::{self, Write};
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
        Mode::Copy => "copy",
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

// ----- inactive mappings ------------------------------------------------

/// A mapping skipped because its `os`/`host` condition does not match this
/// machine. Rendered as one line (or one JSON object), never per entry.
#[derive(Serialize)]
pub struct InactiveNote {
    mapping: String,
    inactive: bool,
    reason: &'static str,
}

/// The inactive mappings of a resolved config, ready to render.
pub fn inactive_notes(config: &ResolvedConfig) -> Vec<InactiveNote> {
    config
        .mappings
        .iter()
        .filter_map(|m| {
            m.inactive.map(|r| InactiveNote {
                mapping: m.name.clone(),
                inactive: true,
                reason: r.key(),
            })
        })
        .collect()
}

fn print_inactive<W: Write>(w: &mut W, notes: &[InactiveNote]) -> io::Result<()> {
    for n in notes {
        writeln!(w, "mapping {}: inactive ({})", n.mapping, n.reason)?;
    }
    Ok(())
}

// ----- run (sync / deploy) ----------------------------------------------

#[derive(Serialize)]
struct RunJson<'a> {
    verb: &'a str,
    dry_run: bool,
    entries: Vec<RunEntryJson>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    inactive_mappings: &'a [InactiveNote],
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
    /// Files copied (`copy` mode).
    copied: usize,
    /// Files backed up before an overwrite.
    backed_up: usize,
    /// Files/dirs removed (overwrite or relink).
    removed: usize,
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
pub fn render_run<W: Write>(
    w: &mut W,
    verb: Verb,
    dry_run: bool,
    planned: &[Planned],
    outcomes: &[Outcome],
    inactive: &[InactiveNote],
    json: bool,
) -> io::Result<u8> {
    let mut summary = RunSummary::default();
    let mut entries = Vec::with_capacity(planned.len());

    for (p, o) in planned.iter().zip(outcomes) {
        let (outcome, action, mut detail) = classify(o, &mut summary);
        let (copied, backed_up, removed) = count_ops(&p.action);
        let drift = matches!(o, Outcome::AppliedDrift(_));
        // For applied entries, surface the per-file counts (and removal visibility)
        // instead of a bare action word.
        if matches!(o, Outcome::Applied(_) | Outcome::AppliedDrift(_)) {
            detail = count_detail(copied, backed_up, removed, drift);
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
            removed,
            drift,
        });
    }

    let (failed, conflicts) = (summary.failed, summary.conflicts);

    if json {
        let doc = RunJson {
            verb: verb_str(verb),
            dry_run,
            entries,
            inactive_mappings: inactive,
            summary,
        };
        writeln!(w, "{}", serde_json::to_string_pretty(&doc).unwrap())?;
    } else {
        if dry_run {
            writeln!(w, "dry run — no changes applied\n")?;
        }
        for e in &entries {
            print_line(w, e)?;
        }
        print_inactive(w, inactive)?;
        print_run_summary(w, verb, dry_run, &summary)?;
    }

    Ok(if failed > 0 {
        EXIT_FAILURE
    } else if conflicts > 0 {
        EXIT_DRIFT
    } else {
        EXIT_OK
    })
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

/// Tally an entry's ops into (copied, backed_up, removed) for reporting.
fn count_ops(action: &Action) -> (usize, usize, usize) {
    let ops = match action {
        Action::Apply { ops, .. } | Action::ApplyDrift { ops, .. } => ops.as_slice(),
        _ => &[][..],
    };
    let (mut copied, mut backed_up, mut removed) = (0, 0, 0);
    for op in ops {
        match op {
            FsOp::Copy { .. } => copied += 1,
            FsOp::Backup(_) => backed_up += 1,
            FsOp::Remove(_) => removed += 1,
            _ => {}
        }
    }
    (copied, backed_up, removed)
}

/// Build the human detail string for an applied entry, e.g. `+2 ~1 -3, drift`.
/// Returns `None` when there is nothing noteworthy (e.g. a plain link adopt).
fn count_detail(copied: usize, backed_up: usize, removed: usize, drift: bool) -> Option<String> {
    let mut parts = Vec::new();
    if copied > 0 {
        parts.push(format!("+{copied}"));
    }
    if backed_up > 0 {
        parts.push(format!("~{backed_up}"));
    }
    if removed > 0 {
        parts.push(format!("-{removed}"));
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

fn print_line<W: Write>(w: &mut W, e: &RunEntryJson) -> io::Result<()> {
    let label = e.action.unwrap_or(e.outcome);
    let detail = match &e.detail {
        Some(d) => format!(" ({d})"),
        None => String::new(),
    };
    writeln!(
        w,
        "  {} {:<8} {}/{}{}",
        symbol(e.outcome),
        label,
        e.mapping,
        e.key,
        detail
    )
}

fn print_run_summary<W: Write>(
    w: &mut W,
    verb: Verb,
    dry_run: bool,
    s: &RunSummary,
) -> io::Result<()> {
    let verbed = if dry_run { "would change" } else { "changed" };
    writeln!(
        w,
        "\n{}: {} {}, {} ok, {} skipped, {} disabled, {} conflicts, {} failed",
        verb_str(verb),
        s.changed,
        verbed,
        s.ok,
        s.skipped,
        s.disabled,
        s.conflicts,
        s.failed
    )
}

// ----- status -----------------------------------------------------------

#[derive(Serialize)]
struct StatusJson<'a> {
    entries: Vec<StatusEntryJson>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    inactive_mappings: &'a [InactiveNote],
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
pub fn render_status<W: Write>(
    w: &mut W,
    entries: &[StatusEntry],
    inactive: &[InactiveNote],
    json: bool,
) -> io::Result<u8> {
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
            inactive_mappings: inactive,
            summary,
        };
        writeln!(w, "{}", serde_json::to_string_pretty(&doc).unwrap())?;
    } else {
        for e in &out {
            let detail = match &e.detail {
                Some(d) => format!(" ({d})"),
                None => String::new(),
            };
            writeln!(w, "  {:<13} {}/{}{}", e.status, e.mapping, e.key, detail)?;
        }
        print_inactive(w, inactive)?;
        writeln!(
            w,
            "\nstatus: {} ok, {} drift, {} failed",
            summary.clean, drift, failed
        )?;
    }

    Ok(if failed > 0 {
        EXIT_FAILURE
    } else if drift > 0 {
        EXIT_DRIFT
    } else {
        EXIT_OK
    })
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
pub fn render_add<W: Write>(
    w: &mut W,
    mapping: &str,
    key: &str,
    file: Option<&Path>,
    status: &str,
    adopt: &Outcome,
    dry_run: bool,
    json: bool,
) -> io::Result<u8> {
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
        writeln!(w, "{}", serde_json::to_string_pretty(&doc).unwrap())?;
    } else {
        let prefix = if dry_run { "would add" } else { "added" };
        match file {
            Some(f) => writeln!(w, "{prefix} {key} to {mapping} ({})", f.display())?,
            None => writeln!(w, "{prefix} {key} to {mapping}")?,
        }
        if status == "unchanged" {
            writeln!(w, "  (already present)")?;
        }
        let done = if dry_run { "would adopt" } else { "adopted" };
        match adopt {
            Outcome::Applied(_) => writeln!(w, "  {done}")?,
            Outcome::AppliedDrift(_) => {
                writeln!(w, "  {done} (drift remains — resolve, then sync)")?
            }
            Outcome::AlreadyOk => writeln!(w, "  already in sync")?,
            Outcome::Conflict => writeln!(w, "  conflict — not adopted (resolve, then sync)")?,
            Outcome::Failed(m) => writeln!(w, "  failed to adopt: {m}")?,
            Outcome::Skipped(r) => writeln!(w, "  not adopted: {r}")?,
            Outcome::Disabled => {}
        }
    }
    Ok(outcome_exit(adopt))
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
pub fn render_remove<W: Write>(
    w: &mut W,
    mapping: &str,
    key: &str,
    files: &[PathBuf],
    restored: bool,
    dry_run: bool,
    json: bool,
) -> io::Result<u8> {
    if json {
        let doc = RemoveJson {
            action: "remove",
            mapping,
            key,
            files: files.iter().map(|p| p.display().to_string()).collect(),
            restored,
            dry_run,
        };
        writeln!(w, "{}", serde_json::to_string_pretty(&doc).unwrap())?;
    } else {
        let prefix = if dry_run { "would remove" } else { "removed" };
        if files.is_empty() {
            writeln!(w, "{prefix} {key} from {mapping}")?;
        } else {
            let list: Vec<String> = files.iter().map(|p| p.display().to_string()).collect();
            writeln!(w, "{prefix} {key} from {mapping} ({})", list.join(", "))?;
        }
        if restored {
            let r = if dry_run { "would restore" } else { "restored" };
            writeln!(w, "  {r}: standalone copy at the live path")?;
        }
    }
    Ok(EXIT_OK)
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
    /// The unmatched condition (`os`/`host`) when the mapping is inactive here.
    #[serde(skip_serializing_if = "Option::is_none")]
    inactive: Option<&'static str>,
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
pub fn render_list<W: Write>(
    w: &mut W,
    config: &ResolvedConfig,
    entries: bool,
    json: bool,
) -> io::Result<u8> {
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
                inactive: m.inactive.map(|r| r.key()),
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
        writeln!(
            w,
            "{}",
            serde_json::to_string_pretty(&ListJson { mappings }).unwrap()
        )?;
        return Ok(EXIT_OK);
    }

    if config.mappings.is_empty() {
        writeln!(w, "no mappings configured")?;
        return Ok(EXIT_OK);
    }
    for m in &config.mappings {
        let inactive = match m.inactive {
            Some(r) => format!("  inactive ({})", r.key()),
            None => String::new(),
        };
        writeln!(
            w,
            "{}  {} → {}  {}  {}  ({} entries){}",
            m.name,
            m.live.display(),
            m.store.display(),
            mode_str(m.mode),
            conflict_str(m.conflict),
            m.links.len(),
            inactive
        )?;
        if entries {
            for (k, v) in &m.links {
                let (live, store) = entry_paths(m, k, v);
                writeln!(w, "  {}  {} → {}", k, live.display(), store.display())?;
            }
        }
    }
    Ok(EXIT_OK)
}

#[cfg(test)]
mod tests {
    //! Fixture-driven tests: the renderers are pure over in-memory data with
    //! fixed fake paths, so human output is snapshot-stable (no tempdirs, no
    //! timestamps, no redaction) and JSON is parsed and asserted by field.
    use super::*;
    use serde_json::Value;
    use symify_core::config::ResolvedMapping;
    use symify_core::model::{Conflict, LinkValue};

    fn copy(key: &str) -> FsOp {
        FsOp::Copy {
            from: PathBuf::from(format!("/live/{key}")),
            to: PathBuf::from(format!("/store/{key}")),
        }
    }

    fn planned(key: &str, mode: Mode, action: Action) -> Planned {
        Planned {
            mapping: "dots".into(),
            key: key.into(),
            live: PathBuf::from(format!("/live/{key}")),
            store: PathBuf::from(format!("/store/{key}")),
            mode,
            conflict: Conflict::Backup,
            action,
        }
    }

    /// One entry per outcome, paired so `action` matches `outcome` as the
    /// executor would produce. Exercises every `ActionKind` and `Outcome`.
    fn run_fixture() -> (Vec<Planned>, Vec<Outcome>) {
        use ActionKind::*;
        let p = vec![
            planned(
                "adopt",
                Mode::Symlink,
                Action::Apply {
                    kind: Adopt,
                    ops: vec![FsOp::Symlink {
                        link: "/live/adopt".into(),
                        target: "/store/adopt".into(),
                    }],
                },
            ),
            planned(
                "relink",
                Mode::Symlink,
                Action::Apply {
                    kind: Relink,
                    ops: vec![],
                },
            ),
            planned(
                "link",
                Mode::Symlink,
                Action::Apply {
                    kind: Link,
                    ops: vec![],
                },
            ),
            planned(
                "push",
                Mode::Copy,
                Action::Apply {
                    kind: Push,
                    ops: vec![copy("push"), FsOp::Backup("/store/push".into())],
                },
            ),
            planned(
                "pull",
                Mode::Copy,
                Action::Apply {
                    kind: Pull,
                    ops: vec![copy("pull")],
                },
            ),
            planned(
                "drifted",
                Mode::Copy,
                Action::ApplyDrift {
                    kind: Push,
                    ops: vec![copy("drifted"), FsOp::Remove("/store/old".into())],
                },
            ),
            planned("clean", Mode::Symlink, Action::AlreadyOk),
            planned("skipped", Mode::Copy, Action::Skip("nothing to do")),
            planned("conflicted", Mode::Copy, Action::Conflict),
            planned("off", Mode::Symlink, Action::Disabled),
            planned("broken", Mode::Symlink, Action::Failed("boom".into())),
        ];
        let o = vec![
            Outcome::Applied(Adopt),
            Outcome::Applied(Relink),
            Outcome::Applied(Link),
            Outcome::Applied(Push),
            Outcome::Applied(Pull),
            Outcome::AppliedDrift(Push),
            Outcome::AlreadyOk,
            Outcome::Skipped("nothing to do"),
            Outcome::Conflict,
            Outcome::Disabled,
            Outcome::Failed("boom".into()),
        ];
        (p, o)
    }

    fn render_to_string(json: bool, dry_run: bool) -> (String, u8) {
        let (p, o) = run_fixture();
        let mut buf = Vec::new();
        let code = render_run(&mut buf, Verb::Sync, dry_run, &p, &o, &[], json).unwrap();
        (String::from_utf8(buf).unwrap(), code)
    }

    #[test]
    fn run_human_every_outcome() {
        let (s, code) = render_to_string(false, false);
        // The fixture has a failure (and conflicts) -> failure code wins.
        assert_eq!(code, EXIT_FAILURE);
        insta::assert_snapshot!("run_human_every_outcome", s);
    }

    #[test]
    fn run_human_dry_run_annotates() {
        let (s, _) = render_to_string(false, true);
        assert!(s.starts_with("dry run — no changes applied\n"));
        assert!(s.contains("would change"));
        insta::assert_snapshot!("run_human_dry_run", s);
    }

    #[test]
    fn run_json_shape_and_counters() {
        let (s, code) = render_to_string(true, false);
        assert_eq!(code, EXIT_FAILURE);
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["verb"], "sync");
        assert_eq!(v["dry_run"], false);
        let entries = v["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 11);

        // push: one copy + one backup, no removal.
        let push = entries.iter().find(|e| e["key"] == "push").unwrap();
        assert_eq!(push["outcome"], "applied");
        assert_eq!(push["action"], "push");
        assert_eq!(push["copied"], 1);
        assert_eq!(push["backed_up"], 1);
        assert_eq!(push["removed"], 0);
        assert_eq!(push["drift"], false);
        assert_eq!(push["mode"], "copy");

        // drifted: applied-drift carries copied + removed and drift=true.
        let drifted = entries.iter().find(|e| e["key"] == "drifted").unwrap();
        assert_eq!(drifted["outcome"], "applied-drift");
        assert_eq!(drifted["copied"], 1);
        assert_eq!(drifted["removed"], 1);
        assert_eq!(drifted["drift"], true);

        // Summary counts: 5 plain applied + 1 applied-drift = 6 changed; the
        // applied-drift and the conflict both count as conflicts (= 2).
        let sum = &v["summary"];
        assert_eq!(sum["changed"], 6);
        assert_eq!(sum["ok"], 1);
        assert_eq!(sum["skipped"], 1);
        assert_eq!(sum["disabled"], 1);
        assert_eq!(sum["conflicts"], 2);
        assert_eq!(sum["failed"], 1);
    }

    #[test]
    fn run_json_failure_outranks_drift_in_exit() {
        let p = vec![planned("x", Mode::Copy, Action::Failed("nope".into()))];
        let o = vec![Outcome::Failed("nope".into())];
        let mut buf = Vec::new();
        let code = render_run(&mut buf, Verb::Deploy, false, &p, &o, &[], true).unwrap();
        assert_eq!(code, EXIT_FAILURE);
        let v: Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(v["verb"], "deploy");
        assert_eq!(v["entries"][0]["detail"], "nope");
    }

    // ----- status --------------------------------------------------------

    fn status_fixture() -> Vec<StatusEntry> {
        let mk = |key: &str, mode: Mode, label: StatusLabel| StatusEntry {
            mapping: "dots".into(),
            key: key.into(),
            live: PathBuf::from(format!("/live/{key}")),
            store: PathBuf::from(format!("/store/{key}")),
            mode,
            label,
        };
        vec![
            mk("ok", Mode::Symlink, StatusLabel::Ok),
            mk("off", Mode::Symlink, StatusLabel::Disabled),
            mk("unadopted", Mode::Symlink, StatusLabel::Unadopted),
            mk("wrong", Mode::Symlink, StatusLabel::WrongTarget),
            mk("live-missing", Mode::Symlink, StatusLabel::LiveMissing),
            mk("store-missing", Mode::Copy, StatusLabel::StoreMissing),
            mk("missing", Mode::Symlink, StatusLabel::Missing),
            mk("differs", Mode::Copy, StatusLabel::Differs),
            mk(
                "broken",
                Mode::Symlink,
                StatusLabel::Failed("guarded".into()),
            ),
        ]
    }

    #[test]
    fn status_human_every_label() {
        let mut buf = Vec::new();
        let code = render_status(&mut buf, &status_fixture(), &[], false).unwrap();
        // Has drift and a failure -> failure code wins.
        assert_eq!(code, EXIT_FAILURE);
        insta::assert_snapshot!("status_human_every_label", String::from_utf8(buf).unwrap());
    }

    #[test]
    fn status_json_summary_and_detail() {
        let mut buf = Vec::new();
        render_status(&mut buf, &status_fixture(), &[], true).unwrap();
        let v: Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        let sum = &v["summary"];
        // Ok + Disabled are clean (2); failed (1); the rest are drift (6).
        assert_eq!(sum["clean"], 2);
        assert_eq!(sum["drift"], 6);
        assert_eq!(sum["failed"], 1);
        let broken = v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["key"] == "broken")
            .unwrap();
        assert_eq!(broken["status"], "failed");
        assert_eq!(broken["detail"], "guarded");
    }

    #[test]
    fn status_clean_exits_ok() {
        let entries = vec![StatusEntry {
            mapping: "dots".into(),
            key: "x".into(),
            live: "/live/x".into(),
            store: "/store/x".into(),
            mode: Mode::Symlink,
            label: StatusLabel::Ok,
        }];
        let mut buf = Vec::new();
        let code = render_status(&mut buf, &entries, &[], false).unwrap();
        assert_eq!(code, EXIT_OK);
    }

    // ----- list ----------------------------------------------------------

    fn list_config() -> ResolvedConfig {
        ResolvedConfig {
            mappings: vec![ResolvedMapping {
                name: "dots".into(),
                live: "/home/user".into(),
                store: "/store/dots".into(),
                mode: Mode::Symlink,
                conflict: Conflict::Backup,
                links: vec![
                    (".bashrc".into(), LinkValue::Boolean(true)),
                    (".vimrc".into(), LinkValue::String("vim/vimrc".into())),
                ],
                inactive: None,
            }],
        }
    }

    #[test]
    fn list_human_with_and_without_entries() {
        let cfg = list_config();
        let mut brief = Vec::new();
        render_list(&mut brief, &cfg, false, false).unwrap();
        insta::assert_snapshot!("list_human_brief", String::from_utf8(brief).unwrap());

        let mut full = Vec::new();
        render_list(&mut full, &cfg, true, false).unwrap();
        insta::assert_snapshot!("list_human_entries", String::from_utf8(full).unwrap());
    }

    #[test]
    fn list_human_empty() {
        let cfg = ResolvedConfig { mappings: vec![] };
        let mut buf = Vec::new();
        let code = render_list(&mut buf, &cfg, true, false).unwrap();
        assert_eq!(code, EXIT_OK);
        assert_eq!(String::from_utf8(buf).unwrap(), "no mappings configured\n");
    }

    #[test]
    fn list_json_resolves_entry_paths() {
        let cfg = list_config();
        let mut buf = Vec::new();
        render_list(&mut buf, &cfg, true, true).unwrap();
        let v: Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        let m = &v["mappings"][0];
        assert_eq!(m["name"], "dots");
        assert_eq!(m["mode"], "symlink");
        assert_eq!(m["conflict"], "backup");
        let entries = m["entries"].as_array().unwrap();
        let bashrc = entries.iter().find(|e| e["key"] == ".bashrc").unwrap();
        assert_eq!(bashrc["live"], "/home/user/.bashrc");
        assert_eq!(bashrc["store"], "/store/dots/.bashrc");
        // Explicit string value redirects the store side.
        let vimrc = entries.iter().find(|e| e["key"] == ".vimrc").unwrap();
        assert_eq!(vimrc["store"], "/store/dots/vim/vimrc");
    }

    // ----- add / remove --------------------------------------------------

    #[test]
    fn add_human_variants() {
        let mut buf = Vec::new();
        let code = render_add(
            &mut buf,
            "dots",
            ".bashrc",
            Some(Path::new("/cfg/symify.toml")),
            "added",
            &Outcome::Applied(ActionKind::Adopt),
            false,
            false,
        )
        .unwrap();
        assert_eq!(code, EXIT_OK);
        insta::assert_snapshot!("add_human_applied", String::from_utf8(buf).unwrap());

        let mut buf2 = Vec::new();
        render_add(
            &mut buf2,
            "dots",
            ".bashrc",
            None,
            "unchanged",
            &Outcome::Conflict,
            false,
            false,
        )
        .unwrap();
        let s = String::from_utf8(buf2).unwrap();
        assert!(s.contains("(already present)"));
        assert!(s.contains("conflict — not adopted"));
    }

    #[test]
    fn add_json_and_exit_codes() {
        let mut buf = Vec::new();
        let code = render_add(
            &mut buf,
            "dots",
            ".bashrc",
            Some(Path::new("/cfg/symify.toml")),
            "added",
            &Outcome::AppliedDrift(ActionKind::Push),
            false,
            true,
        )
        .unwrap();
        assert_eq!(code, EXIT_DRIFT);
        let v: Value = serde_json::from_str(&String::from_utf8(buf).unwrap()).unwrap();
        assert_eq!(v["action"], "add");
        assert_eq!(v["adopt"], "push");
        assert_eq!(v["file"], "/cfg/symify.toml");

        let mut buf2 = Vec::new();
        let code2 = render_add(
            &mut buf2,
            "dots",
            "x",
            None,
            "added",
            &Outcome::Failed("nope".into()),
            false,
            true,
        )
        .unwrap();
        assert_eq!(code2, EXIT_FAILURE);
    }

    #[test]
    fn remove_human_and_json() {
        let mut buf = Vec::new();
        let code = render_remove(
            &mut buf,
            "dots",
            ".bashrc",
            &[PathBuf::from("/cfg/symify.toml")],
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(code, EXIT_OK);
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("removed .bashrc from dots"));
        assert!(s.contains("restored: standalone copy"));
        insta::assert_snapshot!("remove_human_restored", s);

        let mut buf2 = Vec::new();
        render_remove(&mut buf2, "dots", ".bashrc", &[], false, true, true).unwrap();
        let v: Value = serde_json::from_str(&String::from_utf8(buf2).unwrap()).unwrap();
        assert_eq!(v["action"], "remove");
        assert_eq!(v["restored"], false);
        assert_eq!(v["dry_run"], true);
        assert_eq!(v["files"].as_array().unwrap().len(), 0);
    }
}
