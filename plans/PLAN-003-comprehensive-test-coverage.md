# Implementation Plan: Comprehensive test coverage

> Working plan for this change, kept as a record once landed. Written for a fresh
> session — it assumes no prior context. (Plans are retained, not deleted.)

## Goal

Raise symify's test suite from "good on the happy paths" to **comprehensive**,
and keep it there. Today there are 109 tests (75 unit, 8 core-integration, 26
CLI) with strong planner/config/fs coverage but real holes: `output.rs` (591
lines) and `confirm.rs` are untested, error/permission/IO-failure paths are
sparse, and several helpers are only exercised indirectly.

"Comprehensive" is defined here as **two gates, both required**:

1. **Quantitative** — `cargo llvm-cov` **line** coverage **≥ 90%, measured
   per-crate** for `symify-core` and `symify`, enforced in CI. **Region**
   coverage is computed and printed but **advisory** (not a build failure); we
   may promote it to a gate later once we see real numbers.
2. **Qualitative** — every item on the **behavioral checklist** (below) has a
   named test. Coverage % alone can be gamed; the checklist guarantees the
   *right* things are tested, not just that lines were hit.

Both must pass. The % stops silent regressions; the checklist stops "90% but the
untested 10% is all the error handling."

## Resolved design decisions (grilled — do not re-litigate)

