---
type: Reference
id: development-commands
title: Development Commands
description: The npm-script command set and the pre-PR check list's shape.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (everyday commands, authoritative list)
  - id: package-json
    resource: /package.json
    title: Script definitions
---

Everything is wrapped as npm scripts (defined in
[package.json](/package.json); the authoritative annotated list is in
[CONTRIBUTING](/CONTRIBUTING.md)):

- Dailies: `npm run build` / `test` / `lint` / `fmt` / `check` — thin wrappers
  over cargo (`test` is `cargo nextest run --workspace` followed by
  `check:aokf`).
- Knowledgebase: `npm run check:aokf` validates the `knowledge/` AOKF bundle
  with `.agents/aokf/tools/validator.py`.
- Codegen: `npm run codegen` regenerates the Rust config model from the JSON
  Schema; `codegen:check` fails on drift.
- Coverage: `npm run coverage` (HTML) / `coverage:summary` /
  `coverage:check` (the ≥90%-per-crate gate; needs the nightly toolchain).
- Packaging: `npm run test:launcher`, `npm run verify-version` (16 locations
  must agree), `npm run release <version>` (bumps, verifies, commits, tags —
  never pushes).
- Smoke: `npm run smoke` runs a release binary through an adopt round-trip;
  `npm run smoke:launcher` npm-packs the launcher plus the host's platform
  package and runs the real binary through the shim. Release CI runs both
  per buildable target.

Two traps:

- `npm run lint` is only `cargo clippy --workspace`; CI runs clippy with
  `--all-targets -- -D warnings` plus fmt-check, doctests, rustdoc `-D
  warnings`, codegen drift, launcher tests, version consistency, and the
  coverage gate. Before a PR, run the full list in CONTRIBUTING, not the
  dailies.
- Only the launcher package is an npm workspace. The five platform-binary
  packages deliberately are not (npm enforces their `os`/`cpu` fields on
  workspace members, breaking `npm install` on every host); tooling addresses
  them by path.
