# symify

[![CI](https://github.com/six5536/symify/actions/workflows/ci.yml/badge.svg)](https://github.com/six5536/symify/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/symify.svg)](https://crates.io/crates/symify)
[![npm](https://img.shields.io/npm/v/@six5536/symify.svg)](https://www.npmjs.com/package/@six5536/symify)
[![docs.rs](https://img.shields.io/docsrs/symify-core)](https://docs.rs/symify-core)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

symify keeps files in sync between a working location and a backing repository.
Adding a file takes one command:
`symify ~/.zshrc` moves it into the repository and links it back, so it keeps
working in place.

It works two ways: **symlinks** (the default) or **file sync**, which keeps
independent copies up to date on both sides.

Managing dotfiles is a common use,
but symify works just as well for any files or folders you want to mirror, back
up, or deploy across machines.

There are two locations:

- **`live`** — where your files are used (e.g. `~`).
- **`store`** — the repository that holds the real content (e.g. `~/dotfiles`),
  typically tracked in git.

## Install

```sh
npm install -g @six5536/symify   # prebuilt binary, Linux/macOS
cargo install symify             # from source, needs a Rust toolchain
```

Prebuilt binaries cover Linux and macOS on `x64` and `arm64`. The Linux builds
are statically linked against musl, so they need no particular glibc version and
run on Alpine too.

**Windows is not supported yet.** The code is written to be portable and the
platform-specific paths exist, but no Windows binary is built or tested, so
building from source there is at your own risk.

## Quickstart

For the common dotfiles use case, simply add your first file, and symify creates its config file on first use at
 `~/.config/symify/symify.toml`, with the defaults `live = ~`, `store = ~/dotfiles`:

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
symify status            # report the current state of each entry
symify sync --dry-run    # preview the changes without touching anything
symify sync              # apply them
```

**On a fresh machine**, you may clone the store and deploy it:

```sh
git clone <your-dotfiles-repo> ~/dotfiles
symify deploy            # create links in ~ pointing at the store
```

If the machine already has a file where the store would deploy (e.g. a stock
`~/.bashrc`), the default `conflict = "backup"` policy moves the existing file
aside to `<name>.<timestamp>.bak` before linking, so nothing is lost. Run
`symify deploy --dry-run` first to see exactly what will change.

## Configuration

`[settings]` sets the defaults; each `[mappings.<name>]` can override `live`,
`store`, `mode`, or `conflict`.

```toml
[settings]
live = "~"            # where links/copies appear
store = "~/dotfiles"  # where the real content lives
mode = "symlink"      # symlink | sync (sync = independent copy)
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
autocomplete and validation, and documents every field.

## Commands

| Command          | Direction        | What it does                                       |
| ---------------- | ---------------- | -------------------------------------------------- |
| `add <path>`     | `live` → `store` | Track a file: move it into the store and link it.  |
| `remove <path>`  | —                | Stop tracking a file; restore a standalone copy.   |
| `list` (`ls`)    | read-only        | List mappings and where they point.                |
| `sync`           | `live` → `store` | Bring your live files into the store.              |
| `deploy`         | `store` → `live` | Set a machine up from the store.                   |
| `status`         | read-only        | Report the state of each entry.                    |
| `completions <shell>` | read-only   | Print a shell completion script (bash, zsh, fish, PowerShell, elvish). |

- `symify <path>` is shorthand for `symify add <path>`, since adding is the
  common case. (A path named like a verb, e.g. `status`, is read as that verb;
  use `symify add status` to disambiguate.)
- `add`/`remove` take `-m <mapping>` (defaults to your sole mapping) and edit the
  config in place, preserving comments. `remove --no-restore` leaves the link.
- `sync`/`deploy`/`status`/`list` take `-m <mapping>` (repeatable) to act on
  specific mappings; omit it for all.
- `sync`/`deploy`/`add`/`remove` take `--dry-run`; every command takes `--json`
  and `-c <file>` (repeatable; replaces the usual config locations). There's no
  separate `init`; any command creates a default config if none exists.
- In `sync` mode, `sync`/`deploy` copy only changed files (a size+mtime
  quick-check, with mtime preserved on copy). They are **additive** — they never
  delete, so if you remove a file from the source, delete the stale copy on the
  other side yourself (`rm` / `git rm`). Extra flags:
  - `--checksum` — compare file content exactly instead of by size+mtime.
  - `--modify-window <SECONDS>` — treat mtimes within N seconds as equal, for
    coarse-granularity filesystems (default 0 = exact). `status` accepts
    `--checksum`/`--modify-window` too, so its report matches a run.

Run `symify <command> --help` for the full reference and exit codes, or
`symify -V` for the version.

To enable completions, write the script somewhere your shell reads. For example,
with bash:

```sh
symify completions bash > ~/.local/share/bash-completion/completions/symify
```

A man page ships in the archives attached to each
[GitHub release](https://github.com/six5536/symify/releases).

## Safety

symify can move and delete files, so it holds itself to a few rules:

- It only ever touches the exact paths in your config; it never scans a
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

To report a security issue, see [SECURITY.md](SECURITY.md).

## Backups & History

Your store is just a directory, so history comes for free: keep it under **git**
and commit after each `sync`, and `git log` gives you full, deduplicated,
pushable history. If your store isn't a git repository, point a backup tool such
as [restic](https://restic.net/) or [borg](https://www.borgbackup.org/) at it.

symify focuses on keeping your two locations in sync and leaves long-term
archiving to those purpose-built tools. Its own safety net is the
`<name>.<timestamp>.bak` it writes before overwriting a file under
`conflict = "backup"`.

## Development

Toolchains are pinned in `.mise.toml` and `rust-toolchain.toml` (managed with
[mise](https://mise.jdx.dev/)).

```sh
npm run build           # cargo build --workspace
npm run test            # cargo nextest run --workspace
npm run lint            # cargo clippy --workspace
npm run fmt             # cargo fmt --all
npm run codegen         # regenerate the Rust config model from the JSON Schema
npm run coverage:check  # enforce the per-crate coverage gate
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full setup, the test layers, the
schema/codegen workflow, and how releases are cut.
