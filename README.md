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

Just add a file — symify creates the config (`~/.config/symify/symify.toml`,
defaults `live = ~`, `store = ~/dotfiles`) on first use:

```sh
symify add ~/.zshrc      # move it into ~/dotfiles and replace it with a link
symify add ~/.config/nvim
symify list              # see what's tracked
```

Your dotfiles now live in `~/dotfiles`, ready to commit to git, while `~/.zshrc`
and friends keep working as links. To track many files at once, edit the config
directly, then `symify sync`:

```toml
[mappings.dotfiles.links]
".bashrc" = true
".config/fish/config.fish" = true
```

```sh
symify status            # show what each entry will do
symify sync --dry-run    # plan without touching anything
symify sync              # adopt everything listed
```

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
conflict = "backup"   # skip | replace (overwrite, no backup) | backup (.<timestamp>.bak)

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

| Command          | Direction        | What it does                                       |
| ---------------- | ---------------- | -------------------------------------------------- |
| `add <path>`     | `live` → `store` | Track an existing file and adopt it.               |
| `remove <path>`  | —                | Stop tracking a file; restore a standalone copy.   |
| `list` (`ls`)    | read-only        | List mappings and where they point.                |
| `sync`           | `live` → `store` | Bring your live files into the store.              |
| `deploy`         | `store` → `live` | Set a machine up from the store.                   |
| `status`         | read-only        | Report the state of each entry.                    |

- `add`/`remove` take `-m <mapping>` (defaults to your sole mapping) and edit the
  config in place, preserving comments. `remove --no-restore` leaves the link.
- `sync`/`deploy`/`status`/`list` take `-m <mapping>` (repeatable) to act on
  specific mappings; omit it for all.
- `sync`/`deploy`/`add`/`remove` take `--dry-run`; every command takes `--json`
  and `-c <file>` (repeatable; replaces the usual config locations). There's no
  separate `init` — any command creates a default config if none exists.

Run `symify <command> --help` for the full reference and exit codes.

## Safety

symify can move and delete files, so it holds itself to a few rules:

- It only ever touches the exact paths in your config — it never scans a
  directory or tracks files you didn't list.
- It refuses to act on a protected root (`/`, your home directory, or a
  mapping's own `live`/`store` root), and anything outside your `live` root must
  be a single file, not a directory — so a stray `symify add ~` or `add /etc`
  is rejected, not obeyed.
- The mutating commands refuse to run as `root` unless you pass `--allow-root`.
- `conflict = "replace"` is the only setting that deletes without a backup. When
  a run would recursively delete a directory, symify asks first (`[y/N]`); pass
  `-y`/`--yes` to skip the prompt, which is required when output isn't a
  terminal (pipes, `--json`, CI). The default `backup` policy never deletes.

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
