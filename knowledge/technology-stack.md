---
type: Reference
id: technology-stack
title: Technology Stack
description: Languages, runtime and dev dependencies, and the pinned toolchain set.
status: stable
sources:
  - id: mise
    resource: /.mise.toml
    title: Tool pins
  - id: rust-toolchain
    resource: /rust-toolchain.toml
    title: Rust toolchain pin
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (setup detail)
links:
  - rel: references
    to: software-components
    note: Why musl and zigbuild are in the build path.
---

Rust core (library + binary) with a Node launcher layer for npm
distribution. Toolchains are pinned: `rust-toolchain.toml` pins the project
Rust (with `rustfmt` and `clippy`); `.mise.toml` pins Node, a nightly Rust
used only by the coverage job, and the cargo tooling below. Setup detail
lives in [CONTRIBUTING](/CONTRIBUTING.md).

Adding a dependency requires explicit approval and the latest version at the
time, per the dependency policy. The current set:

- **Rust**: `clap` (derive), `clap_complete` + `clap_mangen` (completions and
  man page generated from the same clap definition), `serde`, `serde_json`,
  `toml`, `toml_edit` (format-preserving config edits), `thiserror`,
  `directories` (Windows-aware home/dirs), `blake3` (`sync`-mode content
  equality).
- **Rust (dev)**: `tempfile`, `assert_cmd` for CLI tests, `insta` for
  snapshot assertions on human output.
- **Tooling** (pinned in `.mise.toml`): `cargo-typify` (schema → Rust
  codegen), `cargo-zigbuild` and `zig` (the cross C compiler the musl targets
  need — see [software-components](software-components.md)),
  `cargo-nextest`, `cargo-llvm-cov`.
