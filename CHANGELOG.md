# Changelog

All notable changes to this project are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
symify uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). While
symify is pre-1.0, minor versions may contain breaking changes.

Every released tag needs its own section here. The release workflow refuses to
publish a version it cannot find a heading for.

## [Unreleased]

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

[Unreleased]: https://github.com/six5536/symify/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/six5536/symify/releases/tag/v0.1.0
[0.1.0-rc.1]: https://github.com/six5536/symify/releases/tag/v0.1.0-rc.1
