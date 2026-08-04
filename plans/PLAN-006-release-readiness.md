# Plan: Release readiness (v0.1.0)

> Working plan for this change, kept as a record once landed. Written for a fresh
> session — it assumes no prior context. (Plans are retained, not deleted.)

## Goal

Make symify safely releasable: cut `v0.1.0` from a tag and have correct,
irreversible artifacts land on npm and crates.io without manual babysitting.

The code is done and green (153 tests, clippy/fmt clean, docs strong). This plan
covers the **release machinery, packaging, and the doc/tooling gaps** a first
public release exposes, plus two small behavior changes decided at grill time
(`completions` verb, `-V` output).

Everything lands **before** the rehearsal tag, so `v0.1.0-rc.1` rehearses the
exact final artifact.

## Why now

Registry publishes are effectively irreversible (crates.io never; npm after
72 h). Every gap below is cheap to fix before the first tag and expensive after.

## What "done" means

- `npm run release X.Y.Z` leaves every version and lockfile consistent, asserts a
  changelog section exists, commits and tags — and stops there.
- A `vX.Y.Z` tag publishes only if the tree is tested, version-consistent and
  dry-run clean; partial failure is recoverable and obvious.
- Linux binaries are static musl with no libc floor to track.
- npm and crates.io pages carry full metadata; the GitHub release carries
  binaries, checksums and notes.
- No doc claims anything the release contradicts.

---

## Resolved design decisions (grilled — do not re-litigate)

1. **Rehearse with a real prerelease.** Tag `v0.1.0-rc.1` and publish it for
   real. Dry-runs never exercise auth, provenance, publish ordering or docs.rs.
   `release.yml` detects a prerelease tag and drives three things: npm dist-tag
   `next` (not `latest`), the GitHub Release *prerelease* flag, and nothing else.
   `set-version.mjs`'s existing regex already accepts `-rc.1`.
2. **Repo goes public before the rc.** It is currently private
   (`api.github.com/repos/six5536/symify` → 404) and `main` is **11 commits
   ahead** of `origin/main` (which sits at `feat: safety guardrails`), so
   `ci.yml` and `release.yml` have **never executed**. Public is a hard
   prerequisite: npm provenance requires a public repo, as do the badges and
   SECURITY.md's advisory flow.
3. **npm auth is two-phase.** rc.1 publishes with `NPM_TOKEN` + `--provenance`
   (provenance works with a token given a public repo and `id-token: write`).
   Trusted publishing cannot be configured for a package that does not yet
   exist, so afterwards: configure a trusted publisher on all five now-existing
   packages, then `v0.1.0` onward publishes via OIDC with `NPM_TOKEN` deleted and
   provenance automatic.
4. **Changelog is hand-written**, Keep a Changelog format. `release.yml`
   extracts the section matching the tag and **fails the release if absent** —
   this is the gate that makes "update the changelog" non-optional. Every tag
   needs its own heading, including `0.1.0-rc.1`.
5. **`completions` ships as a runtime verb**; man page as a build artifact.
   `symify completions <shell>` is the only form that reaches npm and
   `cargo install` users — release archives reach neither. `clap_mangen` and
   `clap_complete` are **approved**. Man page generated in `release.yml` into the
   archives, where distro packagers expect it.
6. **`npm run release <version>`** does: set-version (incl. lockfiles) →
   verify-version → assert changelog section → commit → tag. It stops before
   pushing; `git push --follow-tags` stays a deliberate human act.
7. **Release gates via a reusable workflow.** Extract `ci.yml`'s checks into a
   `workflow_call` workflow that both `ci.yml` and `release.yml` call, so the
   release gate cannot drift from CI. Releases are rare — the full gate,
   coverage included, is the right trade before an irreversible publish.
8. **cargo-deny is split.** `licenses`/`bans`/`sources` block PRs and releases
   (deterministic — they only fail when deps change). `advisories` runs on a
   daily schedule and opens an issue, so a new RUSTSEC advisory can never block
   an unrelated PR or an in-flight release.
9. **`-V` switches to `symify 0.1.0`** (from the bare `0.1.0`). Conventional,
   self-identifying in bug reports. This is a deliberate reversal of the
   documented choice at `main.rs:38` — pre-release is the free moment.
