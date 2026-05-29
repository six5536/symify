# symify

A small CLI that keeps your files in sync with a managed backing repository —
as symlinks, hardlinks, or copies. A dotfiles-style file manager.

You keep two locations:

- **`live`** — where files are actually used (e.g. `~`).
- **`store`** — a repository that holds the real content (e.g. `~/dotfiles`),
  usually under version control.

`symify sync` **adopts** your existing live files into the store and replaces
them with links; `symify deploy` installs the store back out onto a fresh
machine. Edits then flow through the links automatically.

## Install

```sh
npm install -g @six5536/symify   # prebuilt binary, Linux/macOS
cargo install symify             # from source, any platform with a Rust toolchain
```

## Quickstart

**1. Describe what to track** in `~/.config/symify/symify.toml`:

```toml
[settings]
live = "~"
store = "~/dotfiles"

[mappings.dotfiles.links]
".bashrc" = true
".config/fish/config.fish" = true
```

**Capture an existing machine** — preview, then adopt your files into the store:

```sh
symify status            # show what each entry will do
symify sync --dry-run    # plan the adoption without touching anything
symify sync              # move files into ~/dotfiles, replace them with links
```

Your dotfiles now live in `~/dotfiles` (commit them to git) while `~/.bashrc`
and friends keep working as links.

**Set up a fresh machine** — clone the store, then deploy it:

```sh
git clone <your-dotfiles-repo> ~/dotfiles
symify deploy            # create links in ~ pointing at the store
```

## Configuration

`[settings]` are defaults; each `[mappings.<name>]` may override `live`,
`store`, `mode`, or `conflict`.

```toml
[settings]
live = "~"            # where links/copies appear
store = "~/dotfiles"  # where the real content lives
mode = "symlink"      # symlink | hardlink | sync (sync = independent copy)
conflict = "backup"   # skip | replace | backup (.<timestamp>.bak)

[mappings.dotfiles.links]
".config/fish/config.fish" = ""              # "" or true: mirror the key under store
".bashrc" = true
"profile.md" = "fixed/in/store/file.md"      # explicit path under store
"old.conf" = false                           # disabled
```

Config is loaded from `~/.config/symify/symify.toml` and
`~/.config/symify/conf.d/*.toml` (later files win per key). The JSON Schema at
[`schema/symify.schema.json`](schema/symify.schema.json) gives editors TOML
autocomplete and validation.

See [specs/ARCHITECTURE.md](specs/ARCHITECTURE.md) for path resolution rules,
the per-entry state machine, and the full design.

## Commands

| Command  | Direction        | Does                                     |
| -------- | ---------------- | ---------------------------------------- |
| `sync`   | `live` → `store` | Capture/adopt live files into the store. |
| `deploy` | `store` → `live` | Install the store onto a machine.        |
| `status` | read-only        | Report per-entry state.                  |

`sync`/`deploy` accept `--dry-run`; all commands accept `--json` and `-c <file>`
(repeatable; replaces default config discovery). Run `symify <command> --help`
for the full reference and exit codes.

## Development

Toolchains are pinned in `.mise.toml` / `rust-toolchain.toml` (managed with
[mise](https://mise.jdx.dev/)).

```sh
npm run build    # cargo build --workspace
npm run test     # cargo nextest run --workspace
npm run lint     # cargo clippy --workspace
npm run fmt      # cargo fmt --all
npm run codegen  # regenerate the Rust config model from the JSON Schema
```
