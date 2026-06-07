# Plan: Remove `mirror` / `--delete` (additive-only sync)

> Working plan for this change, kept as a record once landed. Written for a fresh
> session — it assumes no prior context. (Plans are retained, not deleted.)

## Goal

Remove the `mirror` config setting and the `--delete` flag. `sync` mode becomes
**additive**: it copies and updates files, but never deletes files on the other
side. This restores symify's core safety guarantee — *it only ever touches the
exact paths in your config; it never scans a directory or deletes files you
didn't list* — which `mirror` quietly violated, and it removes the `.git` /
VCS-metadata data-loss risk recorded as a Finding in PLAN-004.

`sync` mode itself **stays**. Its config-driven model (named entries, per-mapping
`mode`/`conflict`, a deterministic plan) is a genuine differentiator from rsync,
which is path- and flag-driven. Only the prune/mirror behavior is cut.

This is a **breaking change**, which is fine pre-release.

## Why now

- `mirror = true` walks a directory tree and deletes files that were never named
  in the config — directly contradicting the "never discovers files" property
  advertised in the README, ARCHITECTURE, and SECURITY.
- It is off by default (niche), reimplements rsync's `--delete`, and is the one
  feature that can destroy a `.git` repository (PLAN-004 Finding).
- Pre-release: breaking changes are free, and this is the cheapest time to cut
  scope and shrink the test/doc surface.

## What "done" means

- No `mirror` / `--delete` anywhere: schema, generated types, model, planner,
  CLI, status, tests, docs.
- `sync`/`deploy` copy and update only; destination-only files are left
  untouched — never pruned, and not reported as drift merely for being extra.
- Gates green: `clippy -D warnings`, `cargo doc -D warnings`, doctests,
  `codegen:check` (no drift), **per-crate line coverage ≥ 90%** (PLAN-003 gate),
  and the full `nextest` suite.
- A repo-wide grep for `mirror` / `--delete` / `Mirror` / `prune` comes back
  clean (code and docs).

## Resolved design decisions (grilled — do not re-litigate)

1. **Clean removal, no migration.** `mirror` leaves the schema. `Settings` /
   `Mapping` use `deny_unknown_fields`, so an old `mirror = …` line now fails to
   load with `unknown field 'mirror'` — accepted (no released users). No
   accept-and-ignore, no deprecation. The auto-init starter template has no
   `mirror` line, so default configs stay valid.
2. **`--delete` removed outright** from `RunArgs` (covers `sync` and `deploy`).
   clap rejects it with `unexpected argument '--delete'`; no bespoke deprecation.
3. **`Mirror` newtype and `ResolvedMapping.mirror` deleted**, along with
   `DEFAULT_MIRROR` and the mirror resolution/merge in `resolve()`.
4. **Planner: remove the prune path only.** Delete `prune()` and the
   destination-only branch in `walk_copy`; drop the `mirror` params from
   `plan_sync_copy` / `plan_deploy_copy` / `diff_copy` / `walk_copy`; stop reading
   `dst_names` (only the prune used it). Keep the skip-conflict **drift**
   (`plan.rs:574`) and `ApplyDrift` / `is_drift` — drift now arises solely from
   unresolved `skip` conflicts.
5. **Output: rename `pruned` → `removed`.** The count tallies `FsOp::Remove`,
   which after this change comes only from `conflict = replace` overwrites and
   `symlink` relinks — not pruning. Rename in the result model, the human `-N`
   detail, and the `--json` field; update the affected output tests/snapshots.
   (Pre-release: breaking the JSON field name is free.)
6. **Additive-only is the whole story — no replacement cleanup.** Deleting a
   source file no longer removes its counterpart; cleanup is manual (`rm` in the
   store / `git rm`). One sentence in the docs states this. No "status reports
   extras" affordance — that would re-introduce destination directory-scanning,
   the exact behavior we are removing. A `clean` verb stays deferred post-v1
   (ARCHITECTURE "Deferred").
7. **Explicitly NOT touched.** `is_artifact` filtering (still needed so the
   additive walk skips `*.bak` / `*.symify-tmp.*`); `-y/--yes` and the
   recursive-delete confirmation gate (they serve `conflict = replace`, which is
   independent of mirror); and the `LinkValue` "**mirror the key under `store`**"
   wording (a different sense of the word — it stays). The cleanup grep targets
   the mirror **setting** only.

## Non-goals

- Removing `sync` mode (kept — config-driven is the differentiator).
- Any change to `symlink` mode, `conflict`, `--checksum`, `--modify-window`, or
  the verbs.
