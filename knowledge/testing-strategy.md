---
type: Reference
id: testing-strategy
title: Testing Strategy
description: The test layers from planner units to CLI end-to-end, the key choices behind them, and the CI platforms.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (test layers and commands)
  - id: plan-src
    resource: /crates/lib/symify-core/src/plan.rs
    title: Planner source
links:
  - rel: references
    to: architecture
    note: The pure planner and injected clock exist largely for testability.
---

The pure planner and the injected clock of [architecture](architecture.md)
exist largely for testability. Tests run under `cargo-nextest` (`npm test`);
the commands and the coverage gate are in [CONTRIBUTING](/CONTRIBUTING.md).

# Layers

- **Planner (unit, the bulk).** The planner is `(merged config + FS state) →
  Vec<Action>`. The per-entry state-machine table is the test matrix: each
  row × mode × verb is a case. Fast; covers the logic that matters most.
- **Executor / library integration.** Lay down a `live` + `store` fixture
  tree, run `sync`/`deploy`/`status` through the `symify-core` API, and
  assert the resulting filesystem: link exists and **resolves** to the right
  target, content matches for `sync` (and mtime is preserved), `.bak` created
  on conflict, only changed files recopied.
- **CLI end-to-end.** Invoke the real binary against temp trees; assert human
  output, `--json` output, and exit codes (`0`/`1`/`2`).
- **Config / merge (table-driven).** TOML strings in, merged config out —
  deep merge, `-c` replace, `conf.d` ordering.
- **Schema codegen.** CI drift guard (regenerate, fail on diff); example
  configs round-trip through the generated types.
- **npm launcher.** A JS test that resolves + spawns a stub binary, and
  errors cleanly when no platform package matches.

# Key choices

- **Real temp directories** (`tempfile`), not a mocked FS. symify's entire
  job is real filesystem semantics (symlink resolution, inode sharing,
  hashing); an in-memory FS would mean re-implementing the OS and would hide
  exactly the bugs that matter. The planner/executor already take root
  *paths*, so the test seam is "point them at a temp tree" — no
  FS-abstraction trait.
- **Injected clock.** A `now` provider keeps `Date::now`-style calls out of
  the pure layers and lets tests pin `.<timestamp>.bak` names for exact
  assertions.
- **Explicit assertions** on human output (key lines / summary) and
  parsed-JSON assertions for `--json`. A small fixture-builder helper (DSL to
  lay down live/store trees) keeps state-machine cases readable.
- **Idempotency invariant.** A dedicated test runs each verb twice and
  asserts the second run is all-`AlreadyOk` with exit `0` — catches a class
  of planner bugs.
- **Partial-failure.** Drive a real failure (e.g. an entry refused by the
  safety guard) and assert other entries still apply and the run exits `2` —
  exercising continue-on-error without mocking.

# CI platforms

Tests run on **Linux, macOS, and Windows**. Test symlinks go through a
per-module cross-platform shim (GitHub's Windows runners execute elevated, so
symlink creation works there); permission-bits tests are `#[cfg(unix)]` —
Unix mode bits have no Windows meaning.
