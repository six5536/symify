# Plan: Shared store targets — documented pattern + collision notes

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

Two entries may resolve to the same store path (`D`) — several `live` paths
backed by one store file, via explicit link values. This is legitimate and
useful (verified 2026-08-05: symlink mode gives one source of truth surfaced
at many paths; copy mode fans one store file out under `deploy`), but it is
undocumented, and the hazardous variant — `sync` in copy mode with diverging
lives — fails silently: last-writer-wins by mapping order, same-second `.bak`
name collisions clobber the earlier backup, and the quick-check can then hide
the flip-flop. Two deliverables:

- **(a)** Document the shared-store-file pattern: what works, what to avoid.
- **(b)** Detect entries whose resolved paths collide and surface a
  **warning note** (never an error) in verb output.

## Functional requirements

### (a) Documentation

- README: a short "Sharing one store file between paths" note under
  Configuration — the explicit-value pattern, symlink = one source of truth,
  copy = deploy-only fan-out, and the sync-divergence warning.
- Knowledgebase: the pattern and its hazards in
  [architecture](/knowledge/architecture.md) (link-resolution section); the
  note output in [api-contracts](/knowledge/api-contracts.md).

### (b) Collision notes

- Detection: after resolve + `-m` selection, group active entries whose
  **normalized** resolved paths are equal — both same-`D` (shared store
  target) and same-`S` (two entries claiming one live path; almost always an
  accident). Exact path equality only; ancestor/descendant overlap is out of
  scope for v1.
- Surfaced on `status`, `sync`, `deploy`, and `diff` as one line per
  collision group, styled like the inactive-mapping notes:
  `note: 2 entries share store path <D>: a/profile, b/prof.d/profile`
  (`share live path` for the `S` case).
- `--json`: a `shared_targets` array of
  `{ side: "store" | "live", path, entries: [{ mapping, key }] }`,
  omitted when empty.
- **Exit code unaffected** — a note, not drift and not an error. Shared
  targets are legitimate; the config says what happens.
- Disabled entries and inactive mappings do not participate.

## Non-functional requirements

- Detection is a pure function of already-resolved entries — a small helper
  in `symify-core` (over `(mapping, key, live, store)` tuples, using
  `fs::normalize` like the guards) so library consumers get it too; the
  binary only renders.
- No blocking, no plan mutation, no dedup of actions: behaviour of colliding
  entries is unchanged, only visibility improves.

## Design decisions

1. Warn-only, all verbs. Escalating copy-mode `sync` collisions to errors
   would break the legitimate deploy-only configs that share a store root and
   occasionally sync other entries; the note plus docs carry the safety story.
2. Same-`S` detection included: identical cost, higher accident value.
3. The same-second `.bak` collision observed in testing is **not** addressed
   here — it is a pre-existing executor naming issue independent of shared
   targets; record it in the backlog as its own item.

## Work breakdown

- **A. Core**: `plan::shared_targets(...)` (or sibling module) returning
  collision groups; unit tests incl. normalization equivalence
  (`/x/../store/f` == `/store/f`), disabled/inactive exclusion, S and D
  cases, no-collision case.
- **B. Binary**: render the note lines (human) and `shared_targets` (JSON)
  in run/status/diff output; wire from the selected config.
- **C. Tests (e2e)**: shared-`D` config → note on `status` and `sync`, exit
  code unchanged (0 when otherwise clean); same-`S` accident → note; `--json`
  shape; distinct paths → no note.
- **D. Docs**: README pattern + note mention; `architecture` pattern;
  `api-contracts` note + JSON field; CHANGELOG entry; backlog item for the
  `.bak` same-second name collision (decision 3).

## Critical files

- `crates/lib/symify-core/src/plan.rs` (helper + tests)
- `crates/app/symify/src/{main,output}.rs`, `crates/app/symify/tests/cli.rs`
- `README.md`, `knowledge/{architecture,api-contracts,backlog}.md`,
  `CHANGELOG.md`

## Definition of done

- [x] Shared-`D` and shared-`S` groups reported as notes on
      `status`/`sync`/`deploy`/`diff`, human and `--json`, exit codes
      unchanged (tests).
- [x] Detection pure, in core, normalization-aware; disabled entries and
      inactive mappings excluded (unit tests).
- [x] Pattern documented in README and `architecture`; note contract in
      `api-contracts`; `.bak` collision recorded in the backlog.
- [x] All gates green; knowledgebase validator passes.
