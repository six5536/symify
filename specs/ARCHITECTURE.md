# symify — Architecture

symify is a CLI tool that keeps files in a working location in sync with a
managed backing repository, using symlinks or copies. Managing dotfiles is a
common use, but it applies to any files or folders you want mirrored, backed up,
or deployed across machines.

The core is written in Rust and shipped two ways:

- **cargo** — the `symify-core` library and `symify` binary.
- **npm** — the `@six5536/symify` package: prebuilt per-platform binaries with a
  thin JS launcher.

> Status: not released. Breaking changes are allowed.

## Mental model

Two location roots:

- **`live`** — the working location where files are actually used (e.g. `~`).
- **`store`** — the managed backing repository that holds the real content
  (e.g. `~/dotfiles`), typically under version control.

In `symlink` mode the **link always lives at `live`** and points
to the real file in `store`. **Adopting** existing live files into the store is
the central workflow, not an add-on: the first run on an existing machine pulls
your real files into `store` and replaces them with links; a fresh machine
deploys the store back out into `live`.

Direction is owned by the **verb**, not by the field names — `live` and `store`
name *locations*, never roles.

## Two orthogonal axes: verb (direction) × mode (mechanism)

**Verbs — direction:**

| Verb     | Direction        | Purpose |
|----------|------------------|---------|
| `add`    | `live` → `store` | Track an existing file: edit the config, then adopt it. |
| `remove` | —                | Stop tracking a file: edit the config, restoring a standalone copy by default. |
| `list`   | read-only        | List mappings and where they point. |
| `sync`   | `live` → `store` | Capture/push live files into the store (includes adoption). |
| `deploy` | `store` → `live` | Install the store onto a machine. |
| `status` | read-only        | Report per-entry state; never mutates. |

There is no `init` verb: any command auto-creates a default config (from the
starter template, `live = ~`, `store = ~/dotfiles`) when none exists. Mappings
are selected with `-m/--mapping` (repeatable filter on the run/query verbs;
single value defaulting to the sole mapping on `add`/`remove`).

**Modes — mechanism** (`mode = symlink | sync`):

- `symlink` (default) — a symbolic link at `live` pointing to the file in
  `store`.
- `sync` — an independent content **copy** (no link), kept up to date
  incrementally (rsync-style: only changed files are copied). The mode name
  collides with the `sync` verb; the two are orthogonal and the collision is
  accepted.

