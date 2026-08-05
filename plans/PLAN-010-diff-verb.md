# Plan: `symify diff` verb

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

A read-only verb showing *what* differs, where `status` only says *that* it
differs. The natural step between `status` and `sync`/`deploy`. The planner
already computes per-file differences; this renders them.

```
symify diff [-m MAP]... [-c FILE]... [--checksum] [--modify-window N] [--json]
```

## Resolved design decisions (grilled 2026-08-05 — do not re-litigate)

1. **`similar` crate approved** for unified diffs (dependency-policy approval
   given; it is already in the tree's dev-dependency cone via `insta`).
   Latest version at add time; record in `knowledge/technology-stack.md`.
2. **New `diff` verb**, not `status --diff` — mirrors the `git diff`/`git
   status` mental model and keeps `status`'s at-a-glance contract. Costs the
   `SUBCOMMANDS` shadow-list entry (tested).
3. **Polarity: store is old (`-`), live is new (`+`)** — the diff reads as
   "what your live edits would push into the store", matching the daily
   edit-then-sync flow and git's worktree convention. Headers name both full
   paths. No direction flag.

## Functional requirements

- Per entry, by state:
  - copy-mode `differs`: a unified content diff per changed file (`-` store,
    `+` live, headers naming both full paths); binary files get a one-line
    `binary files differ (N ↔ M bytes)`.
  - `unadopted` / symlink-content cases: unified diff of live file vs store
    file when both exist and differ; otherwise a one-line summary.
  - `wrong-target`: `expected <D>, actual <target>` one-liner.
  - `live-missing` / `store-missing` / `missing`: one-line summary (whole-file
    diffs for missing sides add noise, not information).
  - `ok` / `disabled`: silent.
- Exit codes as `status`: `0` clean, `1` drift, `2` error.
- `--json`: per-entry, per-file structured list (paths, state, equal/differs)
  — **no content hunks in v1**.
- Read-only: never mutates, never prompts, allowed as root (like `status`).

## Non-functional requirements

- **Planner stays pure**: it identifies differing pairs (it already does);
  file contents are read and rendered in the binary's output layer.
- **Broken-pipe rule**: diff output will be paged (`| less`); EPIPE ends the
  run at exit 0, per [error-handling](/knowledge/error-handling.md).
- Completions and the man page regenerate from the clap definition (free).

## Design decisions

1. **`diff` must join the `SUBCOMMANDS` shadow list** in the bare-path
   shortcut, or `symify diff` rewrites to `symify add diff`. Test this.
2. Large-file guard: skip content diff above a size threshold (1 MiB) with a
   summary line — `diff` is for dotfiles, not databases.
3. No colour in v1 (no terminal-detection machinery exists today; add later
   if wanted).
4. Security posture: read-only, so planner guards apply as in `status`
   (guarded entries render as `failed`); no confirmation gate involvement.

## Work breakdown

- **A. Dependency**: add `similar` (approved above); record in
  `knowledge/technology-stack.md`.
- **B. CLI**: `Diff` subcommand sharing the query-verb args; shadow-list
  entry.
- **C. Core**: expose the per-entry/per-file pair list from the plan (likely
  already present in `Action` ops; add an accessor if not). No new planning
  logic.
- **D. Binary**: render human diffs in `output`; `--json` variant.
- **E. Tests**: CLI end-to-end on fixture trees (changed file, binary file,
  wrong-target, missing sides); bare-path shortcut regression; exit codes;
  `--json` shape; EPIPE behaviour if practically testable.
- **F. Docs/KB**: README command table, knowledgebase (`api-contracts` CLI
  surface + verbs table in `architecture`, `glossary`, `error-handling` if
  exit-code prose enumerates verbs).

## Critical files

- `crates/app/symify/src/{cli,main,output}.rs`
- `crates/lib/symify-core/src/{plan,status}.rs` (accessor only)
- `README.md`, `knowledge/{api-contracts,architecture,glossary}.md`

## Definition of done

- [x] All entry states render as specified; binary and oversize files
      summarised, not dumped.
- [x] `symify diff` is not captured by the bare-path shortcut (test).
- [x] Exit codes match `status` on the same tree (test asserts both).
- [x] Planner untouched by rendering; no mutation anywhere in the verb.
- [x] All gates green; knowledgebase validator passes.
