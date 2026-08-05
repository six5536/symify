---
type: Reference
id: software-components
title: Software Components
description: The Rust crates and their modules, the npm launcher and platform packages, the platform matrix, and the CI/CD workflows.
status: stable
sources:
  - id: core-src
    resource: /crates/lib/symify-core/src
    title: symify-core source
  - id: app-src
    resource: /crates/app/symify/src
    title: symify binary source
  - id: workflows
    resource: /.github/workflows
    title: CI/CD workflows
links:
  - rel: references
    to: architecture
    note: The design these components implement.
---

The system implementing [architecture](architecture.md) is one Rust library
plus one binary (workspace globs `crates/lib/*`, `crates/app/*`), and five npm
packages.

# `crates/lib/symify-core` (library)

All domain logic. No argument parsing. **Designed cross-platform** — all path
work via `std::path`, home resolution Windows-aware, and OS-specific link
operations isolated in `fs` so Windows symlink-privilege handling drops in
later without touching the planner.

| Module   | Responsibility |
|----------|----------------|
| `config` | Source discovery, merge order, TOML parse, `~`/env expansion, apply `[settings]` defaults into each mapping; auto-init; mapping selection. |
| `edit`   | Format-preserving config edits (`toml_edit`) for `add`/`remove`. |
| `model`  | Config types **generated** from the JSON Schema; plus `Mode { Symlink, Sync }`, `Conflict { Skip, Replace, Backup }`. |
| `plan`   | Pure planner. Resolves the merged config + FS state into an ordered `Vec<Action>` per verb. No mutation. |
| `fs`     | Executor + the platform abstraction for link/copy/move/backup. Apply `Action`s. |
| `status` | Derive per-entry status labels from the plan. |
| `clock`  | Injected `now` provider, so `.<timestamp>.bak` names are pinnable in tests. |
| `error`  | Error type (`thiserror`). |

`Action` variants (illustrative): `AlreadyOk`, `Disabled`, `Skip`, `Conflict`,
`Failed`, `Apply { kind, ops }`, and `ApplyDrift { kind, ops }` (applied, but
a residual `skip`-difference remains). `kind` is one of `Adopt`/`Relink`/
`Link`/`Push`/`Pull`; `ops` are primitive `FsOp`s (`Symlink`, `Copy`, `Move`,
`Backup`, `Remove`).

# `crates/app/symify` (binary)

Depends on `symify-core`. Binary name `symify`.

| Module    | Responsibility |
|-----------|----------------|
| `main.rs` | Entry point, wiring, exit codes. |
| `cli`     | Argument parsing (`clap` derive), including the bare-path shortcut. |
| `confirm` | The `[y/N]` gate for unrecoverable deletes; keeps the prompt out of the pure planner. |
| `output`  | Renders a single `serde`-serializable per-entry result model via two renderers — human and `--json`. |

# Publishing

`symify-core` and `symify` publish to crates.io. The compiled binary is also
redistributed via npm.

# npm (prebuilt-binary model, à la esbuild / `@swc`)

```
packages/
  symify/              # published as symify — launcher (bin: symify)
  symify-linux-x64/    # published as @six5536/symify-linux-x64 — prebuilt binary, declares os/cpu
  symify-linux-arm64/
  symify-darwin-x64/
  symify-darwin-arm64/
```

- The launcher (`symify`) declares each platform package in
  `optionalDependencies` pinned to an **exact** version; npm installs only the
  host's match.
- A small JS shim `require.resolve`s the installed platform package's binary
  and `spawnSync`s it with `stdio: "inherit"`, forwarding `argv` and the exit
  code. Descriptive filenames (no `index.ts` unless necessary, per project
  rules).
- **Version lockstep**: launcher + all platform packages share one version and
  publish atomically.
- **Unsupported platform** (no matching optional dep — e.g. Windows in v1, or
  a 32-bit or non-x86/ARM architecture): fail with a message that lists the
  supported platforms and points at `cargo install symify`. **No**
  auto-download / build-from-source fallback in v1. The Linux packages are
  static musl builds, so they cover glibc and musl hosts alike — libc is not a
  dimension of this matrix.

# Platform matrix (v1)

Ships **Unix only**: Linux `x86_64`/`aarch64` (**static musl**) + macOS
`x86_64`/`aarch64`.

Linux is statically linked against musl rather than dynamically against glibc.
It measured marginally *smaller* than the glibc build (the static libc is
offset by dropping the PIE's dynamic-linking machinery), carries no glibc
floor to pin or verify, and runs on Alpine. `cargo-zigbuild` provides the
cross C compiler that `blake3`'s NEON code needs; plain `rust-lld` cannot link
the aarch64 musl target for that reason. zigbuild's musl output is non-PIE,
which is accepted for a local CLI with no network input.

**Windows is designed-for but unshipped** — adding it is a build-target +
symlink-privilege task (Developer Mode / elevation, with `sync`-mode copy as
the no-privilege fallback), not a rewrite.

# CI/CD (`.github/workflows`)

All checks live in a reusable `workflow_call` workflow (`checks.yml`), called
by both `ci.yml` and `release.yml`, so the release gate cannot drift from CI.

- **`checks.yml`**: `cargo fmt --check`, `clippy -D warnings`, `nextest`,
  doctests, `cargo doc -D warnings`, the npm launcher tests, version
  consistency, the AOKF knowledgebase validation (`check:aokf`), the
  per-crate coverage gate, `codegen:check` (schema drift), and `cargo-deny`
  for licences/bans/sources.
- **`ci.yml`**: calls `checks.yml` on push and PR.
- **`audit.yml`**: scheduled `cargo-deny check advisories`, opening an issue
  rather than failing builds — advisories are exogenous and must not block an
  unrelated PR.
- **`release.yml`** (tag `v*`): verify the tag against every version in the
  tree and against a `CHANGELOG.md` section → run `checks.yml` in full →
  cross-build the four binaries and assert the Linux ones are static →
  dry-run every publish → publish platform packages, then the launcher, then
  `cargo publish --workspace --locked` → create a GitHub Release with
  archives, a man page, completions and `SHA256SUMS`. Prerelease tags publish
  under the npm `next` dist-tag and are flagged as prereleases.

Cross-registry atomicity is impossible, so the guarantee is *ordered,
dry-run-gated and recoverable* rather than truly atomic.
