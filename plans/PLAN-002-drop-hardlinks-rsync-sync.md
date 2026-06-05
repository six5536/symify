# Implementation Plan: Drop hardlinks + rsync-like sync mode

> Working plan for an in-progress change. Delete this file once the work has
> landed and ARCHITECTURE.md / README reflect it. Written for a fresh session —
> it assumes no prior context.

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
- Keep `digest`-based exact compare for `--checksum` (rename `synced_equal` →
  e.g. `checksum_equal`, or keep and add `quick_equal`).
- `status` and the planner use `quick_equal` by default, `checksum_equal` when
  `--checksum` is set. (Thread a `checksum: bool` into `status()` and the plan
  path, or carry it on the resolved/Planned data — pick the smaller diff.)

**B2. Preserve mtime + atomic copy (executor).**
- In `copy_tree` (file branch): copy to a temp file in the **same directory**,
  fsync optional, set permissions, **set mtime to match source**, then `rename`
  over the destination. Use `std::fs::FileTimes` (`File::set_times`) for mtime;
  it is stable in recent std (toolchain is 1.96, fine). No new dependency.
- Directory mtime preservation is not required for the quick-check (dirs are
  compared by children); keep existing dir-permission preservation.

**B3. Incremental per-file diff (planner).**
- Replace the body of `plan_sync_copy` / `plan_deploy_copy`:
  - If source missing → existing skip/pull semantics unchanged.
  - If destination missing → copy the whole (sub)tree as today (still emit
    per-file `Copy` ops, or a single tree copy — either is fine when nothing to
    diff).
  - If both exist: walk source vs destination. For each **source** path:
    - dest missing → `Copy(s_file, d_file)`.
    - dest present and `quick_equal` (or `checksum_equal`) → nothing.
    - dest present and differs → conflict policy: `skip` → Conflict (report);
      `backup` → `Backup(d_file)` + `Copy`; `replace` → `Remove(d_file)` +
      `Copy`.
  - For each **destination** path with no source counterpart, **only when
    `mirror` is on**: dispose per conflict: `skip` → leave + report drift;
    `backup` → `Backup(d_path)`; `replace` → `Remove(d_path)`.
  - Aggregate into the entry's `Action::Apply { kind: Push/Pull, ops }`. If no
    ops result → `Action::AlreadyOk`. If only unresolved `skip` differences →
    `Action::Conflict`.
- Single-file sync entries keep today's simple logic.
- Decide the cleanest way to make these helpers know `mirror` + `checksum`:
  thread them as params (they already take `conflict`). `mirror` should come
  from the resolved mapping; `checksum` from the CLI flag for that run.

**B4. Mirror config plumbing.**
- Schema: add `mirror` boolean to `Settings` + `Mapping`. Regen model.
- `model/mod.rs`: `DEFAULT_MIRROR = false`.
- `config.rs` `resolve`: add `mirror` to `ResolvedMapping` (merge like
  `mode`/`conflict`: mapping over settings over default). Update `merge_settings`
  / `merge_mapping` for the new field, and the resolve tests' fixtures.
- `cli.rs`: add `--delete` (sets mirror on for the run) and `--checksum` to
  `RunArgs`. `--delete` overrides config `mirror` to true for that run (config
  can also enable it persistently). `--checksum` selects exact compare.
- `main.rs` `run_verb`: pass the flags down to `plan`. Update `Planned` if it
  needs to carry `mirror` for reporting.
- `plan.rs` test fixtures (`Fx::cfg`) gain a `mirror` field — update the
  `ResolvedMapping` literal construction across plan.rs/status.rs/main.rs tests.

**B5. Confirmation gate.**
- `confirm.rs`: a mirror prune that recursively deletes a non-empty directory is
  unrecoverable under `replace` — ensure the gate still detects `Remove(dir)`
  ops the same way it does for conflict=replace today. (Likely already covered
  since it inspects `FsOp::Remove`; verify with a test.)

**B6. Tests (mirror the existing style — real temp trees).**
- Planner units for the new copy diff: dest-missing-file → Copy; one-file-changed
  → only that file's ops (NOT a whole-tree recopy — this is the headline win,
  assert it); unchanged tree → AlreadyOk; mirror off → extraneous dest file left;
  mirror on + backup → `Backup` then nothing; mirror on + replace → `Remove`;
  mirror on + skip → Conflict/drift.
- `fs.rs`: `quick_equal` detects size/mtime/mode drift and equality; atomic copy
  leaves no partial file on simulated failure (best-effort); mtime preserved.
- CLI e2e (`crates/app/symify/tests/cli.rs`): `sync` of a large-ish dir touches
  only changed files; `--delete` prunes; default does not; `--checksum` forces a
  recompare when mtime was bumped but content is identical.
- Idempotency invariant: second `sync`/`deploy` is all-AlreadyOk, exit 0.

**B7. Docs.**
- ARCHITECTURE.md: update Modes list (drop hardlink), the sync/deploy state
  machine tables, the "Correctness tests" section (size+mtime quick-check +
  `--checksum`; mtime now preserved and part of sync identity), add the `mirror`
  axis + `--delete` flag to the CLI surface and config sections.
- README: mode line, add `--delete`/`--checksum` to the flags blurb, `mirror`
  config key example.
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

## Known environment blocker

As of writing, Bash in the session was failing with a harness permission error
(`EACCES … mkdir '~/.claude/session-env/…'`), which blocked `npm run codegen`,
`cargo build`, and the test suite. **Before implementing, confirm the shell
works** (`cargo --version`, `npm run test`). If it is still broken, either fix
the sandbox/permission or have the user run commands via `! <command>`. Do not
land the schema→codegen regen or the planner rewrite without compiling and
running `cargo nextest run --workspace` and `npm run codegen:check`.

## Definition of done

- `Mode` is `symlink | sync` everywhere (schema, generated model, planner,
  status, CLI, docs); no `hardlink`/`same_inode`/`FsOp::Hardlink` references
  remain.
- `sync`/`deploy` copy only changed files (verified by a test asserting a
  one-file change does not recopy the tree), use a size+mtime quick-check, copy
  atomically, and preserve mtime; `--checksum` forces exact compare.
- `mirror` config key + `--delete` flag work, default off, dispose via
  `conflict`, gated for unrecoverable dir prunes.
- `cargo fmt --check`, `cargo clippy -D warnings`, `cargo nextest run
  --workspace`, and `npm run codegen:check` all pass.
- ARCHITECTURE.md and README updated; this PLAN.md deleted.
</content>
</invoke>