10. **Linux ships static musl**, replacing gnu. Measured on aarch64 with the real
    release profile: musl static **873,832 B** vs gnu dynamic PIE **876,192 B** —
    musl is **2,360 B smaller**, because the static libc is offset by dropping
    the PIE's dynamic-linking machinery. It also removes the glibc-floor problem
    permanently and gains Alpine hosts. The npm package names
    (`symify-linux-x64`/`-arm64`) don't encode libc, so **no rename**.
    - The current gnu build's floor is an **incidental `GLIBC_2.30`** (measured
      max required symbol), which already excludes RHEL 8 / Debian 10 /
      Ubuntu 18.04 and can drift silently with runner or zig updates.
    - Verified: static musl resolves `$HOME` identically to gnu, including the
      `getpwuid_r` fallback with `HOME` unset. The musl NSS caveat only affects
      LDAP/SSSD hosts with `HOME` unset — nil for a per-user dotfiles tool.
11. **Keep `cargo-zigbuild` for Linux.** `blake3` ships C code
    (`c/blake3_neon.c`); aarch64 musl needs `aarch64-linux-musl-gcc`, which
    `zig cc` supplies. Verified: plain `cargo build -C linker=rust-lld` cross-
    builds **x86_64** musl fine (static-PIE) but **fails on aarch64** at blake3's
    build script. So: swap the triples `-gnu` → `-musl`, change nothing else.
    Accepted cost: zigbuild's musl output is non-PIE (`ET_EXEC`), so the main
    image isn't ASLR'd — marginal for a local CLI with no network input.
    (`-C relocation-model=pic` was tested and produced a byte-identical
    non-PIE binary; zigbuild ignores it, per its documented RUSTFLAGS caveats.)
12. **One plan, everything before rc.1.** No split into a follow-up polish plan —
    rc.1 must rehearse the final artifact, including the `completions` verb and
    the `-V` change, so nothing published is later invalidated by a behavior
    change.

---

## Functional requirements

### FR0 — Get to a known-good baseline (do this first)

Push the 11 pending commits, make the repo public, and get `ci.yml` passing for
the **first time ever**. Fix whatever it surfaces before anything else starts.
Everything downstream assumes a green baseline that has never been demonstrated.

### FR1 — Version bump is complete and verifiable

`scripts/set-version.mjs` rewrites `Cargo.toml` and every
`packages/*/package.json`, but not the lockfiles — so `set-version 0.2.0` leaves
`Cargo.lock` (`symify`/`symify-core` at `0.1.0`) and `package-lock.json`
(`packages/symify` at `0.1.0`) stale, breaking `cargo publish --locked` and
`npm ci`.

- Extend it to refresh `Cargo.lock` (`cargo update -w --offline`) and
  `package-lock.json` (`npm install --package-lock-only`).
- Add `verify-version`: assert `Cargo.toml`, all five `package.json`s, and both
  lockfiles agree, and optionally match a supplied tag.
- Add `npm run release <version>` per decision 6.

### FR2 — Release workflow: gate before publish

Add a `verify` job that `publish` depends on:
1. Tag ↔ version consistency (FR1).
2. The reusable CI workflow (decision 7), in full.
3. `cargo publish --dry-run --locked` for both crates.
4. `npm publish --dry-run` for all five packages.
5. Changelog section for the tag exists (decision 4).
6. Smoke-test the staged Linux binary: `symify --version` equals
   `symify <tag-version>` — note the new format per decision 9.

### FR3 — Release workflow: correct publish mechanics

- **Targets**: `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` via
  `cargo-zigbuild`; macOS unchanged (native `cargo`, defaults are fine).
- **crates.io**: replace the two sequential `cargo publish` + `sleep 30` with
  `cargo publish --workspace --locked`, available in the pinned 1.96 toolchain;
  it resolves the `symify-core` → `symify` interdependency properly.
- **npm**: publish **platform packages first, launcher last**, so the launcher
  never exists pointing at absent `optionalDependencies`. `--provenance` on.
  Prerelease tags publish under dist-tag `next`.
- `--locked` everywhere; a `concurrency` group; least-privilege `permissions`
  per job.

True cross-registry atomicity is impossible; the requirement is *ordered,
dry-run-gated and recoverable*, satisfying ARCHITECTURE's "publish atomically"
intent as far as the registries allow.

### FR4 — GitHub Release

