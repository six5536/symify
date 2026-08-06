---
type: Convention
id: error-handling
title: Error Handling & Logging
description: Exit codes, continue-on-error execution, per-entry outcome reporting, and the broken-pipe rule.
status: stable
sources:
  - id: main-src
    resource: /crates/app/symify/src/main.rs
    title: Exit-code classification
  - id: error-src
    resource: /crates/lib/symify-core/src/error.rs
    title: Error type
---

# Execution semantics

Entries are independent — the executor **continues on error**, attempts every
entry, and reports a per-entry outcome. There is **no rollback** (backups are
the recovery path). Parent directories are created automatically on the side
being written. symify never *prunes* directories (it does not clean up empty
parents it created); it only acts on each entry's own leaf path — which a
`replace` policy or a `relink` may delete or move, per the entry's resolved
action.

Errors carry a `thiserror`-derived error type in `symify-core`; the binary
renders them as `error: <message>` on stderr. There is no log file or
verbosity system; reporting is the per-entry human or `--json` output.

# Exit codes

- `0` — success / clean.
- `1` — drift: for `status`/`diff`, any entry out of sync; for `sync`/`deploy`, an
  unresolved `skip` conflict.
- `2` — error: one or more entries failed, or a config/IO error.

A closed stdout pipe is the one I/O failure that is not an error: a reader
that stops early (`| head`, a pager quit) ends the run at `0`, silently. That
does mean a truncated `status` reports `0` rather than the drift it never
finished counting.
