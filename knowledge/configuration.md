---
type: Reference
id: configuration
title: Configuration & Environments
description: Config file structure, discovery and merge order, os/host machine conditions, backup retention, and the JSON Schema that generates the Rust model and drives editor validation.
status: stable
sources:
  - id: config-src
    resource: /crates/lib/symify-core/src/config.rs
    title: Config loader source
  - id: schema
    resource: /schema/symify.schema.json
    title: Config JSON Schema
links:
  - rel: references
    to: api-contracts
    note: Auto-init and -c/--config behaviour at the CLI.
  - rel: references
    to: architectural-rules
    note: The backup_keep discovery carve-out.
---

# Roots and structure

```toml
[settings]
live = "~"            # working location (links/copies appear here)
store = "~/dotfiles"  # backing repository (real content lives here)
mode = "symlink"      # symlink | copy
conflict = "backup"   # skip | replace | backup
# backup_keep = 5     # cap on .bak files per path; absent/0 = keep all

[mappings.dotfiles]
# optional per-mapping overrides of live / store / mode / conflict,
# and optional os / host machine conditions

[mappings.dotfiles.links]
# entries: key = value, resolved per architecture's link-resolution rules
```

`[settings]` provides defaults; each `[mappings.<name>]` may override `live`,
`store`, `mode`, `conflict`, and `backup_keep`. Paths support `~` and
environment-variable expansion (Windows-aware) and are normalised to absolute
paths before planning.

`backup_keep = N` bounds the `<name>.<timestamp>.bak` files: when a new
backup is written, the oldest beyond `N` (the new one included) are planned
as removes — visible in `--dry-run`, and gated by the delete confirmation
when a backup is a non-empty directory. Opt-in: absent or `0` keeps every
backup. Only exact-pattern siblings of the entry's own leaf are touched — the
carve-out recorded in [architectural-rules](architectural-rules.md).

# Machine conditions (`os` / `host`)

A mapping may carry `os` and/or `host` (each a string or non-empty list). At
resolve time they are matched against an injected `MachineContext` (the
binary fills it from `std::env::consts::OS` plus `gethostname(2)` on Unix or
`GetComputerNameExW`'s DNS hostname on Windows — not the 15-character
NetBIOS `COMPUTERNAME`; tests pin it). A non-matching mapping is **inactive**: the planner and `status` skip
it entirely and the binary renders a one-line note — see
[api-contracts](api-contracts.md) for the verb behaviour.

- `os`: matched against `linux` | `macos` | `windows`, case-insensitively; no
  globs. An unknown value is legal and never matches.
- `host`: matched case-insensitively against the hostname; `*` may open
  and/or close a pattern (`wrk-*`, `*.corp`), never sit mid-pattern.
- Both present ⇒ AND. A mid-pattern `*`, an empty list, or an empty pattern
  is a config error.

# Loading and merge order

Loaded and merged in order; later sources override earlier ones:

1. Default: `~/.config/symify/symify.toml`
2. `~/.config/symify/conf.d/*.toml` (lexicographically sorted)
3. CLI `-c/--config` files (repeatable)

When any `-c` is given it **replaces** default discovery (steps 1–2) — "run
exactly this config." How `-c` interacts with auto-init is defined in
[api-contracts](api-contracts.md). Merge granularity within the active set:

- `[settings]` — per-key deep merge (a drop-in can flip just `conflict`).
- `[mappings]` — distinct names accumulate; same-named mappings deep-merge
  (their `links` combine, later file wins per duplicate key; later `mode` /
  `conflict` / root overrides apply).

# Config schema codegen

The config data model has a single source of truth in a **hand-authored JSON
Schema** (`schema/symify.schema.json`). Rust types are generated from it, and
the same file drives editor TOML validation. A plain `cargo build` requires
neither Node nor `typify` (the generated Rust is committed).

```
schema/symify.schema.json  (hand-authored, source of truth)
        │
        ├──(cargo-typify, dev/CI only)──▶ checked-in generated Rust
        │                                  (symify-core::model::generated, CI drift guard)
        │
        └────────────────────────────────▶ editor TOML validation (taplo / VS Code)
```

- **JSON Schema → Rust**: `cargo typify` generates serde types into the
  **checked-in** `crates/lib/symify-core/src/model/generated.rs` (regen via
  `npm run codegen`; drift-checked in CI via `npm run codegen:check`). No
  build-time codegen, no `typify` build-dependency, reviewable diffs.
- **Editor DX**: the same schema drives TOML validation in editors.
- At runtime TOML is validated structurally by serde in Rust (the schema uses
  `additionalProperties: false`, so generated structs get
  `deny_unknown_fields`); the JSON Schema is the editor-time validator.

The schema models `links` and `mappings` as objects with
`additionalProperties` (maps), the link value as `oneOf:[string, boolean]`
(generates a clean untagged `LinkValue` enum), and `mode`/`conflict` as string
enums.

**History:** the model was originally specified in TypeSpec, but the
`@typespec/json-schema` emitter produces a `$id`-per-type bundle using
`unevaluatedProperties` and nested per-subschema `$defs` that `typify` cannot
ingest. `typify` works cleanly on a hand-authored schema, so TypeSpec was
dropped in favour of authoring the JSON Schema directly.
