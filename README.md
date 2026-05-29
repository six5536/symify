# symify

symify keeps your files in sync with a backing repository, as symlinks,
hardlinks, or copies. It's a dotfiles manager: the files you use day to day stay
where programs expect them, while the real copies live in a repository you can
keep under version control.

There are two locations:

- **`live`** — where your files are used (usually `~`).
- **`store`** — the repository that holds the real content (say `~/dotfiles`),
  typically tracked in git.

`symify sync` moves your existing live files into the store and leaves links in
their place. On a new machine, `symify deploy` recreates those links from the
store. From then on, editing a file edits the real one through its link.

## Install

```sh
npm install -g @six5536/symify   # prebuilt binary, Linux/macOS
cargo install symify             # from source, any platform with a Rust toolchain
```

## Quickstart

Describe what to track in `~/.config/symify/symify.toml`:

```toml
[settings]
live = "~"
store = "~/dotfiles"

[mappings.dotfiles.links]
".bashrc" = true
".config/fish/config.fish" = true
```

**On a machine you already use**, preview first, then bring your files into the
store:

```sh
symify status            # show what each entry will do
symify sync --dry-run    # plan the move without touching anything
symify sync              # move files into ~/dotfiles, replace them with links
```

Your dotfiles now live in `~/dotfiles`, ready to commit to git, while `~/.bashrc`
and friends keep working as links.

**On a fresh machine**, clone the store and deploy it:

```sh
git clone <your-dotfiles-repo> ~/dotfiles
symify deploy            # create links in ~ pointing at the store
```

## Configuration

`[settings]` sets the defaults; each `[mappings.<name>]` can override `live`,
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

symify reads `~/.config/symify/symify.toml` and any `~/.config/symify/conf.d/*.toml`
files, with later files winning key by key. The JSON Schema at
[`schema/symify.schema.json`](schema/symify.schema.json) gives editors TOML
autocomplete and validation.

For path resolution rules, the per-entry state machine, and the full design, see
[specs/ARCHITECTURE.md](specs/ARCHITECTURE.md).

## Commands

| Command  | Direction        | What it does                          |
| -------- | ---------------- | ------------------------------------- |
| `sync`   | `live` → `store` | Bring your live files into the store. |
| `deploy` | `store` → `live` | Set a machine up from the store.      |
| `status` | read-only        | Report the state of each entry.       |

`sync` and `deploy` take `--dry-run`; every command takes `--json` and `-c <file>`
(repeatable, and it replaces the usual config locations). Run
`symify <command> --help` for the full reference and exit codes.

## Development

Toolchains are pinned in `.mise.toml` and `rust-toolchain.toml` (managed with
[mise](https://mise.jdx.dev/)).

```sh
npm run build    # cargo build --workspace
npm run test     # cargo nextest run --workspace
npm run lint     # cargo clippy --workspace
npm run fmt      # cargo fmt --all
npm run codegen  # regenerate the Rust config model from the JSON Schema
```
