# Plan: Bounded `.bak` retention

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

Cap the `.bak` files that accumulate on repeated conflicts — the open backlog
item and the one wart in the "safe by default" story (the safety mechanism
itself makes a mess). A `backup_keep = N` setting, applied when a new backup
is written.

```toml
[settings]
backup_keep = 3   # keep the newest 3 backups per path; absent/0 = keep all
```

## Functional requirements

- `backup_keep`: non-negative integer on `[settings]`, overridable per
  mapping. Absent or `0` = unlimited (today's behaviour — the feature is
  opt-in).
- When the planner emits a `Backup` op for a path whose leaf is `<name>`, it
  lists siblings matching exactly `^<name>\.\d{14}\.bak$` and plans `Remove`
  for all but the newest `N - 1` (the new backup counts toward `N`). Newest =
  lexicographic on the timestamp segment, which is chronological for
  `YYYYMMDDHHMMSS`.
- Retention runs **only** when a new backup is being written. No sweep, no
  standalone cleanup pass.
- The removals appear in `--dry-run` and `--json` like any other op.
- A `.bak` that is a non-empty directory goes through the existing
  unrecoverable-delete confirmation gate (`[y/N]` / `--yes`) — deleting a
  backup destroys a recovery path, so the gate is correct, not an annoyance
  to engineer around.

## Non-functional requirements

- Planner purity holds: the sibling listing is a read; deletes are planned
  ops executed by the executor.
- Idempotency invariant holds: a second run plans no retention removals
  (nothing new to back up ⇒ retention does not run).

## Architecture carve-out (signed off — record it)

"symify never discovers files" gains one narrow exception: when writing a
backup, the planner reads the entry's own parent directory for names matching
the exact `<name>.<14-digit-timestamp>.bak` pattern of the entry's own leaf.
It never matches other files, other entries' backups, or user files.
**Sign-off given (grilled 2026-08-05)** per
[security-requirements](/knowledge/security-requirements.md). Record the
carve-out in [architectural-rules](/knowledge/architectural-rules.md) next to
the additive-copy rule.

## Resolved design decisions (grilled 2026-08-05 — do not re-litigate)

1. **Opt-in: absent/`0` = unlimited** (today's behaviour, byte-identical).
   The safest backup policy is keeping them; no upgrade silently deletes.
   The auto-init starter template gains a commented `# backup_keep = 5` line
   so new users discover the setting.
2. Setting, not a `clean-backups` verb: the backlog asked for a cap, and a
   verb is manual (the mess returns). A future verb can reuse the same
   matcher if wanted.
3. Per-path retention, not global: `N` bounds each entry's backups; a global
   budget would need cross-entry ordering for little gain.
4. Files not matching the exact pattern (hand-renamed backups, `foo.bak`) are
   invisible to retention — same principle as the artifact filter.

## Work breakdown

- **A. Schema + regen**: `backup_keep` on `Settings` and `Mapping`;
  `npm run codegen`; commented `# backup_keep = 5` line in the starter
  template.
- **B. Core**: resolve/merge the setting; retention logic beside backup
  planning in `plan.rs` (matcher shared with `is_artifact` machinery where
  sensible); ops flow through the existing executor unchanged.
- **C. Tests**: planner cases (under/at/over the cap, `0`/absent, non-matching
  names untouched, per-mapping override); executor case with the injected
  clock pinning timestamps; `--dry-run` shows removals; confirmation-gate
  case for a directory backup; idempotency (second run plans nothing).
- **D. Docs/KB**: README Safety + Backups sections, schema description,
  knowledgebase (`configuration`, `architectural-rules` carve-out,
  `glossary`; move the backlog item to Done with the reasoning).

## Critical files

- `schema/symify.schema.json`, `crates/lib/symify-core/src/model/`
- `crates/lib/symify-core/src/{config,plan}.rs`
- `README.md`, `knowledge/{configuration,architectural-rules,backlog,glossary}.md`

## Definition of done

- [x] Cap enforced exactly (newest `N` survive, new backup included), only on
      exact-pattern siblings of the entry's own leaf.
- [x] Absent/`0` behaviour byte-identical to today.
- [x] Removals visible in `--dry-run`/`--json`; directory backups gated.
- [x] Carve-out recorded in `architectural-rules` (sign-off already given,
      2026-08-05).
- [x] All gates green; knowledgebase validator passes.