1. **Gate metric & threshold:** gate on **line coverage ≥ 90%, per-crate**
   (`symify-core` and `symify` each clear it independently, so the well-tested
   lib can't mask the binary). Region coverage is reported but advisory.
2. **`confirm.rs` testability:** refactor `gate()` to take an injectable
   reader/writer (`impl BufRead` / `impl Write`) so the interactive `[y/N]`
   accept/decline/EOF decision is unit-testable. `main` passes real
   stdin/stderr at the single call site. Byte-identical behavior.
3. **`main.rs` testability:** extract a pure
   `root_refused(mutating: bool, root: bool, allow_root: bool) -> bool` seam
   (replacing the inline `&&` at the refusal site); the tiny `geteuid` syscall
   wrapper `is_root()` stays uncovered-but-documented. All app tests live as
   in-file `#[cfg(test)]` modules **in the app crate** — do **not** migrate CLI
   helpers into core (that would regress the lib/app layering).
4. **Snapshot strategy:** **fixture-driven** — build `Planned`/`StatusEntry`/
   `Outcome` values in-memory with **fixed fake paths** (`/live/...`,
   `/store/...`), feed the formatters, snapshot the result. No tempdirs, no
   timestamps, no redaction. Use **external `.snap` files** (not inline) under
   the app crate's `snapshots/`.
5. **CI shape:** add **one new ubuntu-only `coverage` job** (tool via
   `taiki-e/install-action`); **drop ubuntu from the plain `test` matrix**
   (the coverage job already runs the full suite on ubuntu) while **keeping the
   macos cell** as the cross-platform plain-nextest check.
6. **Untestable-glue exclusion:** the CI `coverage` job runs on a **nightly
   toolchain** so `#[cfg_attr(coverage_nightly, coverage(off))]` markers
   actually suppress the few untestable spots (`is_root` syscall, `fn main`,
   error-print-then-`exit` sinks). **The plan's earlier `#[cfg(not(coverage))]`
   idea is a bug** — `cargo-llvm-cov` sets `--cfg coverage`, so that attribute
   would *delete* the code from the coverage build. Use the `coverage_nightly`
   marker instead.
7. **Scope when tests find bugs:** **tests-only.** Every new test asserts
   *current* behavior. Suspected bugs are logged in the **Findings** section
   (below) for a follow-up plan — **not** fixed here. The only behavior changes
   allowed are the agreed behavior-preserving test seams (decisions 2, 3, 9).
8. **Measurement boundary:** **exclude `model/generated.rs`** from coverage
   (`--ignore-filename-regex 'generated\.rs'`) — it's typify-generated and
   independently drift-checked, so counting it measures typify, not us. The
   gate is **Rust-only**; the npm launcher (`packages/symify/`) keeps its
   existing `node --test` suite as-is and is **out of the coverage gate**.
9. **`output.rs` write-injection:** the `render_*` functions currently
   `println!` directly and return only a `u8`. Refactor them to write into
   `&mut impl Write` (main passes `io::stdout().lock()`; tests pass a `Vec<u8>`
   and snapshot it). Output bytes stay identical — a test seam under decision 7.
   This is part of Step 2 (it must precede the snapshots).
10. **Toolchain parity:** pin the coverage **npm scripts to nightly**
    (`cargo +nightly llvm-cov …`) and add `nightly` as a managed toolchain
    (mise/rustup) so `npm run coverage:check` is identical locally and in CI —
    no "passes on CI, fails on my machine" from inert stable markers. Stable
    **1.96 remains the toolchain for everything else** (build/test/clippy/fmt);
    nightly is scoped to the coverage subcommand only.

> Both new tools (`cargo-llvm-cov` **0.8.7**, `insta` **1.47.2**) are
> **dependencies** — per CLAUDE.md the versions were checked and approved.
> Re-confirm "latest as of 7 days ago" at implementation time and bump if newer.

## Non-goals

- Windows support or cross-platform test abstraction (stay **Unix-only**, matching
  the current `std::os::unix` symlink code).
- Property-based / fuzz testing (could be a later plan; not required for 90%).
- Performance/benchmark tests for large-file streaming (correctness only).
- Beefing up the npm launcher's node tests (out of the gate; existing tests stay).
- **Fixing** bugs surfaced by new tests (logged as Findings; see decision 7).
- Rewriting existing passing tests except where a snapshot or shared helper
  replaces brittle hand-assertions.

## Current state (baseline inventory)

Counts to confirm at start (`cargo nextest run --workspace` → **109**):

| Area | File | Tests | Notes |
|---|---|---|---|
| clock | `core/src/clock.rs` | 2 | fine |
| config | `core/src/config.rs` | 17 | strong |
| edit | `core/src/edit.rs` | 4 | ok |
| fs | `core/src/fs.rs` | 11 | strong |
| plan | `core/src/plan.rs` | 32 | strong |
| status | `core/src/status.rs` | 3 | ok |
| cli (arg norm) | `app/src/cli.rs` | 6 | fine |
| core integration | `core/tests/integration.rs` | 8 | good |
| CLI e2e | `app/tests/cli.rs` | 26 | good |
| **output** | `app/src/output.rs` | **0** | **gap (591 lines)** |
| **confirm** | `app/src/confirm.rs` | **0** | **gap** |
| **error** | `core/src/error.rs` | **0** | indirect only |
| **main helpers** | `app/src/main.rs` | **0** | indirect only |

First implementation step is to **record the real baseline %** from
`cargo +nightly llvm-cov` (per-crate, with `generated.rs` excluded) so we know
the starting line and can target the gaps with the biggest payoff.

## Behavioral checklist (the qualitative gate)

Each line below must map to at least one named test. Grouped by module.

### output.rs (new — fixture snapshots + unit; needs the Write-injection refactor)
- [ ] Human run output: every `Outcome` variant rendered (AlreadyOk, Applied per
      `ActionKind` Adopt/Relink/Link/Push/Pull, AppliedDrift, Skip, Conflict,
      Disabled, Failed) — snapshot.
- [ ] Human status output: every `StatusLabel` (OK, Unadopted, LiveMissing,
      WrongTarget, Missing, Differs, StoreMissing, Disabled) — snapshot.
- [ ] Human list output (mappings + entries) — snapshot.
- [ ] JSON run output: shape + every counter field (copied, backed_up, pruned,
      drift, changed, conflicts) for representative runs — parse + assert.
- [ ] JSON status/list output — parse + assert.
- [ ] `--dry-run` annotation present in both human and JSON.
- [ ] Symbols/exit-affecting classification (`!` drift, conflict) correct.
- [ ] Empty result set (no mappings / no entries) renders cleanly.
- [ ] Auto-init notice goes to stderr, never pollutes `--json` stdout (already
      tested e2e; add a focused output-level test).

### confirm.rs (new — needs the reader/writer injection seam)
- [ ] No destructive ops → no prompt, proceeds.
- [ ] Destructive ops present + `--yes` → no prompt, proceeds.
- [ ] Destructive ops present, injected "y"/"yes" → proceeds; "n"/""/EOF → aborts.
- [ ] Prompt body lists the destructive deletes (Apply + ApplyDrift, nonempty
      dir prunes) accurately.
- [ ] `--json` / non-TTY mode never blocks — refuses with the `--yes` hint.
- [ ] `dry_run` → Proceed without scanning.

### error.rs
- [ ] `Error::io` and `Error::config` construct + Display formatting.
- [ ] `From`/`?` conversions used in the codebase produce expected variants.
- [ ] Error Display strings are stable enough for the CLI messages that quote them.

### main.rs helpers
- [ ] `root_refused(...)`: all 8 (mutating × root × allow_root) combinations.
- [ ] `is_mutating()` classification for every verb.
- [ ] `derive_key()` for in-root vs out-of-root paths (relative key vs absolute).
- [ ] `sole_mapping()` defaulting + ambiguity error.
- [ ] `--delete` pre-plan override flips mirror on all mappings.

### fs.rs (fill gaps)
- [ ] `is_nonempty_dir()` directly: empty, nonempty, missing, file-not-dir.
- [ ] `dir_entries()` directly: artifact filtering, sorting, missing dir error.
- [ ] Copy into a **read-only destination dir** → clean error (not panic, temp
      cleaned up).
- [ ] `copy_file_atomic` leaves no `*.symify-tmp.*` on failure (inject failure).
- [ ] Symlink **cycle** handled without infinite loop (compare/inspect).
- [ ] Symlink-to-self handled.
- [ ] Special characters in filenames (spaces, unicode, `:`).
- [ ] Broken/dangling symlink inspected and compared correctly.

### plan.rs / status.rs (fill gaps)
- [ ] `guard_reason()` / `resolve_paths()` exercised directly (not just via plan).
- [ ] Empty mapping (no links) is a clean no-op.
- [ ] Path traversal: a link key with `..` cannot escape live root (guard fires).
- [ ] `continue_on_error` aggregates multiple failures across entries.

### config.rs (fill gaps)
- [ ] `home_dir()` / `config_base_dir()` behavior under env overrides.
- [ ] Malformed TOML → `Error::config` with useful message.
- [ ] Unknown/extra keys behavior (reject vs ignore — **assert current contract**;
      if it looks wrong, log a Finding, don't change it).
- [ ] Empty config file (no settings, no mappings).

## Implementation steps

Keep the build green at every step. Order is by payoff: tooling first (so we
measure), then the biggest untested module, then gap-fill, then the gate.

1. **Add tooling + baseline.**
   - mise: `"cargo:cargo-llvm-cov" = "0.8.7"` and a managed `nightly` toolchain
     (with `llvm-tools-preview`) for the coverage subcommand only.
   - `package.json` scripts (all `+nightly`, `generated.rs` excluded):
     - `coverage`: `cargo +nightly llvm-cov nextest --workspace --ignore-filename-regex 'generated\.rs' --html`
     - `coverage:summary`: `… --summary-only` (prints line **and** region)
     - `coverage:check`: per-crate line gate — either two scoped runs
       (`-p symify-core` / `-p symify`, each `--fail-under-lines 90`) or one
       `--json` run parsed per-package. Pick whichever the pinned llvm-cov
       supports cleanly; document the choice.
   - `.gitignore`: coverage artifacts (`target/llvm-cov*`, `*.profraw`).
   - Record the **baseline per-crate %** in the PR description and DoD notes.

2. **output.rs Write-injection + insta snapshots.**
   - Refactor `render_run`/`render_status`/`render_add`/`render_remove`/
     `render_list` (and `print_line`/`print_run_summary`) to write into
     `&mut impl Write`; `main` passes `io::stdout().lock()`. Verify byte-identical
     output (existing e2e tests are the safety net).
   - Add `insta = "1.47.2"` to the app crate's dev-dependencies.
   - Fixture-driven file snapshots for every human case; parse+assert JSON.
     Commit `cargo insta accept`ed `.snap` files.

3. **confirm.rs seam + tests.** Refactor `gate()` to inject reader/writer; cover
   accept/decline/EOF, `--yes`, dry_run, empty, json/non-tty refusal, and the
   pure `destructive_deletes()`.

4. **main.rs helper tests.** Extract `root_refused`; unit-test it + `is_mutating`,
   `derive_key`, `sole_mapping`, `--delete` override. IO-bound `do_restore`/
   `load_set` via tempdir or left to existing e2e.

5. **error.rs tests.** Constructors, Display, conversions.

6. **fs.rs gap-fill.** `is_nonempty_dir`, `dir_entries`, read-only dest dir,
   temp-cleanup-on-failure, symlink cycle/self, special chars, dangling links.

7. **plan/status/config gap-fill.** Direct helper tests, empty/malformed inputs,
   path-traversal guard, continue-on-error aggregation.

8. **Close the last % gaps.** Re-run `coverage:summary`, find remaining uncovered
   regions, add targeted tests until each crate's line coverage ≥ 90%. Mark
   genuinely-untestable glue with `#[cfg_attr(coverage_nightly, coverage(off))]`
   (never `#[cfg(not(coverage))]`) — and only with a one-line justification.

9. **CI gate.** Add the ubuntu-only nightly `coverage` job running
   `coverage:check`; drop ubuntu from the `test` matrix (keep macos). Sits
   alongside `clippy` / `fmt` / `schema` (codegen drift).

## Findings (surfaced while testing — for a follow-up, NOT fixed here)

- **No behavioural bugs found.** Every characterization test matched the code's
  intent; nothing needed a fix.
- **Contract was stricter than assumed (now locked):** unknown TOML keys are
  *rejected*, not ignored — the schema sets `additionalProperties: false`, so a
  typo'd setting fails loudly. Locked by `config::tests::unknown_keys_are_rejected`.
- **Tooling note — e2e subprocess coverage is partial.** `cargo-llvm-cov` does
  not fully attribute coverage from the real binary spawned by the `assert_cmd`
  e2e tests, so a few `main.rs` error branches (e.g. `run_add`'s "no such file",
  unknown-mapping) read as uncovered even though the e2e suite exercises them.
  This is a measurement limitation, not a real gap; the app crate still clears
  90% comfortably. If we ever want those counted, set
  `LLVM_PROFILE_FILE`/`--include-build-script`-style subprocess capture.
- **Coverage markers needed only on `is_root`.** Both crates clear 90% with all
  glue counted, so the only `#[cfg_attr(coverage_nightly, coverage(off))]` applied
  is on the `geteuid` syscall wrapper (genuinely unrunnable without being root).
  `fn main` and the run dispatch are covered by the e2e subprocess.
- **`coverage:check` must `clean` first.** Consecutive `cargo llvm-cov … --no-report`
  runs *accumulate* profraw (that's the point of `--no-report`), so the script and
  CI job run `cargo llvm-cov clean --workspace` before measuring to avoid stale
  data from a previous build inflating/poisoning the numbers.

## Risks & mitigations

- **Flaky mtime-based tests.** Sync quick-check uses mtime; coverage runs are
  slower and may shift timing. Use explicit `set_mtime` helpers (already exist in
  plan tests), never wall-clock sleeps.
- **llvm-cov + nextest interaction.** `cargo llvm-cov nextest` is supported but
  needs `llvm-tools-preview`; the coverage job/scripts run on nightly (decision
  6/10). Document in devcontainer post-create if missing.
- **Per-crate gating mechanics.** `--fail-under-lines` applies to the whole run;
  per-crate enforcement needs either scoped `-p` runs or JSON parsing. Settle the
  exact mechanism in Step 1 and document it.
- **Snapshot churn.** Output format changes update many `.snap` files; that's
  intended (review the diff). Keep snapshots small and focused per case.
- **90% per-crate may be tight for the binary** (process-exit/glue). The
  Write-injection (output) and `root_refused`/confirm seams move most logic into
  testable territory; the residue is marked `coverage(off)` and justified.
- **Local/CI parity.** Inert markers on stable would under-report; the scripts
  pin `+nightly` (decision 10) so local matches CI.

## Definition of done

- [x] `cargo-llvm-cov` (0.8.7, + nightly toolchain) and `insta` (1.47) added.
- [x] `npm run coverage`, `coverage:summary`, `coverage:check` work (and agree
      with CI because they pin nightly + `clean` first).
- [x] Baseline and final per-crate coverage % recorded (below).
- [x] **Line** coverage **≥ 90% per-crate**: `symify-core` 96.1%, `symify` 94.9%
      (`generated.rs` excluded); region reported (core 93.6%, app 92.6%).
- [x] Every behavioral-checklist item has a named, passing test.
- [x] `output.rs` and `confirm.rs` go from 0 tests to fully covered (96.9% / 98.5%).
- [x] CI runs `coverage:check` (ubuntu nightly) and fails under threshold; macos
      plain-nextest cell retained.
- [x] No behavior changes beyond the agreed test seams; findings captured above.
- [x] `npm test` (nextest, 158 tests), clippy `-D warnings`, `fmt`, and
      `codegen:check` all green.
- [x] The one `coverage(off)` exclusion (`is_root`) carries a justification.

### Coverage results (line, per-crate, generated.rs excluded)

| Crate | Baseline | Final |
|---|---|---|
| `symify-core` | 94.78% | **96.1%** |
| `symify` (app) | 78.27% | **94.9%** |

Test count: 109 → **158** (+49). Biggest movers: `output.rs` 73% → 96.9%,
`confirm.rs` 69% → 98.5%, `error.rs` 33% → 100%, `model/mod.rs` 70% → 100%.

## Environment notes

- Tests run via `cargo nextest run --workspace`; coverage via
  `cargo +nightly llvm-cov nextest …`. Both managed through mise.
- Coverage needs a nightly toolchain with `llvm-tools-preview`.
- Stable **1.96** remains the toolchain for build/test/clippy/fmt; nightly is
  scoped to the coverage subcommand only.
- Commit signing key may be absent in-container; commit with
  `-c commit.gpgsign=false` if signing fails.