- Per-target archives: binary + man page + `README.md` + `LICENSE`.
- A `SHA256SUMS` file.
- Notes from the changelog section for that version.
- Prerelease flag set for `-rc` tags.

### FR5 — CHANGELOG

Add `CHANGELOG.md` (Keep a Changelog), seeded with `0.1.0` and `0.1.0-rc.1`.
Cover the pre-release breaking changes with explanation, notably the
`mirror`/`--delete` removal (PLAN-005).

### FR6 — npm package metadata

None of the five `package.json` files declare `repository`, `homepage`, `bugs`,
`keywords`, or `author`. Add them (with `repository.directory` per package) plus
`publishConfig.access = "public"`.

### FR7 — `completions` verb and man page

- Add `Completions { shell }` to `Command` in `cli.rs`, generating via
  `clap_complete` from `CommandFactory::command()`.
- **Add `"completions"` to the `SUBCOMMANDS` shadow list at `cli.rs:158`** —
  otherwise the bare-path rewrite turns `symify completions bash` into
  `symify add completions bash`. Extend the existing
  `known_subcommands_and_aliases_untouched` test.
- Generate the man page with `clap_mangen` in `release.yml` (not `build.rs`) into
  the archives.
- Update ARCHITECTURE's CLI surface section (new verb).
- Add tests; the per-crate 90 % coverage gate applies to the new `crates/app`
  code.

### FR8 — `-V` output change

`symify 0.1.0` instead of bare `0.1.0`. Touches `main.rs:38`, the `cli.rs` help
string ("just the number, for scripts"), the `version_flag_prints_bare_number`
test at `tests/cli.rs:307`, ARCHITECTURE's CLI surface, and FR2's smoke test.

Keep the manual `-V` flag rather than clap's built-in: the current flag is
`global = true`, so `symify sync -V` works; clap's built-in would not preserve
that without `propagate_version`.

### FR9 — Doc corrections

- `README.md` claims `cargo install symify` works on "any platform with a Rust
  toolchain"; ARCHITECTURE ships Unix-only and `packages/symify/README.md` tells
  Windows users to use cargo. Reconcile. Note Windows stays
  **designed-for-but-unshipped** per ARCHITECTURE — the `#[cfg(windows)]` blocks
  in `fs.rs:388` and `main.rs:98` stay.
- `specs/ARCHITECTURE.md` says "CI/CD (`.github/workflows`, **currently
  absent**)" — they exist. Also update the platform matrix (gnu → musl) and the
  CLI surface (`completions`, `-V`).
- `CONTRIBUTING.md` "Releasing" must match the new flow.
- README badges (CI, crates.io, npm, docs.rs, license).
- Add `CODE_OF_CONDUCT.md`.
- Mark the `IDEAS.md` man/completions entry resolved.

---

## Non-functional requirements

### NFR1 — Supply chain

- `.github/dependabot.yml` for `cargo`, `npm`, `github-actions`.
- `deny.toml` + cargo-deny per decision 8.

### NFR2 — Reproducibility & CI speed

- `--locked` on every CI and release cargo invocation; `npm ci` not
  `npm install`.
