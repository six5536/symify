# Plan: `add` / `remove` / `list` verbs, mapping scoping, and auto-init

## Context

symify can read and apply config, but there's no way to *change* it without
hand-editing TOML, no way to see the mappings at a glance, and no way to run a
subset of mappings. This adds three verbs and two cross-cutting behaviours:

- `symify add <path>` — start tracking an existing file (and adopt it).
- `symify remove <path>` — stop tracking a file, restoring a standalone copy.
- `symify list` — show mappings and where they point.
- **Mapping scoping** — `-m/--mapping` limits any run/query verb to chosen mappings.
- **Auto-init** — any command auto-creates a default config when none exists; the
  separate `init` verb is **removed**.

The `config` module (`crates/lib/symify-core/src/config.rs`) currently only reads
(`load`/`resolve`/`ResolvedConfig`/`ResolvedMapping`/`default_config_path`/
`expand_path`/`render_starter`). The serde `toml` crate doesn't preserve comments,
so editing the user's annotated config (with its `#:schema` line) needs
`toml_edit`.

## Final CLI surface

```
symify add    <path>   [-m MAP] [--store-path P] [-c FILE]... [--force] [--dry-run] [--json]
symify remove <path>   [-m MAP] [-c FILE]... [--no-restore] [--dry-run] [--json]   (alias: rm)
symify list            [-m MAP]... [-c FILE]... [--entries] [--json]               (alias: ls)
symify sync            [-m MAP]... [-c FILE]... [--dry-run] [--json]
symify deploy          [-m MAP]... [-c FILE]... [--dry-run] [--json]
symify status          [-m MAP]... [-c FILE]... [--json]
```

- The **only positional** is `<path>` on `add`/`remove` (a real filesystem path).
- **`-m/--mapping`** names mappings everywhere (cargo `-p` style): repeatable
  filter on run/query verbs (no `-m` = all); single value on `add`/`remove`,
  defaulting to the sole mapping.
- **`-c/--config`** is the uniform, repeatable config set; when given it replaces
  default discovery (and suppresses auto-init).

## Resolved design (from grilling)

- **add `<path>`** = a filesystem path (CWD-relative / `~` / absolute). symify
  derives the config **key** by relativizing the absolute path against the
  mapping's `live`; if the file is outside `live`, it uses an **absolute key**
  (mirrored under `store`, leading `/` stripped). Prints the derived key.
  - Path **must exist** (error if missing). Value defaults to `true`; an explicit
    store location is given by `--store-path` (a flag, not a positional).
  - **Idempotent** when the entry already exists with the same value; a differing
    value requires `--force`.
- **remove `<path>`** derives the key the same way but **does not require the file
  to exist**. `--restore` (default) replaces a managed `live` link with a
  **standalone independent copy** (removing a symlink, or breaking a hardlink's
  inode share); it's a no-op for copy-mode / already-independent / missing-store.
  `--no-restore` only edits config.
