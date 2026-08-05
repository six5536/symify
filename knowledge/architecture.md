---
type: Reference
id: architecture
title: Architecture
description: Locations, verbs × modes, the pure-planner pipeline, the per-entry state machine, and link resolution.
status: stable
sources:
  - id: plan-src
    resource: /crates/lib/symify-core/src/plan.rs
    title: Planner source
  - id: fs-src
    resource: /crates/lib/symify-core/src/fs.rs
    title: Executor source
links:
  - rel: references
    to: configuration
    note: Config structure and merge order that feed the planner.
  - rel: references
    to: api-contracts
    note: The CLI surface over this machinery.
---

symify keeps files in a working location in sync with a managed backing
repository, using symlinks or copies. Dotfiles are the common use, but it
applies to any files to be mirrored, backed up, or deployed across machines.

# Mental model

Two location roots:

- **`live`** — the working location where files are actually used (e.g. `~`).
- **`store`** — the managed backing repository that holds the real content
  (e.g. `~/dotfiles`), typically under version control.

In `symlink` mode the **link always lives at `live`** and points to the real
file in `store`. **Adopting** existing live files into the store is the central
workflow, not an add-on: the first run on an existing machine pulls your real
files into `store` and replaces them with links; a fresh machine deploys the
store back out into `live`.

Direction is owned by the **verb**, not by the field names — `live` and `store`
name *locations*, never roles.

# Two orthogonal axes: verb (direction) × mode (mechanism)

**Verbs — direction:**

| Verb     | Direction        | Purpose |
|----------|------------------|---------|
| `add`    | `live` → `store` | Track an existing file: edit the config, then adopt it. |
| `remove` | —                | Stop tracking a file: edit the config, restoring a standalone copy by default. |
| `list`   | read-only        | List mappings and where they point. |
| `sync`   | `live` → `store` | Capture/push live files into the store (includes adoption). |
| `deploy` | `store` → `live` | Install the store onto a machine. |
| `status` | read-only        | Report per-entry state; never mutates. |
| `diff`   | read-only        | Show what differs as content diffs; never mutates. |

**Modes — mechanism** (`mode = symlink | copy`):

- `symlink` (default) — a symbolic link at `live` pointing to the file in
  `store`.
- `copy` — an independent content **copy** (no link), kept up to date
  incrementally (rsync-style: only changed files are copied). Named `sync`
  before PLAN-008 renamed it to end the collision with the `sync` verb.

