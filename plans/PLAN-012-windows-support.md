# Plan: Ship Windows

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

Ship the designed-for-but-unshipped platform: Windows x64, via npm and
`cargo install`. Per
[software-components](/knowledge/software-components.md) this is a
build-target + symlink-privilege task, not a rewrite — the core is
cross-platform (`std::path`, Windows-aware home resolution via `directories`,
link ops isolated in `fs`). Depends on PLAN-008 landing first, so the
no-privilege fallback is documented as `mode = "copy"` from day one.

## Phases

Each phase is independently landable; A must precede the rest.

### A. Green on Windows CI

- Add a `windows-latest` job to `checks.yml` (fmt/clippy/nextest/doctests) so
  both `ci.yml` and `release.yml` inherit it.
- Un-gate the Windows-specific tests; fix what falls out. Expected areas:
  path separators and `canonicalize` (`\\?\` verbatim prefixes) in the
  symlink correctness test, `~`/env expansion, permission-bit semantics in
  the copy quick-check (Unix mode bits don't map — see decision 2), the
  `geteuid` root-refusal no-op, EPIPE behaviour.

### B. Symlink privilege handling

- `fs` picks `symlink_file` vs `symlink_dir` by target type (both exist in
  `std::os::windows::fs`; a wrong pick yields a broken link).
- No auto-elevation. When link creation fails with a privilege error
  (`ERROR_PRIVILEGE_NOT_HELD`), the per-entry failure message says: enable
  Developer Mode, run elevated, or use `mode = "copy"`. Continue-on-error
  semantics already report this per entry without aborting the run.
- `status` on a machine without the privilege still works (reading links
  needs no privilege).

### C. Distribution

- Target: `x86_64-pc-windows-msvc` only (grilled 2026-08-05). Windows-on-ARM
  runs it under the OS's x64 emulation; native arm64 is deferred until it can
  be executed in CI — never ship a binary no CI has run.
- Release workflow: build on the `windows-latest` runner (zigbuild is for the
  musl targets only); `.zip` archives with `symify.exe`; include in
  `SHA256SUMS`.
- npm: new `@six5536/symify-win32-x64` package (`os`/`cpu` fields; **not** a
  workspace member, like the other platform packages); launcher adds it to
  `optionalDependencies` in exact-version lockstep; `.exe` handling in the
  shim's resolve/spawn; update the unsupported-platform message and the
  launcher tests.
- `npm run verify-version` and `npm run release` learn the new locations
  (the "14 locations" count grows — update the number wherever stated).

### D. Docs / knowledgebase

- README: install matrix, a short Windows section (Developer Mode note, copy
  mode as the fallback).
- Knowledgebase: `software-components` (platform matrix, npm tree, workflows),
  `constraints-non-goals` (drop the "designed-for but unshipped" constraint;
  remove the deferred item), `project-overview`, `testing-strategy` (CI
  platforms), `architectural-rules` (root-refusal wording: Windows no-op
  stands).

## Design decisions

1. **Copy mode is the no-privilege story** (grilled 2026-08-05). No
   fallback-to-copy magic and no directory junctions: an entry declared
   `symlink` that cannot link **fails with guidance** ("enable Developer
   Mode, run elevated, or set `mode = \"copy\"`") — silently substituting a
   mechanism, junction semantics included, would violate "the config says
   what happens".
2. **Permission bits**: on Windows the copy quick-check compares size + mtime
   only (readonly-attribute games are noise, not signal). Platform-gated in
   one place in the quick-check; documented in `architecture`.
3. **Coverage gate stays Linux-only.** Windows runs tests, not coverage —
   mirrors the existing macOS arrangement.
4. **No MSI/winget/chocolatey in v1.** npm + cargo only, like Unix. Package
   managers can come later without touching the core.

## Risks & mitigations

- *CI runner cost/flakiness*: Windows jobs are slow; keep the job minimal
  (no doc build duplicate) and cache cargo.
- *Path edge cases beyond CI's reach* (network drives, non-NTFS): out of
  scope; document that symlink mode requires NTFS.

## Critical files

- `.github/workflows/{checks,release}.yml`
- `crates/lib/symify-core/src/{fs,plan,config}.rs` (gated code paths)
- `packages/` (launcher + new platform package), `package.json`, release
  tooling
- `README.md`, `knowledge/{software-components,constraints-non-goals,project-overview,testing-strategy}.md`

## Definition of done

- [ ] Full test suite green on `windows-latest` in CI.
- [ ] Symlink privilege failure produces the guidance message; copy mode
      works without any privilege.
- [ ] `npm i -g symify` on Windows x64 installs and runs the prebuilt binary;
      unsupported-platform message updated for the rest.
- [ ] Release dry-run publishes the Windows package in lockstep;
      version-consistency check covers it.
- [x] Knowledgebase updated; "Windows unshipped" constraint removed.

## Verification status (2026-08-05)

Implemented and verified locally: privilege-guidance code, launcher + package
+ workflow changes, and a full-workspace `x86_64-pc-windows-gnu` cross-build
(all targets, zero warnings) as the compile check. The unticked boxes need the
first `windows-latest` CI run to execute — treat that run's fallout as phase A
work. Before the next release, attach the npm trusted publisher for
`@six5536/symify-win32-x64` (0.0.0 placeholder published 2026-08-05).
