# Implementation Plan: Drop hardlinks + rsync-like sync mode

> Working plan for this change, kept as a record once landed. Written for a fresh
> session — it assumes no prior context. (Plans are retained, not deleted.)

## Goal

Two user-requested changes to symify (project is **unreleased**, breaking
changes allowed):

1. **Remove `hardlink` mode entirely** — it is considered dangerous. `Mode`
   becomes `symlink | sync`.
2. **Make `sync` mode fast and reliable like rsync** — not the current
   "hash the whole tree, then recopy the whole tree" behaviour.

## Decisions (locked with the user — do not re-litigate)

- **Hardlinks:** remove entirely. A config with `mode = "hardlink"` should now
  fail to load with a clear error (default serde enum behaviour is acceptable;
  a friendlier message is a nice-to-have, not required).
- **Sync identity (the per-run "is this file unchanged?" check):** size + mtime
  quick-check (rsync's default). Add a `--checksum` flag that forces an exact
  content compare (the existing BLAKE3 digest path). mtime must be **preserved
  on copy** so the quick-check is stable across runs.
- **Mirror / delete:** deletion of destination files that no longer exist in the
  source is a **separate, opt-in axis** from `conflict` (they are orthogonal:
  `conflict` = how to overwrite a file present on both sides; mirror = whether to
  prune files present only on the destination).
  - New `mirror` config key (in `[settings]` and per-mapping, like `mode`) +
    `--delete` CLI flag on `sync`/`deploy`. **Default off** (additive: only
    add/update; never prune).
  - When mirror is on, dispose of a pruned file **via the `conflict` policy**:
    `backup` → rename to `.bak` then remove; `replace` → hard delete; `skip` →
    do not delete, report as drift.
  - Mirror only bites on **directory entries** (a single-file entry has nothing
    extraneous to prune). It prunes whichever side the verb writes: `sync`
    prunes `store`, `deploy` prunes `live`.
  - Whole-directory prunes must still go through the existing unrecoverable-
    delete confirmation gate (`crates/app/symify/src/confirm.rs`).

## Resolved design decisions (grilled — do not re-litigate)

These eight resolutions close gaps the high-level plan left open. They refine
B1/B3/B4/B6 below; where they conflict with looser wording later in this file,
these win.

1. **Partial application + a drift state.** A `sync`-mode *directory* entry under
   `conflict = skip` copies brand-new files (dest absent → pure add) but leaves
   same-path differing files untouched. The leftover difference is reported via a
   new "applied, but drift remains" state, so the idempotency invariant stays
   honest (a second run reports the residual `Differs`, not a false all-clean).
   This is `rsync --ignore-existing` semantics plus the drift reporting rsync
   lacks. The `Action`/`Outcome` model gains an applied-with-drift variant, and
   exit-code logic (`is_drift` / `output.rs`) counts it as drift (exit 1).

2. **symify ignores its own artifacts in the walk.** The directory diff treats
   `*.bak` and `*.symify-tmp.*` as invisible on **both** sides — never pruned as
   extraneous, never picked up as a source add. This prevents `.bak.bak…` chains
   (mirror + `backup`) and stops a deploy-side backup from being copied into the
   store on the next sync. Cost: a file the user genuinely named `*.bak` can't be
   tracked (acceptable — unreleased).

3. **Symlinks inside a synced tree are compared as symlinks (lstat).** A symlink
   node is equal iff both sides are symlinks with the same target string; never
   follow it. `digest()` (the `--checksum` path) is changed to match — use
   `symlink_metadata`, hash the link target string for link nodes. This makes
   compare consistent with `copy_tree` (which already preserves links), matches
   rsync's default, and turns today's "dangling link → entry fails" into graceful
   handling.

4. **Run-flag plumbing.** `mirror` lives on `ResolvedMapping` (merged mapping >
   settings > default `false`) and is the planner's single source of truth.
   `--delete` is applied as a **pre-plan config override** at the CLI boundary
   (`main.rs run_verb`: set `mirror = true` on the selected mappings before
   calling `plan`) — equivalent to setting it in config, so the planner stays
   CLI-ignorant. Per-run flags travel in a struct: `RunOptions { checksum: bool,
   modify_window: u64 /* seconds */ }`, threaded into `plan(config, verb, opts)`
   and `status(config, opts)`. `checksum` only flows into the copy-mode helpers
   (link-mode relink keeps exact `content_equal`). `status` also gains
   `--checksum` and `--modify-window` so its report matches what a run decides.

5. **Atomic copy is hand-rolled (no new dependency).** In `copy_tree`'s file
   branch: write to `.<name>.symify-tmp.<pid>.<AtomicU64>` in the destination's
   own directory → `set_permissions` (source mode) → `set_times`
   (mtime = source, via `std::fs::FileTimes`) → `rename` over the destination
   (same-fs ⇒ atomic). Best-effort `remove_file` of the temp on any error after
   creation. `tempfile` stays a dev-dependency only.

6. **mtime: exact by default, plus `--modify-window`.** `quick_equal` treats two
   mtimes as equal when they differ by ≤ the window; default `0` = exact (correct
   on normal local filesystems). `--modify-window 1` covers FAT/coarse/network
   stores (rsync-compatible). The window is ignored under `--checksum`. Note for
   the git-backed store case: a `git clone`/`pull` resets mtime, but this is
   self-healing — the first `deploy` into a missing `live` re-establishes a
   matching baseline; a content-changing pull correctly re-syncs; a mtime-only
   touch causes one no-op copy then stabilizes. No special handling needed.

7. **Prune confirmation: opt-in is the boundary.** Keep the existing gate
   (prompt only on recursive delete of a non-empty directory). The opt-in
   `--delete` / `mirror = true` is itself the confirmation; `sync` prunes the
   git-backed store. Do **not** add a per-file/bulk-delete prompt. Do add prune
   **visibility**: `--dry-run` and normal output list each pruned path (and a
   count) so the user can preview before committing.

8. **Run output stays one line per entry.** The renderer derives copied /
   backed-up / pruned counts from `Planned.action.ops` (core carries no counts —
   only the applied-with-drift signal from #1). Human line example:
   `! push  dots/nvim (+2 ~1 -3, 1 drift)`. JSON: add `copied`, `backed_up`,
   `pruned`, and `drift` fields to `RunEntryJson`. `status` stays coarse
   (`ok`/`differs` per entry — no per-file status). Do not expand to per-file
   sub-lines (would break the 1:1 entry↔config-entry shape of the JSON array).

## Architectural principle to respect

The planner (`crates/lib/symify-core/src/plan.rs`) is **pure**: it reads the FS
but never mutates, and emits an ordered `Vec<FsOp>` per entry. `status`,
`--dry-run`, and real runs share this path. Keep the sync diffing **in the
planner** (emit per-file `Copy`/`Backup`/`Remove` ops), NOT hidden inside the
executor — that keeps dry-run, `status`, and the delete-confirmation gate
accurate for free, and matches the existing test style (tests assert exact op
lists).

## Current-state map (key references)

- `crates/lib/symify-core/src/model/`
  - `generated.rs` — typify-generated from the schema. `Mode` enum at ~`287-294`
    (`Symlink`/`Hardlink`/`Sync`) + `Display`/`FromStr` at ~`295-314`. **Do not
    hand-edit casually**; regenerate via `npm run codegen` after the schema
    change (CI drift-checks it). Hand-editing to match is acceptable only if
    codegen can't be run — but then it must still match what codegen would emit.
  - `mod.rs` — `DEFAULT_MODE = Mode::Symlink`, `DEFAULT_CONFLICT`, `LinkKind`,
    `LinkValue::kind()`. Add a `DEFAULT_MIRROR = false` here.
- `schema/symify.schema.json`
  - `Mode` enum at line 21: `["symlink", "hardlink", "sync"]` → `["symlink", "sync"]`.
  - Add a `Mirror`/boolean field to `Settings` (line 34-51) and `Mapping`
    (line 52-68). Model it as `"mirror": { "type": "boolean", ... }`.
- `crates/lib/symify-core/src/plan.rs`
  - `FsOp` enum `28-62` — remove `Hardlink` variant (`48-54`).
  - `link_op` `307-318` — collapses to always-`Symlink`.
  - `plan_sync` `322-328`, `plan_sync_link` `330-386`, `plan_sync_copy` `388-410`.
  - `plan_deploy` `414-422`, `plan_deploy_link` `424-466`, `plan_deploy_copy`
    `468-490`.
  - Hardlink tests: `sync_hardlink_dir_fails` `746-757`,
    `sync_hardlink_already_ok_when_same_inode` `759-771`,
    `deploy_hardlink_dir_fails` `905-916`. Remove them.
  - `Planned.mode` etc. — note there is currently no `mirror` on `Planned`;
    thread it through (see below).
- `crates/lib/symify-core/src/fs.rs`
  - `same_inode` `83-99` (+ test `408-419`) — becomes dead after hardlink
    removal; delete.
  - `synced_equal`/`content_equal`/`equal`/`digest`/`perm_bits` `101-171` — the
    identity machinery. Replace `synced_equal`'s whole-tree hash with a quick
    size+mtime+mode walk; keep `digest` for `--checksum`.
  - `apply_op` `177-195` — remove `FsOp::Hardlink` arm; `copy_tree` `234-255`
    is the file/dir copy (make per-file copy atomic + preserve mtime).
  - `is_nonempty_dir` `37-44` — used by the confirm gate; reused for mirror
    prunes.
- `crates/lib/symify-core/src/status.rs`
  - `label_for` `90-131` — remove hardlink branches (`95-105` collapses).
- `crates/app/symify/src/`
  - `main.rs` `would_restore` `299-308` — drop `Mode::Hardlink` arm (uses
    `same_inode`). Add `--delete`/`--checksum` wiring into `run_verb`.
  - `cli.rs` — `RunArgs` (`44-65`): add `--delete` and `--checksum` bools. Doc
    comment line 7 mentions "hardlinks". (Note: a separate already-applied change
    added `normalize_args` for the bare-path→`add` shortcut; leave it intact.)
  - `output.rs` `mode_str` ~line 25 — remove `"hardlink"`.
  - `confirm.rs` — gate over planned ops; ensure it still flags unrecoverable
    recursive deletes produced by mirror prunes.
- Docs: `README.md` (lines 3, 68 mention hardlinks; mode comment), `Cargo.toml`
  keywords (`"hardlink"`), `specs/ARCHITECTURE.md` (many references: mode table
  ~50-57, directory entries 233-238, correctness tests 240-258, state machine
  84-124).

## Work plan

### Phase A — Remove hardlink mode (do first; it shrinks the surface B touches)

1. Schema: drop `"hardlink"` from the `Mode` enum.
2. Regenerate `model/generated.rs` (`npm run codegen`), or hand-match if codegen
   is unavailable.
3. `plan.rs`: delete `FsOp::Hardlink`; collapse `link_op` to symlink-only;
   delete hardlink branches in `plan_sync_link`/`plan_deploy_link`; the
   `Mode::Symlink | Mode::Hardlink` arms become `Mode::Symlink`; remove the 3
   hardlink tests.
4. `fs.rs`: remove the `FsOp::Hardlink` executor arm; delete `same_inode` (both
   cfg variants) and its test.
5. `status.rs`: remove hardlink branches in `label_for`; remove the
   `disabled_and_hardlink_dir` hardlink half (keep the disabled assertion).
6. `main.rs` `would_restore`, `output.rs` `mode_str`, `cli.rs` doc.
7. Docs: README, ARCHITECTURE.md, Cargo.toml keywords.
8. Build + clippy + nextest green. Commit: "Remove hardlink mode".

### Phase B — rsync-like sync mode

**B1. Fast identity.**
- Add `quick_equal(a, b) -> Result<bool>` in `fs.rs`: recursive walk comparing
  per-node `(is_dir, len for files, mtime, perm_bits)`, short-circuiting on the
  first diff. Directories: compare the set of entry names, recurse. This is
  O(files) stat calls, not O(bytes).
- mtime equality honours `modify_window` (decision #6): equal when
  `|mtime_a - mtime_b| <= window`; `window = 0` means exact.
- Symlink nodes are compared **as symlinks** (decision #3): equal iff both are
  symlinks with the same target string; never follow. Use `symlink_metadata`.
- Keep `digest`-based exact compare for `--checksum` (rename `synced_equal` →
  e.g. `checksum_equal`, or keep and add `quick_equal`). **Change `digest` to
  match decision #3**: `symlink_metadata` + hash the link target string for link
  nodes, so the checksum path agrees with `quick_equal` and stops erroring on
  dangling links.
- `status` and the planner use `quick_equal` by default, `checksum_equal` when
  `--checksum` is set. Per-run flags travel in `RunOptions { checksum: bool,
  modify_window: u64 }` (decision #4), threaded into `status(config, opts)` and
  the plan path.

**B2. Preserve mtime + atomic copy (executor).**
- In `copy_tree` (file branch): copy to a temp file in the **same directory**
  named `.<name>.symify-tmp.<pid>.<AtomicU64>` (decision #5), set permissions,
  **set mtime to match source**, then `rename` over the destination. Use
  `std::fs::FileTimes` (`File::set_times`) for mtime; it is stable in recent std
  (toolchain is 1.96, fine). Best-effort `remove_file` of the temp on any error
  after creation. No new dependency (`tempfile` stays dev-only).
- Directory mtime preservation is not required for the quick-check (dirs are
  compared by children); keep existing dir-permission preservation.

**B3. Incremental per-file diff (planner).**
- Replace the body of `plan_sync_copy` / `plan_deploy_copy`:
  - If source missing → existing skip/pull semantics unchanged.
  - If destination missing → copy the whole (sub)tree as today (still emit
    per-file `Copy` ops, or a single tree copy — either is fine when nothing to
    diff).
  - The walk **skips `*.bak` and `*.symify-tmp.*`** on both sides (decision #2):
    they are neither copied as source adds nor pruned as extraneous.
  - If both exist: walk source vs destination. For each **source** path:
    - dest missing → `Copy(s_file, d_file)` (a pure add — emitted even under
      `conflict = skip`, decision #1).
    - dest present and `quick_equal` (or `checksum_equal`) → nothing.
    - dest present and differs → conflict policy: `skip` → leave + mark the entry
      as carrying drift (decision #1); `backup` → `Backup(d_file)` + `Copy`;
      `replace` → `Remove(d_file)` + `Copy`.
  - For each **destination** path with no source counterpart, **only when
    `mirror` is on**: dispose per conflict: `skip` → leave + mark drift;
    `backup` → `Backup(d_path)`; `replace` → `Remove(d_path)`. Emit copy ops
    before prune ops for readable output.
  - Aggregate into the entry's `Action`. No ops and no drift → `AlreadyOk`.
    Ops but no unresolved `skip` difference → `Apply { kind: Push/Pull, ops }`.
    Unresolved `skip` differences present → the new **applied-with-drift** state
    (decision #1): carries `kind` + `ops` (the pure adds still run) **and** a
    drift flag, so the entry both applies and reports drift (exit 1). With no
    applicable ops at all, it degrades to plain `Conflict`.
- Single-file sync entries keep today's simple shape, but switch to
  `quick_equal` + the atomic/mtime-preserving copy (consequence of #5/#6).
- Helpers learn `mirror` + `RunOptions`: `mirror` from the resolved mapping,
  `checksum`/`modify_window` from `RunOptions` threaded through `plan`.

**B4. Mirror config plumbing.**
- Schema: add `mirror` boolean to `Settings` + `Mapping`. Regen model.
- `model/mod.rs`: `DEFAULT_MIRROR = false`.
- `config.rs` `resolve`: add `mirror` to `ResolvedMapping` (merge like
  `mode`/`conflict`: mapping over settings over default). Update `merge_settings`
  / `merge_mapping` for the new field, and the resolve tests' fixtures.
- `cli.rs`: add `--delete`, `--checksum`, and `--modify-window <SECONDS>`
  (default 0) to `RunArgs`; add `--checksum` and `--modify-window` to `QueryArgs`
  (decision #4/#6) so `status` matches what a run would decide.
- `main.rs` `run_verb`: build `RunOptions { checksum, modify_window }` and pass
  to `plan`/`status`. Apply `--delete` as a **pre-plan config override**: when
  set, flip `mirror = true` on the selected mappings before calling `plan`
  (decision #4) — the planner reads only `ResolvedMapping.mirror`.
- `plan.rs` test fixtures (`Fx::cfg`) gain a `mirror` field — update the
  `ResolvedMapping` literal construction across plan.rs/status.rs/main.rs tests.

**B5. Confirmation gate + prune visibility.**
- `confirm.rs`: a mirror prune that recursively deletes a non-empty directory is
  unrecoverable under `replace` — ensure the gate still detects `Remove(dir)`
  ops the same way it does for conflict=replace today. (Likely already covered
  since it inspects `FsOp::Remove`; verify with a test.)
- No per-file/bulk-delete prompt (decision #7): the opt-in `--delete` / `mirror`
  is the safety boundary. Instead, surface pruned paths in `--dry-run` and normal
  output so the user can preview before committing.

**B6. Tests (mirror the existing style — real temp trees).**
- Planner units for the new copy diff: dest-missing-file → Copy; one-file-changed
  → only that file's ops (NOT a whole-tree recopy — this is the headline win,
  assert it); unchanged tree → AlreadyOk; mirror off → extraneous dest file left;
  mirror on + backup → `Backup` then nothing; mirror on + replace → `Remove`;
  mirror on + skip → leave + drift.
- Decision-specific units:
  - **#1 partial apply:** dir with a new file + a same-path diff under `skip` →
    the new file's `Copy` runs **and** the entry reports applied-with-drift
    (exit 1); second run still reports the residual drift.
  - **#2 artifact exclusion:** a `*.bak` / `*.symify-tmp.*` on the dest is not
    pruned by `mirror`; on the source it is not copied. Mirror + backup is
    idempotent (no `.bak.bak`).
  - **#3 symlinks:** equal link target → AlreadyOk; changed target → diff;
    dangling link does not error; copy recreates the link verbatim.
  - **#6 modify-window:** mtime within window → equal; outside → diff.
- `fs.rs`: `quick_equal` detects size/mtime/mode drift and equality; atomic copy
  leaves no partial file on simulated failure (best-effort); mtime preserved.
- CLI e2e (`crates/app/symify/tests/cli.rs`): `sync` of a large-ish dir touches
  only changed files; `--delete` prunes and the output **lists pruned paths**
  (decision #7); default does not prune; `--checksum` forces a recompare when
  mtime was bumped but content is identical; `--modify-window` tolerates a small
  mtime skew. Assert the per-entry count/drift fields in `--json` (decision #8).
- Idempotency invariant: second `sync`/`deploy` is all-AlreadyOk, exit 0
  (except entries left in drift by `conflict = skip`, which persist by design).

**B7. Docs.**
- ARCHITECTURE.md: update Modes list (drop hardlink), the sync/deploy state
  machine tables (incl. the new applied-with-drift state, decision #1), the
  "Correctness tests" section (size+mtime quick-check + `--checksum` +
  `--modify-window`; mtime now preserved and part of sync identity; symlinks
  compared as symlinks, decision #3), add the `mirror` axis + `--delete` flag to
  the CLI surface and config sections, and note that symify's own `*.bak` /
  `*.symify-tmp.*` are invisible to the diff (decision #2).
- README: mode line, add `--delete`/`--checksum`/`--modify-window` to the flags
  blurb, `mirror` config key example.
- Schema starter template in `config.rs` `render_starter` — mention `mirror`
  only if you want it discoverable (optional; keep minimal).

Commit B in logical chunks (identity+atomic; incremental diff; mirror plumbing;
docs) or as one feature commit — reviewer's preference.

### Phase C — Release automation (separate, not required for A/B)

Only `.github/workflows/ci.yml` exists (PR/push: fmt, clippy, nextest, launcher
test, schema drift). No tag-triggered release. When shipping: add a release
workflow that cross-builds via the existing `package.json` `build:*` scripts
(cargo-zigbuild), assembles the `packages/*` platform packages, `npm publish`es
launcher + platform packages atomically, and `cargo publish`es. Out of scope for
this change.

## Environment notes

- The earlier Bash permission blocker (`EACCES … session-env`) is resolved — the
  shell, `cargo`, and `mise` run normally. Still: do not land the schema→codegen
  regen or the planner rewrite without `cargo nextest run --workspace` and
  `npm run codegen:check` green.
- `cargo-zigbuild` is now pinned in `.mise.toml` (`cargo:cargo-zigbuild
  = 0.22.3`) and installed, so Phase C's cross-builds via `package.json`
  `build:*` can run under mise.

## Definition of done

- `Mode` is `symlink | sync` everywhere (schema, generated model, planner,
  status, CLI, docs); no `hardlink`/`same_inode`/`FsOp::Hardlink` references
  remain.
- `sync`/`deploy` copy only changed files (verified by a test asserting a
  one-file change does not recopy the tree), use a size+mtime quick-check, copy
  atomically, and preserve mtime; `--checksum` forces exact compare and
  `--modify-window` tolerates coarse-fs mtime skew (decision #6).
- Under `conflict = skip`, a partially-changed directory still copies its
  brand-new files and reports the residual difference as applied-with-drift
  (exit 1), and a second run reports the same drift (decision #1).
- symify's own `*.bak` / `*.symify-tmp.*` are invisible to the diff — mirror +
  backup is idempotent, no `.bak.bak` chains (decision #2).
- Symlinks inside a synced tree are compared as symlinks and copied verbatim; a
  dangling link does not fail the entry (decision #3).
- `mirror` config key + `--delete` flag work, default off, dispose via
  `conflict`, gated for unrecoverable dir prunes, and pruned paths are listed in
  output / `--dry-run` (decision #7).
- `--json` run output carries per-entry `copied`/`backed_up`/`pruned`/`drift`
  fields, one entry per config entry (decision #8).
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run
  --workspace`, and `npm run codegen:check` all pass.
- ARCHITECTURE.md and README updated. (This plan is retained as a record.)
</content>
</invoke>
