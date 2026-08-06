---
type: Convention
id: coding-standards
title: Coding Standards
description: Prose rules, Rust and TypeScript conventions, and the code-is-canonical principle.
status: stable
sources:
  - id: prose
    resource: /.agents/PROSE.md
    title: Prose rules
  - id: coding
    resource: /.agents/CODING.md
    title: Coding behaviour rules
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (documentation expectations)
---

# Approach

The behavioural rules are in [CODING.md](/.agents/CODING.md): think before
coding, simplicity first, surgical changes only, and verifiable success
criteria defined before executing.

# Prose

Be concise without losing information; use plain language. British English
spelling (`behaviour`, `normalise`). The full rules are in
[PROSE.md](/.agents/PROSE.md); the core: one idea per sentence, no filler or
preamble, comments explain *why* never *what*, no hedging unless genuinely
uncertain.

# Rust

- `rustfmt` formatting, checked in CI (`cargo fmt --all -- --check`).
- Clippy clean at `-D warnings`, all targets.
- Public items in `symify-core` need doc comments (`#![warn(missing_docs)]`);
  rustdoc builds clean under `RUSTDOCFLAGS=-D warnings`; rustdoc examples run
  as doctests.
- Schema fields get a `description`; it becomes both the rustdoc and the
  editor tooltip.

# TypeScript / JavaScript

- Only use `index.ts` when necessary; otherwise name files descriptively.

# The code is the canonical reference

README, CLI `--help`, schema descriptions, and this knowledgebase all
describe actual behaviour. When a doc disagrees with the code, fix the doc —
unless the code is wrong, in which case fix the code and say so.