`copy`-mode copies are governed by two per-run flags: `--checksum` (exact
content compare instead of size+mtime) and `--modify-window <SECONDS>` (mtime
tolerance for coarse filesystems). The copy is **additive** — only changed
files are copied, and destination-only files are never deleted. See
[copy mode](#copy-mode-incremental).

Flag-to-verb assignments and output forms are in
[api-contracts](api-contracts.md).

# Overview pipeline

```
CLI args ──▶ Config loader ──▶ Planner ──▶ Executor ──▶ Reporter
            (discover + merge   (resolve   (apply       (human
             + parse TOML)        entries    actions)     or --json)
                                  to actions)
```

- The **planner is pure**: a function of *(merged config + current filesystem
  state)*. It reads the FS and emits an ordered list of actions, but never
  mutates. symify is **stateless** — there is no manifest or persisted record
  of what it created; the planner derives everything from config + FS each run.
- `status` = loader + planner + reporter. `sync`/`deploy` = full pipeline, with
  the executor skipped under `--dry-run`.
- This keeps `status`, `--dry-run`, and real runs on one code path.

Config discovery and merge are described in [configuration](configuration.md).

# Per-entry state machine

For one entry: live path **`S`**, store path **`D`**. "Differs" is judged by
the [correctness tests](#correctness-tests).

## `sync` (live → store)

| `S` (live) | `D` (store) | `symlink` | `copy` |
|---|---|---|---|
| missing | any | nothing to push → skip | nothing to push → skip |
| real file | missing | **adopt**: move `S`→`D`, then link `S`→`D` | copy `S`→`D` (`S` stays a real file) |
| real file | exists, same content | **relink**: remove `S`, link `S`→`D` (no backup needed) | AlreadyOk |
| real file | exists, differs | **back up `D`**, move `S`→`D`, link `S`→`D` | per-file diff: back up/replace only changed files |
| already a correct link → `D` | exists | AlreadyOk | — |
| copy matching `D` | exists | — | AlreadyOk |

For `copy`-mode **directories** the copy is a per-file diff, not a whole-tree
recopy — see [copy mode](#copy-mode-incremental) for adds and the
partial-apply/drift rule.

The "same content" relink case applies when the live file already matches the
store (e.g. re-running after a manual edit that happened to converge): there is
no data to preserve, so `S` is replaced by a link with no backup.

## `deploy` (store → live)

Mirror of `sync`: `D` missing → skip; `S` missing → create link/copy at `S`
from `D`; `S` already matches `D` but isn't a link → **relink** (no backup);
`S` exists and differs → **back up `S`**, then write `S` from `D`.

## Conflict & backup rule

A conflict is "both sides exist and the entry is not already in its desired
state." The backup **always protects the side being overwritten**:

- `sync` writes `D` → back up `D` to `<name>.<timestamp>.bak`, then write.
- `deploy` writes `S` → back up `S` to `<name>.<timestamp>.bak`, then write.

The `conflict` setting selects the policy for that overwrite:

- `skip` — leave the existing file, report the conflict (counts as drift /
  non-success).
- `replace` — delete the existing file, then write.
- `backup` (the default, and the safest) — rename to `<name>.<timestamp>.bak`,
  then write. Timestamp format `YYYYMMDDHHMMSS`.

Once a `symlink` entry is established, `S` is a link with no independent
content, so `sync` is a no-op for it (edits flow through to `D`). `copy`-mode
entries have real bytes on both sides, so `sync` and `deploy` remain
meaningful in both directions.

# Link resolution

Each `[mappings.<name>.links]` entry `key = value` resolves to an absolute
`(S, D)` pair, where `S` is the live-side path (link/copy location) and `D` is
the store-side path (real content).

| Input | Result |
|-------|--------|
| `key` relative | `S = live_root / key` |
| `key` absolute | `S = key` as-is; the mirrored `D` strips the leading `/` and joins under `store_root` |
| `value = ""` or `value = true` | mirror: `D = store_root / key` |
| `value = "<relative>"` | explicit: `D = store_root / <relative>` |
| `value = "<absolute>"` | explicit: `D = <absolute>` as-is |
| `value = false` | entry disabled |

`""` and `true` are identical (two spellings of "mirror"). Worked examples
(`live = "~"`, `store = "~/dotfiles"`):

| Entry | `S` (live) | `D` (store) |
|-------|-----------|-------------|
| `".config/fish/config.fish" = ""` | `~/.config/fish/config.fish` | `~/dotfiles/.config/fish/config.fish` |
| `".bashrc" = true` | `~/.bashrc` | `~/dotfiles/.bashrc` |
| `"/absolute/path/file.txt" = true` | `/absolute/path/file.txt` | `~/dotfiles/absolute/path/file.txt` |
| `"profile.md" = "fixed/in/store/file.md"` | `~/profile.md` | `~/dotfiles/fixed/in/store/file.md` |

## Shared store targets

Explicit values may resolve several entries to the same store path `D` (one
source of truth surfaced at several live paths). In `symlink` mode every
live path links to the one store file and edits through any of them are
edits of the store; in `copy` mode `deploy` fans the file out as
independent copies. `sync` with shared-`D` copy entries is last-writer-wins
(entries are independent) and best avoided — treat the store as the source
of truth. Colliding entries — same `D`, or same live path `S` (usually an
accident) — are reported as notes by the verbs; the contract is in
[api-contracts](api-contracts.md). Detection (`plan::shared_targets`) is a
pure, lexically-normalized grouping; it never alters behaviour.

## Directory entries

A key may resolve to a directory. In `symlink` mode it is linked **as a whole
unit** (one link to the entire directory). In `copy` mode the directory is
kept in sync by a **per-file diff** (see
[copy mode](#copy-mode-incremental)): only changed files are copied;
destination-only files are left untouched (additive). Stow-style per-file
folding (one link per file) is out of scope for v1.

## Correctness tests

An entry is AlreadyOk (no-op) when:

- `symlink`: `S` is a symlink whose **resolved** target equals `D`
  (canonicalize and compare, so an equivalent spelling isn't needlessly
  rewritten). Symlinks are written with **absolute** targets.
- `copy`: by default a fast **size + mtime + permission-bits** quick-check per
  file (rsync's default), recursing over a directory's entries. mtime is
  **preserved on copy**, so the check is stable across runs. `--checksum`
  forces an exact BLAKE3 content compare instead; `--modify-window <SECONDS>`
  widens the mtime tolerance for coarse-granularity filesystems (default 0 =
  exact). A mode-only change counts as drift. Symlinks inside a synced tree
  are compared **as symlinks** (by target string), never followed — consistent
  with the executor, which recreates them verbatim; a dangling link is handled
  gracefully.

Permission bits are part of identity only for `copy` mode, where both
sides are independent real files — and only on Unix. Windows has no mode
bits, and its read-only attribute is deliberately ignored (noise, not
signal): there the quick-check is size + mtime only. In `symlink` mode the
real file lives in
`store` and keeps its own mode; the relink decision for an already-matching
live file compares content only (the live file is about to become a link, so
its mode is discarded).

## copy mode (incremental)

`copy`-mode entries are diffed **in the pure planner**, which emits per-file
`Copy`/`Backup`/`Remove` ops — so `--dry-run`, `status`, and the
delete-confirmation gate all see the real per-file work. For a directory the
planner walks source against destination:

- **source file absent in destination** → `Copy` (a pure add; emitted even
  under `conflict = skip`, since it overwrites nothing).
- **present on both, equal** (quick-check or `--checksum`) → nothing.
- **present on both, differs** → the `conflict` policy: `skip` leaves it and
  marks the entry as carrying drift; `backup` backs up then copies; `replace`
  removes then copies.

The walk is **additive**: a file present only on the destination is left
untouched — symify never deletes a file you didn't list. (Removing a stale
copy after you delete its source is a manual step: `rm` / `git rm`.) symify's
own artifacts (`*.bak`, `*.symify-tmp.*`) are **invisible** to the walk on
both sides — never copied as a source add — so backups don't churn and a
second run is idempotent.

**Partial apply + drift.** When an entry both applies changes (adds, resolved
conflicts) **and** leaves an unresolved `skip`-difference, the planner returns
an *applied-with-drift* action: the work runs, but the entry is reported as
drift (exit 1) so a follow-up run isn't falsely all-clean. This mirrors
rsync's `--ignore-existing` (copy new, leave existing) but adds the drift
reporting rsync lacks.

**Atomic, mtime-preserving copy.** The executor copies each file to a temp
beside the destination (`.<name>.symify-tmp.<pid>.<n>`), sets its permission
bits, sets its modification time to match the source, then `rename`s over the
destination — a same-filesystem atomic swap. Readers and crashes never observe
a half-written file; preserved mtime keeps the quick-check stable.
