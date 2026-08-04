# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
symify uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While
symify is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here — the release workflow refuses to
publish a version it cannot find a heading for.

## [Unreleased]

## [0.1.0-rc.1] - 2026-08-04

First public prerelease. Published to validate the release pipeline end to end;
the feature set is the same one intended for `0.1.0`.

### Added

- **Verbs.** `add`, `remove` (`rm`), `list` (`ls`), `sync`, `deploy`, `status`.
  `symify <path>` is shorthand for `symify add <path>`. There is no `init` — any
  command creates a default config when none exists.
- **Two modes.** `symlink` (default) places a link in your live location
  pointing at the real file in the store. `sync` keeps an independent copy on
  each side, updating only what changed (size + mtime quick-check, with
  `--checksum` for exact content compare and `--modify-window` for coarse
  filesystems).
- **Config.** TOML at `~/.config/symify/symify.toml` plus `conf.d/*.toml`, merged
  key by key, with `-c` to replace discovery entirely. `add`/`remove` edit it in
  place, preserving comments and ordering. A JSON Schema drives editor
  completion and validation.
- **Safety model.** symify never discovers files — it only touches paths named in
  your config. It refuses protected roots (`/`, `$HOME`, a mapping's own roots),
  refuses directories outside your live root, refuses to run as `root` without
  `--allow-root`, and asks before any unrecoverable recursive delete.
- **Backups.** The default `conflict = "backup"` policy moves the file being
  overwritten to `<name>.<timestamp>.bak` first. `skip` and `replace` are also
  available.
- **Output.** Human-readable by default, `--json` on every verb, and exit codes
  `0` clean / `1` drift / `2` error. `--dry-run` on the mutating verbs.
- **Shell completions and man page.** `symify completions <shell>` supports bash,
  zsh, fish, PowerShell and elvish. A man page ships in the release archives.
- **Distribution.** Prebuilt binaries via npm (`@six5536/symify`) for Linux and
  macOS on x64 and arm64, and from source via `cargo install symify`. Linux
  binaries are statically linked against musl, so they carry no glibc
  requirement and run on Alpine.

### Notes

- Windows is designed for but not shipped: the platform-specific code paths
  exist, but no binary is built or tested. Use `cargo install symify` at your own
  risk there.
- `symify-core` is published so the binary can depend on a released version. Its
  API is **not** stable yet.
- This project's pre-release development history — including features that were
  added and then removed before any public release, such as the `mirror` /
  `--delete` prune behavior — is recorded in [`plans/`](plans/) rather than here,
  since none of it ever shipped to users.

[Unreleased]: https://github.com/six5536/symify/compare/v0.1.0-rc.1...HEAD
[0.1.0-rc.1]: https://github.com/six5536/symify/releases/tag/v0.1.0-rc.1