- **Auto-locate** (no single "target file"): both verbs operate over the
  discovered config set. `remove` deletes the key from **every** file that defines
  it (else errors). `add` writes beside an existing mapping (the
  highest-precedence file if it's split across several), or creates a new mapping
  in the **primary** file (`symify.toml` in default mode, or the first `-c` file).
  Both report the file(s) touched.
- **`-m` resolution**: sole-mapping default for `add`/`remove` (0 or >1 → error);
  all-by-default for run/query verbs. **Unknown `-m` name**: `add` *creates* the
  mapping (it's the one creating verb); everyone else **errors**. Multiple `-m` =
  union for run/query, error for `add`/`remove`.
- **add then adopt / remove then restore** run through the existing planner; both
  honour `--dry-run` (preview the config edit *and* the filesystem action).
- **Auto-init**: in default mode (no `-c`) when the default config is absent,
  create it from `render_starter("~", "~/dotfiles")`, print
  `Created <path> (defaults).`, then proceed. A missing **`-c`** file stays an
  error. Replaces the `init` verb and the no-config hint.
- **list**: per-mapping summary (name, `live`, `store`, `mode`, `conflict`, entry
  count); `--entries` adds each entry's resolved `live → store`; `--json` always
  includes entries. Aliases: `rm`, `ls`.

## Approach

### 1. Dependency
Add `toml_edit = "0.25"` to `[workspace.dependencies]` and to
`crates/lib/symify-core/Cargo.toml`.

### 2. `symify-core`
- **New `edit.rs`** (toml_edit, format/comment-preserving):
  - `locate(files: &[PathBuf], mapping, key) -> Result<Vec<PathBuf>>` — which files
    define `[mappings.<m>.links].<key>`.
  - `add_link(files: &[PathBuf], primary: &Path, mapping, key, value: LinkValue, force) -> Result<AddReport { file, created_mapping, replaced }>`
    — pick the target file per the auto-locate rules, ensure
    `[mappings.<m>.links]`, set `key`, error on differing value unless `force`.
  - `remove_link(files: &[PathBuf], mapping, key) -> Result<Vec<PathBuf>>` — remove
    from every defining file; error if none.
- **`select(resolved, names: &[String]) -> Result<ResolvedConfig>`** in `config.rs`
  — keep named mappings in order; error on unknown; empty names = unchanged.
- **`ensure_config(cli: &[PathBuf]) -> Result<Vec<PathBuf>>`** in `config.rs` —
  `discover`; if empty and `cli` empty, auto-init (write `render_starter` defaults,
  print created-path), then re-discover.
- Promote `plan::resolve_paths` to **`pub fn entry_paths(m, key, value) -> (PathBuf, PathBuf)`** for `list`.
- `lib.rs`: export `edit`, `select`, `ensure_config`, `entry_paths`.

### 3. Binary (`crates/app/symify/src/`)
- **Remove** the `Init` command, `run_init`, and `no_config_hint`.
- **`cli.rs`**: `Add`/`Remove`(`rm`)/`List`(`ls`) commands; `-m` on every verb
  (`Vec<String>` for run/query, `Option<String>` for add/remove); `<path>`
  positional on add/remove; flags as in the surface above.
- **`main.rs`**: every verb starts with `config::ensure_config(&args.config)` then
  `load` + `resolve`; run/query verbs apply `config::select(_, &args.mapping)`.
  - `add`: resolve to find the mapping's `live`; derive key; `edit::add_link`;
    unless `--dry-run`, build a one-entry `ResolvedConfig` and `plan(Sync)` +
    `execute` to adopt just that file; render.
  - `remove`: derive key; resolve `(live, store)`; unless `--no-restore`/`--dry-run`,
    restore via `fs::apply_op` (`Remove(live)` + `Copy(store→live)`) when `live` is
    a managed link; `edit::remove_link`; render.
  - `list`: render per-mapping summary; `--entries` → per-entry `live → store` via
    `entry_paths`; `--json`.
- **`output.rs`**: `render_add`/`render_remove`/`render_list` (human + `--json`),
  reusing `EXIT_*` and existing renderer patterns.

### 4. Tests
- `edit.rs` units: add preserves `#:schema` + comments; creates a missing mapping;
  `locate`/`remove_link` across multiple files; `--force` semantics.
- `config` units: `select` (keep/order/unknown-error); `ensure_config` auto-init
  writes a valid default and is a no-op when a config exists.
- CLI (`tests/cli.rs`): `add` adopts + shows in `list`; comment/`#:schema`
  preserved after `add`; `remove` restores a standalone file and clears the entry
  from all files; `--no-restore`; `--dry-run` changes nothing; `-m` scoping limits
  a `sync` to one mapping and errors on unknown; auto-init fires for a bare command
  with an empty `XDG_CONFIG_HOME` and prints the created path; `rm`/`ls` aliases.

### 5. Docs
- `README.md`: drop `init`; Commands table gains `add`/`remove`/`list`; quickstart
  becomes "just `symify add ~/.zshrc`" (auto-init); note `-m` scoping.
- `specs/ARCHITECTURE.md`: replace the `init` verb with `add`/`remove`/`list`;
  document `-m` scoping, auto-init, and a "config mutation" note (toml_edit
  preserves formatting; `add` adopts; `remove` restores by default).

## Critical files
- `Cargo.toml`, `crates/lib/symify-core/Cargo.toml`
- `crates/lib/symify-core/src/edit.rs` (new), `src/config.rs` (`select`,
  `ensure_config`), `src/plan.rs` (`entry_paths`), `src/lib.rs`
- `crates/app/symify/src/{cli,main,output}.rs` (remove init), `tests/cli.rs`
- `README.md`, `specs/ARCHITECTURE.md`

## Reuse
- `config::load`/`resolve`/`render_starter`/`default_config_path`/`expand_path`.
- `plan::plan` + `execute` (add-adopt); `fs::apply_op` + `FsOp::{Remove,Copy}` and
  `fs::{symlink_points_to,same_inode}` (remove-restore detection).
- `output::EXIT_*` and the human/JSON renderers.

## Out of scope
- Setting `mode`/`conflict` per mapping via `add` (new mappings inherit
  `[settings]`). Pruning emptied `[mappings.*.links]` tables. Auto-initing a
  missing `-c` file. `~user` expansion.

## Verification
- `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets -D
  warnings`, `cargo fmt --all --check`, launcher test — all green.
- Manual e2e (empty `XDG_CONFIG_HOME`): `symify add ~/.zshrc` → prints
  `Created …`, adds `.zshrc = true` to the new config, and `~/.zshrc` becomes a
  symlink into `~/dotfiles`; `symify list --entries` shows it with comments/`#:schema`
  intact; `symify remove ~/.zshrc` → standalone `~/.zshrc` restored, entry gone;
  `--no-restore` leaves the link; with two mappings, `symify sync -m dotfiles`
  touches only that one and `symify sync -m nope` exits `2`.