`sync`-mode copies are governed by two per-run flags: `--checksum` (exact content
compare instead of size+mtime) and `--modify-window <SECONDS>` (mtime tolerance
for coarse filesystems). The copy is **additive** — only changed files are
copied, and destination-only files are never deleted. See
[sync mode](#sync-mode-incremental-copy).

`--dry-run` is available on `sync` and `deploy`. All verbs support human-readable
and `--json` output.

## Overview pipeline

```
CLI args ──▶ Config loader ──▶ Planner ──▶ Executor ──▶ Reporter
            (discover + merge   (resolve   (apply       (human
             + parse TOML)        entries    actions)     or --json)
                                  to actions)
```

- The **planner is pure**: a function of *(merged config + current filesystem
  state)*. It reads the FS and emits an ordered list of actions, but never
  mutates. symify is **stateless** — there is no manifest or persisted record of
  what it created; the planner derives everything from config + FS each run.
- `status` = loader + planner + reporter. `sync`/`deploy` = full pipeline, with
  the executor skipped under `--dry-run`.
- This keeps `status`, `--dry-run`, and real runs on one code path.

## Per-entry state machine

For one entry: live path **`S`**, store path **`D`**. "Differs" is judged by the
[correctness tests](#correctness-tests).

### `sync` (live → store)

| `S` (live) | `D` (store) | `symlink` | `sync` (copy) |
|---|---|---|---|
| missing | any | nothing to push → skip | nothing to push → skip |
| real file | missing | **adopt**: move `S`→`D`, then link `S`→`D` | copy `S`→`D` (`S` stays a real file) |
| real file | exists, same content | **relink**: remove `S`, link `S`→`D` (no backup needed) | AlreadyOk |
| real file | exists, differs | **back up `D`**, move `S`→`D`, link `S`→`D` | per-file diff: back up/replace only changed files |
| already a correct link → `D` | exists | AlreadyOk | — |
| copy matching `D` | exists | — | AlreadyOk |

For `sync`-mode **directories** the copy is a per-file diff, not a whole-tree
recopy — see [sync mode](#sync-mode-incremental-copy) for adds and the
partial-apply/drift rule.

The "same content" relink case applies when the live file already matches the
store (e.g. re-running after a manual edit that happened to converge): there is
no data to preserve, so `S` is simply replaced by a link with no backup.

### `deploy` (store → live)

Mirror of `sync`: `D` missing → skip; `S` missing → create link/copy at `S` from
`D`; `S` already matches `D` but isn't a link → **relink** (no backup); `S`
exists and differs → **back up `S`**, then write `S` from `D`.

### Conflict & backup rule

A conflict is "both sides exist and the entry is not already in its desired
state." The backup **always protects the side being overwritten**:

- `sync` writes `D` → back up `D` to `<name>.<timestamp>.bak`, then write.
- `deploy` writes `S` → back up `S` to `<name>.<timestamp>.bak`, then write.

The `conflict` setting selects the policy for that overwrite:

- `skip` — leave the existing file, report the conflict (counts as drift /
  non-success).
- `replace` — delete the existing file, then write.
- `backup` (recommended default behavior for safety) — rename to
  `<name>.<timestamp>.bak`, then write. Timestamp format `YYYYMMDDHHMMSS`.

Once a `symlink` entry is established, `S` is a link with no
independent content, so `sync` is a no-op for it (edits flow through to `D`).
`sync`-mode (copy) entries have real bytes on both sides, so `sync` and `deploy`
remain meaningful in both directions.

## Safety

symify moves, links, and (under `replace`) deletes files, so it constrains what
it will act on. The foundational property is that it **never discovers files**:
the planner only ever touches the exact keys written in config — there is no
glob, directory walk, or "track everything under `live`." Beyond that:

**Planner guards** — refused as a per-entry `Failed` (so `status`, `sync`,
`deploy`, and `add` all agree; `add` aborts before editing config). Shared by
`plan` and `status` via `plan::guard_reason`:

- **Protected roots (sentinels).** Refuse an entry whose resolved `live` or
  `store` equals `/`, `$HOME`, or the mapping's own `live`/`store` root. Kills
  `symify add ~` and similar whole-home captures.
- **Store containment.** Refuse an entry whose `live` equals or is an ancestor of
  the `store` root — adopting it would pull the store into itself. (This is the
  one ancestor check kept, because containing the store is never safe; it has no
  false positives for ordinary entries.)
- **Out-of-root ⇒ file-only.** Anything resolving *outside* the live root must be
  a single file, not a directory, on either side. A leaf like `/etc/hosts` is
  fine; `/etc` (a tree) is refused. This preserves the absolute-key feature while
  preventing system-tree captures, even under `sudo`.

**Config validation** — at resolve time, a mapping whose `live` and `store`
resolve to the same directory is rejected outright (every entry would be both
source and destination). Note the default layout deliberately *nests* `store`
under `live` (`~` / `~/dotfiles`); that is fine — only equality is refused.

**Refuse to run as root.** The mutating verbs (`sync`/`deploy`/`add`/`remove`)
refuse when `euid == 0` unless `--allow-root` is passed; `status`/`list` are
always allowed. (`geteuid` is declared directly — no `libc` dependency. Windows
is a no-op pending a future milestone.)

**Confirmation for unrecoverable deletes.** The only unrecoverable op symify
emits is a recursive delete of a non-empty directory, produced by
`conflict = replace`. Before executing such a plan it requires confirmation: an
interactive `[y/N]` prompt (default No) on a TTY, or `--yes`. Non-interactive
runs (piped, `--json`, CI) are *refused* unless `--yes` is given, so a script can
never silently trigger one. `--dry-run` never prompts and shows the deletes in
its preview. A `relink` (removing content byte-identical to the other side) is
recoverable and therefore never gated. This lives in the binary (`confirm.rs`);
the planner stays pure.

## Configuration

### Roots and structure

```toml
[settings]
live = "~"            # working location (links/copies appear here)
store = "~/dotfiles"  # backing repository (real content lives here)
mode = "symlink"      # symlink | sync
conflict = "backup"   # skip | replace | backup

[mappings.dotfiles]
# optional per-mapping overrides of live / store / mode / conflict

[mappings.dotfiles.links]
# entries described below
```

`[settings]` provides defaults; each `[mappings.<name>]` may override `live`,
`store`, `mode`, and `conflict`. Paths support `~` and
environment-variable expansion (Windows-aware) and are normalized to absolute
paths before planning.

### Loading and merge order

Loaded and merged in order; later sources override earlier ones:

1. Default: `~/.config/symify/symify.toml`
2. `~/.config/symify/conf.d/*.toml` (lexicographically sorted)
3. CLI `-c/--config` files (repeatable)

When any `-c` is given it **replaces** default discovery (steps 1–2) — "run
exactly this config." Merge granularity within the active set:

- `[settings]` — per-key deep merge (a drop-in can flip just `conflict`).
- `[mappings]` — distinct names accumulate; same-named mappings deep-merge
  (their `links` combine, later file wins per duplicate key; later `mode` /
  `conflict` / root overrides apply).

## Link resolution

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

### Directory entries

A key may resolve to a directory. In `symlink` mode it is linked **as a whole
unit** (one link to the entire directory). In `sync` mode the directory is kept
in sync by a **per-file diff** (see [sync mode](#sync-mode-incremental-copy)):
only changed files are copied; destination-only files are left untouched
(additive). Stow-style per-file folding (one link per file) is out of
scope for v1.

### Correctness tests

An entry is AlreadyOk (no-op) when:

- `symlink`: `S` is a symlink whose **resolved** target equals `D` (canonicalize
  and compare, so an equivalent spelling isn't needlessly rewritten). Symlinks
  are written with **absolute** targets.
- `sync`: by default a fast **size + mtime + permission-bits** quick-check per
  file (rsync's default), recursing over a directory's entries. mtime is
  **preserved on copy**, so the check is stable across runs. `--checksum` forces
  an exact BLAKE3 content compare instead; `--modify-window <SECONDS>` widens the
  mtime tolerance for coarse-granularity filesystems (default 0 = exact). A
  mode-only change counts as drift. Symlinks inside a synced tree are compared
  **as symlinks** (by target string), never followed — consistent with the
  executor, which recreates them verbatim; a dangling link is handled gracefully.

Permission bits are part of identity only for `sync` (copy) mode, where both
sides are independent real files. In `symlink` mode the real file lives in
`store` and keeps its own mode; the relink decision for an already-matching live
file compares content only (the live file is about to become a link, so its mode
is discarded).

### sync mode (incremental copy)

`sync`-mode entries are diffed **in the pure planner**, which emits per-file
`Copy`/`Backup`/`Remove` ops — so `--dry-run`, `status`, and the
delete-confirmation gate all see the real per-file work. For a directory the
planner walks source against destination:

- **source file absent in destination** → `Copy` (a pure add; emitted even under
  `conflict = skip`, since it overwrites nothing).
- **present on both, equal** (quick-check or `--checksum`) → nothing.
- **present on both, differs** → the `conflict` policy: `skip` leaves it and marks
  the entry as carrying drift; `backup` backs up then copies; `replace` removes
  then copies.
The walk is **additive**: a file present only on the destination is left
untouched — symify never deletes a file you didn't list. (Removing a stale copy
after you delete its source is a manual step: `rm` / `git rm`.) symify's own
artifacts (`*.bak`, `*.symify-tmp.*`) are **invisible** to the walk on both sides
— never copied as a source add — so backups don't churn and a second run is
idempotent.

**Partial apply + drift.** When an entry both applies changes (adds, resolved
conflicts) **and** leaves an unresolved `skip`-difference, the planner returns an
*applied-with-drift* action: the work runs, but the entry is reported as drift
(exit 1) so a follow-up run isn't falsely all-clean. This mirrors rsync's
`--ignore-existing` (copy new, leave existing) but adds the drift reporting rsync
lacks.

**Atomic, mtime-preserving copy.** The executor copies each file to a temp beside
the destination (`.<name>.symify-tmp.<pid>.<n>`), sets its permission bits, sets
its modification time to match the source, then `rename`s over the destination —
a same-filesystem atomic swap. Readers and crashes never observe a half-written
file; preserved mtime keeps the quick-check stable.

## CLI surface

```
symify <path>          shortcut for `symify add <path> …` (see below)
symify add    <path>   [-m MAP] [--store-path P] [-c FILE]... [--force] [--dry-run] [-y] [--json]
symify remove <path>   [-m MAP] [-c FILE]... [--no-restore] [--dry-run] [--json]   (alias: rm)
symify list            [-m MAP]... [-c FILE]... [--entries] [--json]               (alias: ls)
symify sync            [-m MAP]... [-c FILE]... [--dry-run] [-y] [--checksum] [--modify-window N] [--json]
symify deploy          [-m MAP]... [-c FILE]... [--dry-run] [-y] [--checksum] [--modify-window N] [--json]
symify status          [-m MAP]... [-c FILE]... [--checksum] [--modify-window N] [--json]

Global: --allow-root  (permit mutating verbs to run as root; refused otherwise)
        -V, --version (print the bare version number and exit)

Bare `symify` (no command) prints help and exits 0.
```

- The only positional is `<path>` on `add`/`remove` — a real filesystem path
  (CWD-relative / `~` / absolute) from which the config key is derived
  (relativized against the mapping's `live`; an absolute key if outside it).
- **Bare-path shortcut.** `symify <path> …` is rewritten to `symify add <path> …`,
  since adding is the common case. The `add` is inserted before the first non-flag
  token unless that token is a known subcommand or alias; a leading `--` disables
  the rewrite. So `symify status` is still the `status` verb — use `symify add
  status` to track a file literally named `status`.
- `-V, --version` — global; prints the bare version number (just `x.y.z`, for
  scripts) and exits.
- `-m, --mapping` — repeatable filter on the run/query verbs (omit = all);
  a single value on `add`/`remove` defaulting to the sole mapping. Unknown name:
  `add` creates the mapping, every other verb errors.
- `-c, --config <file>` — the repeatable config set; replaces default discovery
  and suppresses auto-init.
- `--dry-run` — `sync`/`deploy`/`add`/`remove`; plan/report without mutating.
- `-y, --yes` — pre-approve unrecoverable recursive deletes (`sync`/`deploy`/`add`);
  see Safety. Required for such a plan when not on a TTY (piped / `--json` / CI).
- `--allow-root` — global; permit a mutating verb to run as root (otherwise refused).
- `--json` — machine-readable output (every verb).

### Config mutation (`add` / `remove`)

`add`/`remove` edit config files with `toml_edit`, preserving comments,
ordering, and the `#:schema` line. They auto-locate across the config set:
`remove` clears the key from **every** file that defines it; `add` writes beside
an existing mapping (highest-precedence file if split) or creates a new mapping
in the primary file. `add` then **adopts** the new entry (a one-entry `sync`);
`remove` **restores** a standalone independent copy at the live path by default
(`--no-restore` to skip). Both honour `--dry-run`.

### Auto-init

There is no `init` verb. In default mode (no `-c`), when the default config is
absent, every command first creates it from the starter template (`live = ~`,
`store = ~/dotfiles`, a `dotfiles` mapping; with a `#:schema` line for editor
validation), printing `Created <path> (defaults).`, then proceeds. An
explicitly-named `-c <file>` that is missing stays an error — auto-init never
fabricates a file the user named.

**Execution semantics:** entries are independent — the executor **continues on
error**, attempts every entry, and reports a per-entry outcome. There is **no
rollback** (backups are the recovery path). Parent directories are created
automatically on the side being written. symify never *prunes* directories
(it does not clean up empty parents it created); it only acts on each entry's
own leaf path — which a `replace` policy or a `relink` may delete or move, per
the entry's resolved action.

**Exit codes:** `0` success / clean; `1` drift (for `status`: any entry out of
sync; for `sync`/`deploy`: an unresolved `skip` conflict); `2` error (one or more
entries failed, or config/IO error).

### `status` reporting

Read-only, direction-neutral. Per entry it reports a state label:

- `symlink`: `ok`, or specific drift (`missing`, `wrong-target`,
  `unadopted` — `S` is a real file, etc.).
- `sync` (copy): `ok` (quick-check or `--checksum` equal), `differs`,
  `live-missing`, `store-missing` — without claiming which direction you should
  run.

## Rust crate layout

One library plus one binary, under the existing workspace globs
(`crates/lib/*`, `crates/app/*`).

### `crates/lib/symify-core` (library)

All domain logic. No argument parsing. **Designed cross-platform** — all path
work via `std::path`, home resolution Windows-aware, and OS-specific link
operations isolated in `fs` so Windows symlink-privilege handling drops in later
without touching the planner.

| Module   | Responsibility |
|----------|----------------|
| `config` | Source discovery, merge order, TOML parse, `~`/env expansion, apply `[settings]` defaults into each mapping; auto-init; mapping selection. |
| `edit`   | Format-preserving config edits (`toml_edit`) for `add`/`remove`. |
| `model`  | Config types **generated** from the JSON Schema (see [Schema codegen](#config-schema-codegen)); plus `Mode { Symlink, Sync }`, `Conflict { Skip, Replace, Backup }`. |
| `plan`   | Pure planner. Resolves the merged config + FS state into an ordered `Vec<Action>` per verb. No mutation. |
| `fs`     | Executor + the platform abstraction for link/copy/move/backup. Apply `Action`s. |
| `status` | Derive per-entry status labels from the plan. |
| `error`  | Error type (`thiserror`). |

`Action` variants (illustrative): `AlreadyOk`, `Disabled`, `Skip`, `Conflict`,
`Failed`, `Apply { kind, ops }`, and `ApplyDrift { kind, ops }` (applied, but a
residual `skip`-difference remains). `kind` is one of `Adopt`/`Relink`/`Link`/
`Push`/`Pull`; `ops` are primitive `FsOp`s (`Symlink`, `Copy`, `Move`, `Backup`,
`Remove`).

### `crates/app/symify` (binary)

Depends on `symify-core`. Binary name `symify`.

| Module    | Responsibility |
|-----------|----------------|
| `main.rs` | Entry point, wiring, exit codes. |
| `cli`     | Argument parsing (`clap` derive). |
| `output`  | Renders a single `serde`-serializable per-entry result model via two renderers — human and `--json`. |

### Publishing

`symify-core` and `symify` publish to crates.io. The compiled binary is also
redistributed via npm (see [Packaging](#packaging--distribution)).

## Config schema codegen

The config data model has a single source of truth in a **hand-authored JSON
Schema** (`schema/symify.schema.json`). Rust types are generated from it, and the
same file drives editor TOML validation. A plain `cargo build` requires neither
Node nor `typify` (the generated Rust is committed).

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

The schema models `links` and `mappings` as objects with `additionalProperties`
(maps), the link value as `oneOf:[string, boolean]` (generates a clean untagged
`LinkValue` enum), and `mode`/`conflict` as string enums.

> **History:** the model was originally specified in TypeSpec, but the
> `@typespec/json-schema` emitter produces a `$id`-per-type bundle using
> `unevaluatedProperties` and nested per-subschema `$defs` that `typify` cannot
> ingest. `typify` works cleanly on a hand-authored schema, so TypeSpec was
> dropped in favor of authoring the JSON Schema directly.

## Packaging & distribution

### cargo

Publish `symify-core` and `symify` to crates.io.

### npm (prebuilt-binary model, à la esbuild / `@swc`)

```
packages/
  symify/              # published as @six5536/symify — launcher (bin: symify)
  symify-linux-x64/    # published as @six5536/symify-linux-x64 — prebuilt binary, declares os/cpu
  symify-linux-arm64/
  symify-darwin-x64/
  symify-darwin-arm64/
```

- The launcher (`@six5536/symify`) declares each platform package in
  `optionalDependencies` pinned to an **exact** version; npm installs only the
  host's match.
- A small JS shim `require.resolve`s the installed platform package's binary and
  `spawnSync`s it with `stdio: "inherit"`, forwarding `argv` and the exit code.
  Descriptive filenames (no `index.ts` unless necessary, per project rules).
- **Version lockstep**: launcher + all platform packages share one version and
  publish atomically.
- **Unsupported platform** (no matching optional dep — e.g. Windows in v1, musl):
  fail with a clear, actionable message listing supported platforms and pointing
  to `cargo install symify`. **No** auto-download / build-from-source fallback in
  v1.

### Platform matrix (v1)

Ships **Unix only**: Linux `x86_64`/`aarch64` (gnu) + macOS `x86_64`/`aarch64`.
**Windows is designed-for but unshipped** — adding it is a build-target +
symlink-privilege task (Developer Mode / elevation, with `sync`-mode copy as the
no-privilege fallback), not a rewrite. A static **musl** Linux build can be added
if the npm story needs it.

### CI/CD (`.github/workflows`, currently absent)

- **PR / push**: `cargo fmt --check`, `clippy`, `nextest`; `codegen:check`
  (regenerate Rust from the schema, fail on diff).
- **Release (tag)**: cross-build binaries (`cargo-zigbuild` or `cross`), assemble
  platform packages, atomically `npm publish` launcher + platform packages,
  `cargo publish`.

## Testing

The pure planner and the injected clock exist largely for testability. Tests run
under `cargo-nextest` (`npm test`).

### Layers

- **Planner (unit, the bulk).** The planner is `(merged config + FS state) →
  Vec<Action>`. The per-entry [state machine](#per-entry-state-machine) table is
  the test matrix: each row × mode × verb is a case. Fast; covers the logic that
  matters most.
- **Executor / library integration.** Lay down a `live` + `store` fixture tree,
  run `sync`/`deploy`/`status` through the `symify-core` API, and assert the
  resulting filesystem: link exists and **resolves** to the right target,
  content matches for `sync` (and mtime is preserved), `.bak` created on
  conflict, only changed files recopied.
- **CLI end-to-end.** Invoke the real binary against temp trees; assert human
  output, `--json` output, and exit codes (`0`/`1`/`2`).
- **Config / merge (table-driven).** TOML strings in, merged config out — deep
  merge, `-c` replace, `conf.d` ordering.
- **Schema codegen.** CI drift guard (regenerate, fail on diff); example configs
  round-trip through the generated types.
- **npm launcher.** A JS test that resolves + spawns a stub binary, and errors
  cleanly when no platform package matches.

### Key choices

- **Real temp directories** (`tempfile`), not a mocked FS. symify's entire job is
  real filesystem semantics (symlink resolution, inode sharing, hashing); an
  in-memory FS would mean re-implementing the OS and would hide exactly the bugs
  that matter. The planner/executor already take root *paths*, so the test seam
  is "point them at a temp tree" — no FS-abstraction trait.
- **Injected clock.** A `now` provider keeps `Date::now`-style calls out of the
  pure layers and lets tests pin `.<timestamp>.bak` names for exact assertions.
- **Explicit assertions** on human output (key lines / summary) and parsed-JSON
  assertions for `--json`. A small fixture-builder helper (DSL to lay down
  live/store trees) keeps state-machine cases readable.
- **Idempotency invariant.** A dedicated test runs each verb twice and asserts
  the second run is all-`AlreadyOk` with exit `0` — catches a class of planner
  bugs.
- **Partial-failure.** Drive a real failure (e.g. an entry refused by the safety
  guard) and assert other entries still apply and the run exits `2` — exercising
  continue-on-error without mocking.

### CI platforms

Tests run on **Linux and macOS** (the v1 ship targets). Windows-specific
behavior (symlink privilege, path handling) is gated until Windows is shipped.

## Recommended dependencies

Named as recommendations only — per project rules, adding a dependency requires
explicit approval and using the latest version at implementation time.

- Rust: `clap` (derive), `serde`, `serde_json`, `toml`, `toml_edit`
  (format-preserving config edits), `thiserror`, `directories` (Windows-aware
  home/dirs), `blake3` (`sync`-mode content equality).
- Rust (dev): `tempfile`, `assert_cmd` for CLI tests.
- Tooling: `cargo-typify` (schema → Rust codegen), `cargo-zigbuild`,
  `cargo-nextest`.

## Deferred (post-v1)

- `clean` verb + optional state manifest for orphan removal (currently
  stateless).
- Stow-style per-file directory folding.
- Windows binary distribution; relative symlink targets as an opt-in.
- `--json`-driven richer tooling; npm download/build fallback.
