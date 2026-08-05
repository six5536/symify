---
type: Reference
id: api-contracts
title: API Contracts
description: The CLI surface and flags, config-mutation and auto-init behaviour, status labels, JSON output, and the stability promises.
status: stable
sources:
  - id: cli-src
    resource: /crates/app/symify/src/cli.rs
    title: CLI definition
  - id: output-src
    resource: /crates/app/symify/src/output.rs
    title: Output rendering
links:
  - rel: references
    to: architecture
    note: The state machine behind these verbs.
  - rel: references
    to: error-handling
    note: Exit codes and failure semantics.
---

# CLI surface

```
symify <path>          shortcut for `symify add <path> …` (see below)
symify add    <path>   [-m MAP] [--store-path P] [-c FILE]... [--force] [--dry-run] [-y] [--json]
symify remove <path>   [-m MAP] [-c FILE]... [--no-restore] [--dry-run] [--json]   (alias: rm)
symify list            [-m MAP]... [-c FILE]... [--entries] [--json]               (alias: ls)
symify sync            [-m MAP]... [-c FILE]... [--dry-run] [-y] [--checksum] [--modify-window N] [--json]
symify deploy          [-m MAP]... [-c FILE]... [--dry-run] [-y] [--checksum] [--modify-window N] [--json]
symify status          [-m MAP]... [-c FILE]... [--checksum] [--modify-window N] [--json]
symify diff            [-m MAP]... [-c FILE]... [--checksum] [--modify-window N] [--json]
symify completions <SHELL>   (bash | zsh | fish | powershell | elvish)
symify man             (hidden; roff to stdout, for packaging)

Global: --allow-root  (permit mutating verbs to run as root; refused otherwise)
        -V, --version (print `symify x.y.z` and exit)

Bare `symify` (no command) prints help and exits 0.
```

- The only positional is `<path>` on `add`/`remove` — a real filesystem path
  (CWD-relative / `~` / absolute) from which the config key is derived
  (relativized against the mapping's `live`; an absolute key if outside it).
- **Bare-path shortcut.** `symify <path> …` is rewritten to
  `symify add <path> …`, since adding is the common case. The `add` is
  inserted before the first non-flag token unless that token is a known
  subcommand or alias; a leading `--` disables the rewrite. So `symify status`
  is still the `status` verb — use `symify add status` to track a file
  literally named `status`.
- `-V, --version` — global; prints `symify x.y.z` and exits. Rendered by hand
  rather than by clap's built-in flag, so it stays `global = true` (i.e.
  `symify sync -V` works).
- `completions <shell>` — writes a completion script to stdout. Generated from
  the clap definition, so it cannot drift from the CLI. `man` does the same
  for a roff man page and is hidden: it exists for packaging, not daily use.
  Both names are in the `SUBCOMMANDS` shadow list, without which the bare-path
  shortcut would rewrite `symify completions bash` to
  `symify add completions bash`. Both also render into a buffer before
  writing it out, because `clap_complete` panics rather than returning an
  error when a write fails.
- `-m, --mapping` — repeatable filter on the run/query verbs (omit = all); a
  single value on `add`/`remove` defaulting to the sole mapping. Unknown name:
  `add` creates the mapping, every other verb errors.
- **Inactive mappings** (an `os`/`host` condition not matching this machine —
  see [configuration](configuration.md)): run/query verbs skip them with a
  one-line `mapping <name>: inactive (<os|host>)` note and exit 0, even when
  named with `-m`; `list` marks them; `--json` reports them as
  `inactive_mappings: [{ mapping, inactive, reason }]` (run/status/diff) or an
  `inactive` field per mapping (`list`). `add`/`remove` refuse an inactive
  mapping — their adopt/restore half cannot act on this machine.
- `-c, --config <file>` — the repeatable config set; replaces default
  discovery and suppresses auto-init.
- `--dry-run` — `sync`/`deploy`/`add`/`remove`; plan/report without mutating.
- `-y, --yes` — pre-approve unrecoverable recursive deletes
  (`sync`/`deploy`/`add`). Required for such a plan when not on a TTY (piped /
  `--json` / CI).
- `--allow-root` — global; permit a mutating verb to run as root (otherwise
  refused).
- `--json` — machine-readable output (every config-reading verb; not
  `completions`/`man`, which read no config and take neither `--json` nor
  `-c`).

# Config mutation (`add` / `remove`)

`add`/`remove` edit config files with `toml_edit`, preserving comments,
ordering, and the `#:schema` line. They auto-locate across the config set:
`remove` clears the key from **every** file that defines it; `add` writes
beside an existing mapping (highest-precedence file if split) or creates a new
mapping in the primary file. `add` then **adopts** the new entry (a one-entry
`sync`); `remove` **restores** a standalone independent copy at the live path
by default (`--no-restore` to skip). Both honour `--dry-run`.

# Auto-init

There is no `init` verb. In default mode (no `-c`), when the default config is
absent, every config-reading verb first creates it from the starter template
(`live = ~`, `store = ~/dotfiles`, a `dotfiles` mapping; with a `#:schema`
line for editor validation), printing `Created <path> (defaults).`, then
proceeds. An explicitly-named `-c <file>` that is missing stays an error —
auto-init never fabricates a file the user named.

# `diff` reporting

Read-only, the content view of `status`: per non-clean entry a header line,
then unified diffs (store as `-`, live as `+`, headers naming both absolute
paths) for changed files and `only in live/store:` lines for one-sided ones —
the union walk matches what the quick-check calls a difference. Symlinks
compare by target; kind mismatches, binary content, and files over 1 MiB get
a one-line summary instead of hunks; byte-identical files that fail the
quick-check report `contents identical (metadata differs)`. `--json` carries
per-file `{live, store, state}` (state: `differs` | `live-only` |
`store-only`) with no content hunks. Exit codes as `status`. `diff` is in the
bare-path shortcut's shadow list; `symify add diff` tracks a file named
`diff`.

# `status` reporting

Read-only, direction-neutral. Per entry it reports one `StatusLabel`, never
claiming which direction you should run. The states behind the labels are the
[architecture](architecture.md) state machine's.

| Label | `symlink` | `copy` | Meaning |
|---|---|---|---|
| `ok` | ✓ | ✓ | A correct link; or, in copy mode, content in sync. |
| `unadopted` | ✓ | — | `S` is a real file, not yet a link. |
| `wrong-target` | ✓ | — | `S` is a symlink pointing somewhere else. |
| `differs` | — | ✓ | Both sides exist but differ (quick-check or `--checksum`). |
| `live-missing` | ✓ | ✓ | `S` is absent. |
| `store-missing` | ✓ | ✓ | `D` is absent. |
| `missing` | ✓ | ✓ | Both sides are absent. |
| `disabled` | ✓ | ✓ | `value = false`. |
| `failed` | ✓ | ✓ | Unusable, e.g. refused by a planner guard. |

`disabled` counts towards the `ok` total in the summary line; only drift and
failures move the exit code off `0` — see
[error-handling](error-handling.md) for the exit-code contract.

# Stability

Pre-1.0: minor versions may carry breaking changes to any of the above.
`symify-core`'s Rust API is **not** stable yet; the CLI and `--json` output
are the intended integration surfaces.