- A general exclude/ignore mechanism (a separate idea; record in `IDEAS.md` if
  wanted).
- A CHANGELOG entry (deferred to the release, per PLAN-004 decision 6).

## Current state (where `mirror` / `--delete` live)

- **Schema** (`schema/symify.schema.json`): the `Mirror` `$def`; a `mirror`
  property in `Settings` and in `Mapping`.
- **`model/generated.rs`**: the `Mirror` type and `mirror` fields (regenerated).
- **`model/mod.rs`**: `DEFAULT_MIRROR`; the `Mirror` re-export.
- **`config.rs`**: `ResolvedMapping.mirror`; mirror resolution inside `resolve()`.
- **`plan.rs`**: `mirror` params threaded through `plan_sync_copy` /
  `plan_deploy_copy` / `diff_copy` / `walk_copy`; the `prune()` function; the
  destination-only branch.
- **`cli.rs`**: `--delete` on `RunArgs`.
- **`main.rs`**: the `--delete` → force-`mirror`-on override applied before
  planning.
- **Tests**: planner prune cases, integration mirror/delete cases, CLI
  `--delete` cases.
- **Docs**: README (config line + `--delete` bullet), ARCHITECTURE (modes/flags,
  the state-machine prune rows, the sync-mode section, the CLI surface), schema
  descriptions.

## Work breakdown

- **A. Schema + regen.** Remove the `Mirror` `$def` and the `mirror` properties;
  `npm run codegen`; confirm `generated.rs` drops the type and fields.
- **B. Core.** Drop `DEFAULT_MIRROR`, `ResolvedMapping.mirror`, and the resolve
  logic; remove the `mirror` params, `prune()`, and the destination-only branch
  in the planner; remove the `--delete` override in `main.rs`; remove `--delete`
  from `cli.rs`. Rename the output `pruned` count → `removed` (model, human
  `-N` detail, `--json`).
- **C. Tests.** Delete the mirror/`--delete` tests; keep/extend additive-behavior
  tests (a `sync` run leaves destination-only files alone); re-check the coverage
  gate and backfill a test if a branch drops below 90%.
- **D. Docs.** Strip mirror/`--delete` from README, ARCHITECTURE (including the
  rewrite of the sync-mode section to additive-only) and the schema descriptions;
  mark the PLAN-004 mirror Finding **resolved by removal**. Add one sentence
  that cleanup is now manual (`rm` / `git rm`). Preserve the `LinkValue`
  "mirror the key" wording.
- **E. Verify.** All gates green; final grep clean.

## Critical files

- `schema/symify.schema.json`, `crates/lib/symify-core/src/model/{generated,mod}.rs`
- `crates/lib/symify-core/src/{config,plan,status}.rs`
- `crates/app/symify/src/{cli,main}.rs`
- tests in `crates/lib/symify-core/src/*.rs`, `crates/lib/symify-core/tests/`,
  `crates/app/symify/tests/`
- `README.md`, `specs/ARCHITECTURE.md`, `plans/PLAN-004-finish-documentation.md`

## Risks & mitigations

- **Coverage dips** after deleting code and its tests. *Mitigate:* removing
  tested code with its tests is roughly coverage-neutral; re-check per-crate
  ≥ 90% and backfill if a branch drops.
- **Missed references** leaving dead code or stale docs. *Mitigate:* a final
  repo-wide grep for `mirror` / `--delete` / `Mirror` / `prune`.
- **`deny_unknown_fields` breaks an existing config.** *Mitigate:* intended
  (Q1); no released users.
- **ARCHITECTURE drift.** *Mitigate:* rewrite the sync-mode section; code is
  canonical (PLAN-004 decision 10).

## Definition of done

- [ ] schema / model / planner / CLI / main free of `mirror` and `--delete`;
      regen clean.
- [ ] `sync`/`deploy` never prune; additive behavior covered by a test.
- [ ] grep for the mirror **setting** (`mirror =`, `.mirror`, `Mirror`,
      `DEFAULT_MIRROR`, `prune`/`pruned`) is clean — the `LinkValue` "mirror the
      key" wording is intentionally kept.
- [ ] `clippy -D warnings`, `cargo doc -D warnings`, doctests, `codegen:check`,
      coverage ≥ 90 %/crate, full `nextest` — all green.
- [ ] docs updated; PLAN-004 mirror Finding marked resolved.
- [ ] `symlink` mode and additive `sync` semantics otherwise unchanged.
