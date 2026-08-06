# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
symify uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While
symify is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here. The release workflow refuses to
publish a version it cannot find a heading for.

## [0.2.0] - 2026-08-06

### Added

- Shared-target notes: when several entries resolve to the same store path
  (one source of truth surfaced at several live paths) or the same live path
  (usually a config accident), the verbs print a
  `note: N entries share … path` line and `--json` carries a
  `shared_targets` array. Informational only — exit codes are unchanged. The
  README documents the shared-store-file pattern, including why `sync`
  should be avoided for shared copy-mode entries.
- Windows support (x64): prebuilt binaries via `npm i -g symify` and a `.zip`
  release archive, with CI running the test suite on Windows. Creating
  symlinks needs Developer Mode or elevation; without it, symlink entries
  fail with guidance and `mode = "copy"` works unprivileged.
- `backup_keep = N` (settings or per mapping): when a new
  `<name>.<timestamp>.bak` backup is written, the oldest beyond `N` are
  deleted (the new backup counts). Opt-in — absent or `0` keeps every backup,
  as before. Prunes are visible in `--dry-run` and only ever match symify's
  own exact backup pattern for that path. Pruning a non-empty *directory*
  backup goes through the usual delete confirmation, so scripted runs need
  `--yes` for that case; they also count toward the `removed` total in
  output.
- `symify diff`: a read-only verb showing what `status` only labels — unified
  content diffs per changed file (store side as `-`, live as `+`), `only in
  live/store:` lines for one-sided files, and one-line summaries for binary,
  oversized, or metadata-only differences. Takes the `status` flag set;
  `--json` reports per-file states without content.
- Per-machine mappings: `os` and `host` keys on `[mappings.<name>]` (string
  or list; hostnames match case-insensitively with `*` allowed at a pattern's
  ends). A non-matching mapping is inactive: runs skip it with a one-line
  note and exit 0, `list` marks it, `--json` reports it under
  `inactive_mappings`, and `add`/`remove` refuse it.
- The man page now documents every verb's flags and positionals, plus EXIT
  STATUS and FILES sections; `--help` gains a closing pointer to the config
  location, exit codes, and docs. The README documents every verb's `--json`
  fields.

### Changed

- **Breaking:** the `sync` mode is renamed to `copy`, ending the name
  collision with the `sync` verb. `mode = "sync"` in a config now fails to
  load (`unknown variant 'sync', expected 'symlink' or 'copy'`) — change it
  to `mode = "copy"`; behaviour is identical. The per-entry `mode` field in
  `--json` output reports `"copy"` accordingly.

### Fixed

- Absolute keys resolve correctly on Windows: the mirrored store path now
  strips the drive prefix as well as the root, so `C:\x` nests under the
  store instead of `join` replacing the store root — which made store and
  live the same path. A new planner guard also refuses any entry whose two
  sides resolve to the same file (a self-mapping such as `"/x" = "/x"`),
  which would otherwise replace the file with a link to itself.
- Piping output into a reader that stops early no longer crashes or invents an
  error. `symify completions fish | head` aborted outright (`clap_complete`
  panics on a failed write, and the release profile turns that into SIGABRT);
  `status`, `list` and `man` printed `error: I/O error at <stdout>: Broken pipe`
  and exited 2 once their output outgrew the buffer. A closed pipe is now a
  silent exit 0. One consequence worth knowing: `symify status | head` reports 0
  even when there is drift, because the run never finished.

## [0.1.0] - 2026-08-04

First release.

### Added

- Six verbs: `add`, `remove` (`rm`), `list` (`ls`), `sync`, `deploy`, `status`.
  `symify <path>` is shorthand for `symify add <path>`. There is no `init`; any
  command writes a default config when none exists.
- Two modes. `symlink` (the default) puts a link in your live location pointing
  at the real file in the store. `sync` keeps an independent copy on each side
  and updates only what changed, comparing size and mtime. `--checksum` compares
  content exactly; `--modify-window` covers filesystems with coarse timestamps.
- TOML config at `~/.config/symify/symify.toml`, plus `conf.d/*.toml` merged key
  by key. `-c` replaces discovery entirely. `add` and `remove` edit the file in
  place and keep your comments and ordering. A JSON Schema drives editor
  completion and validation.
- Safety rules. symify never discovers files; it touches only the paths your
  config names. It refuses protected roots (`/`, `$HOME`, a mapping's own roots)
  and directories outside your live root, refuses to run as `root` without
  `--allow-root`, and asks before any unrecoverable recursive delete.
- Backups on overwrite. The default `conflict = "backup"` moves the file being
  replaced to `<name>.<timestamp>.bak` first. `skip` and `replace` are the other
  two policies.
- `--json` on every config-reading verb, `--dry-run` on the mutating ones, and
  exit codes 0 for clean, 1 for drift, 2 for error.
- `symify completions <shell>` for bash, zsh, fish, PowerShell and elvish. A man
  page ships in the release archives.
- Prebuilt binaries on npm (`symify`) for Linux and macOS, x64 and arm64.
  `cargo install symify` builds from source. The Linux binaries are statically
  linked against musl, so they have no glibc floor and run on Alpine.

### Notes

- Windows is designed for but not shipped. The platform-specific code paths
  exist, but no binary is built or tested. `cargo install symify` there is at
  your own risk.
- `symify-core` is published so the binary can depend on a released version. Its
  API is **not** stable yet.
- Development history from before this release is in [`plans/`](plans/) rather
  than here, including features that were added and then dropped again (the
  `mirror` / `--delete` prune behaviour). None of it ever shipped.
- The `0.0.0` versions on npm are empty placeholders. They exist only to reserve
  the package names so trusted publishing could be configured, contain no
  software, and can be ignored.

## [0.1.0-rc.1] - 2026-08-04

Prerelease of `0.1.0`, published to validate the release pipeline end to end.
Functionally identical to `0.1.0`.

[0.2.0]: https://github.com/six5536/symify/releases/tag/v0.2.0
[0.1.0]: https://github.com/six5536/symify/releases/tag/v0.1.0
[0.1.0-rc.1]: https://github.com/six5536/symify/releases/tag/v0.1.0-rc.1
