---
type: Glossary
id: glossary
title: Domain Glossary
description: The project's terms of art, each defined in one or two lines.
status: stable
sources:
  - id: plan-src
    resource: /crates/lib/symify-core/src/plan.rs
    title: Planner source
  - id: schema
    resource: /schema/symify.schema.json
    title: Config JSON Schema
links:
  - rel: references
    to: architecture
    note: Where most of these terms are defined in context.
---

Terms as used throughout the project; mechanics in
[architecture](architecture.md).

- **live** — the working location where files are actually used (e.g. `~`). A
  location, never a role; direction belongs to the verb.
- **store** — the managed backing repository holding the real content (e.g.
  `~/dotfiles`), typically under version control.
- **mapping** — a named `[mappings.<name>]` config section: a set of link
  entries plus optional overrides of `live`/`store`/`mode`/`conflict`.
- **entry** — one `key = value` line under `[mappings.<name>.links]`,
  resolving to an `(S, D)` pair.
- **`S` / `D`** — an entry's resolved live-side path (link/copy location) and
  store-side path (real content).
- **mode** — the mechanism per entry: `symlink` (a link at `S` pointing to
  `D`) or `copy` (independent copies on both sides; named `sync` before
  PLAN-008).
- **verb** — the direction-owning command: `add`, `remove`, `list`, `sync`,
  `deploy`, `status`, `diff`.
- **adopt** — move a real live file into the store and replace it with a
  link; the central first-run workflow.
- **relink** — replace a live file whose content already matches the store
  with a link, no backup (nothing distinct to preserve).
- **mirror entry** — an entry whose value is `""` or `true`: `D` is `key`
  joined under the store root.
- **disabled entry** — an entry whose value is `false`; counted as `ok` in
  summaries.
- **inactive mapping** — a mapping whose `os`/`host` condition does not match
  this machine; skipped from planning and reported as a one-line note, never
  per entry.
- **conflict** — both sides exist and the entry is not in its desired state;
  resolved by the `skip` | `replace` | `backup` policy.
- **backup** — the `<name>.<timestamp>.bak` rename protecting whichever side
  is about to be overwritten (timestamp `YYYYMMDDHHMMSS`).
- **drift** — a difference the run did not resolve; drives exit code `1`.
- **quick-check** — the default `copy`-mode equality test: size + mtime +
  permission bits, per file. `--checksum` upgrades it to a BLAKE3 content
  compare.
- **additive** — the `copy`-mode property: only changed files are
  copied, destination-only files are never deleted.
- **AlreadyOk** — the planner's no-op action for an entry already in its
  desired state.
- **auto-init** — any config-reading verb creates the default config from the
  starter template when none exists; there is no `init` verb.
- **bare-path shortcut** — `symify <path>` rewrites to `symify add <path>`
  unless the first token is a known subcommand.