- `ci.yml`'s `schema` job runs `cargo install cargo-typify --version 0.6.2
  --locked` — an uncached from-source build — while `.mise.toml` **already
  pins** `cargo:cargo-typify = "0.6.2"`. Switch to mise or
  `taiki-e/install-action`. This matters more now that the release runs the full
  gate.

### NFR3 — Papercuts

- `default-members = []` makes a bare `cargo build` fail with *"the workspace has
  no members"*, contradicting CONTRIBUTING's "a plain `cargo build` needs neither
  Node nor `typify`". Set it or document it.
- `packages/symify/bin/symify.js` maps `status === null` to exit 1, losing signal
  exits (Ctrl-C should be 130). Map `result.signal` → `128 + signum`.
- `[package.metadata.docs.rs]` on `symify-core`.

---

## Explicitly out of scope

- Windows CI / binary distribution — ARCHITECTURE defers it post-v1.
- Shipping **both** gnu and musl Linux variants; Homebrew, AUR, or any channel
  beyond npm + crates.io + GitHub Releases.
- Static-PIE on musl (blocked by zigbuild — decision 11).
- Any change to symify's sync/link behavior, config, or schema.
- Automated version bumping (release-please etc.) — the bump stays manual.

## Already landed (prerequisite)

Root `package.json` `workspaces` narrowed from `packages/*` to
`packages/symify`. The platform packages were workspace members, and npm
enforces `os`/`cpu` on members unconditionally, so `npm i` failed with
`EBADPLATFORM` on every host. `set-version` and `release.yml` address the
platform packages by path, so nothing depended on their membership.

## Work breakdown

- **A. Baseline** — FR0. Push, go public, first green CI. *Blocks everything.*
- **B. musl switch** — decision 10/11. Triples `-gnu` → `-musl` in
  `release.yml` and the `build:*` npm scripts; verify both arches run.
- **C. Version integrity** — FR1.
- **D. CI refactor** — decision 7, NFR2. Reusable workflow; fix the typify job.
- **E. Release workflow** — FR2, FR3, FR4.
- **F. Code changes** — FR7, FR8, NFR3.
- **G. Changelog + npm metadata** — FR5, FR6.
- **H. Supply chain** — NFR1.
- **I. Docs** — FR9.
- **J. Rehearse** — tag `v0.1.0-rc.1`, verify end to end, then configure npm
  trusted publishing (decision 3) before `v0.1.0`.

## Critical files

- `scripts/set-version.mjs`, `package.json`
- `.github/workflows/{ci,release}.yml` + new reusable workflow,
  `.github/dependabot.yml` (new), `deny.toml` (new)
- `packages/*/package.json`, `packages/symify/bin/symify.js`
- `crates/app/symify/src/{cli,main}.rs`, `crates/app/symify/tests/cli.rs`
- `Cargo.toml`, `crates/app/symify/Cargo.toml`,
  `crates/lib/symify-core/Cargo.toml`
- `README.md`, `CONTRIBUTING.md`, `specs/ARCHITECTURE.md`, `IDEAS.md`,
  `CHANGELOG.md` (new), `CODE_OF_CONDUCT.md` (new)

## Risks & mitigations

- **CI has never run** (11 unpushed commits). *Mitigate:* FR0 first, before any
  other work, so failures surface against a small diff.
- **A bad publish is permanent.** *Mitigate:* FR2's dry-run gate plus the full
  rc.1 rehearsal.
- **Wrong publish order strands the launcher.** *Mitigate:* FR3 ordering.
- **musl regression not caught by CI** — tests run on the host toolchain, not the
  musl artifact. *Mitigate:* FR2's smoke test on the staged binary; consider
  running the CLI e2e suite against it.
- **New `completions` code dips the 90 % per-crate coverage gate.**
  *Mitigate:* test the verb directly; it is trivially testable (stdout capture).
- **Trusted-publishing switch (decision 3) silently breaks `v0.1.0`.**
  *Mitigate:* it is the one step the rc cannot rehearse — verify the publisher
  config on npmjs.com before tagging, and keep `NPM_TOKEN` until `0.1.0` lands.

## Definition of done

- [ ] Repo public; 11 commits pushed; `ci.yml` green.
- [ ] Linux artifacts are static musl, both arches verified running.
- [ ] `npm run release X.Y.Z` bumps, verifies, asserts changelog, commits, tags —
      and does not push.
- [ ] `release.yml` gates on the reusable CI workflow, version match, dry-runs
      and changelog before publishing.
- [ ] Platform packages publish before the launcher, `--provenance` on; crates
      via `cargo publish --workspace --locked`.
- [ ] Tag produces a GitHub Release with archives, man page, `SHA256SUMS`, notes.
- [ ] `symify completions <shell>` works and is in the `SUBCOMMANDS` shadow list;
      `symify -V` prints `symify X.Y.Z`.
- [ ] `CHANGELOG.md`, `CODE_OF_CONDUCT.md`, `dependabot.yml`, `deny.toml` exist.
- [ ] All five `package.json`s carry repo/homepage/bugs/keywords/author.
- [ ] README badges; Windows claim, ARCHITECTURE (CI note, musl matrix, CLI
      surface) and CONTRIBUTING release steps corrected.
- [ ] `v0.1.0-rc.1` published and verified end to end; trusted publishing
      configured before `v0.1.0`.
- [ ] Existing gates still green: clippy/doc `-D warnings`, doctests,
      `codegen:check`, coverage ≥ 90 %/crate, full `nextest`.
