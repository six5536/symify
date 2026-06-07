# Plan: Finish the documentation

> Working plan for this change, kept as a record once landed. Written for a fresh
> session — it assumes no prior context. (Plans are retained, not deleted.)

## Goal

Bring symify's documentation to a **complete, release-ready** state across every
audience, and add a guard so it doesn't rot. symify is "not released" but close;
the docs are strong in places (README, `specs/ARCHITECTURE.md`, CLI `--help`) and
absent in others (npm package pages, contributor guide, changelog, library API
guarantees).

Four audiences, each needs a coherent entry point:

1. **End users** — install, configure, run. (README + CLI `--help`.)
2. **npm consumers** — the page shown on npmjs.com for `@six5536/symify`.
3. **crates.io / docs.rs consumers** — the `symify-core` library API.
4. **Contributors** — how to build, test, codegen, measure coverage, release.

## What "finished" means (the gate)

Like PLAN-003, two gates — both required:

1. **Completeness checklist** — every artifact in [Work breakdown](#work-breakdown)
   exists and is accurate. A human-checkable list, ticked in the DoD.
2. **CI guard** — documentation that *can* rot is machine-checked:
   - `cargo doc --workspace --no-deps` builds clean under
     `RUSTDOCFLAGS=-D warnings` (catches broken intra-doc links, bad code
     fences, malformed rustdoc).
   - `#![warn(missing_docs)]` on the published library so a new undocumented
     public item fails CI (under the existing clippy `-D warnings`).
   - Doctests run as part of the normal test job (`cargo test --doc`), so the
     examples in the docs are compile-and-run-checked, never stale.

Gate 1 stops "we forgot the npm README"; gate 2 stops "the rustdoc silently
went stale." Both must pass.

## Resolved design decisions (grilled — do not re-litigate)

1. **Enforced doc-rot gate (full).** CI enforces all three of: `cargo doc
   --workspace --no-deps` under `RUSTDOCFLAGS=-D warnings`; `#![warn(missing_docs)]`
   (caught by clippy `-D warnings`); and doctests via `cargo test --doc`.
   Consistent with PLAN-003's coverage gate; costs no dependencies. The per-PR
   friction is accepted as the price of docs that stay finished.
2. **`missing_docs` on BOTH crates; gaps fixed at the schema source.** Measured
   cascade: the hand-written `symify-core` code is already 100% item-documented,
   the binary crate has **0** gaps, and **16 of 17** warnings are inside the
   generated config types (typify omitting docs for schema fields/variants that
   lack a `description`), plus 1 for the `pub mod generated;` declaration. So
   enabling on both crates is essentially free. Clear the generated gaps by
   adding the missing `description`s to `schema/symify.schema.json` and
   regenerating (`npm run codegen`) — the docs then flow to **both** rustdoc and
   editor TOML validation. Fall back to a **targeted** `#[allow(missing_docs)]`
   only for any item typify cannot document from the schema (e.g. the
   `LinkValue` `oneOf` arms). Document the `generated` module declaration with a
   one-line `///`. No blanket module-wide allow.
3. **Doctests: one runnable showcase + `no_run` per item.** A single runnable
   end-to-end example on `lib.rs` (`load → resolve → plan → execute(dry_run=true)`
   on a `tempfile` temp tree — `tempfile` is already a dev-dep, available to
   doctests) proves the pipeline. Per-function examples on the primary entry
   points (`config::load`/`resolve`, `plan::plan`, `plan::execute`,
   `status::status`) are marked **`no_run`** — compile-checked for API/signature
   drift, not executed, so no fixture and no flakiness. Examples use only the
   public API; **no new dev-deps** (`toml` is a normal dep, not available to
   doctests, so examples go through `load`/`render_starter`, not `toml::from_str`).
4. **Separate `CONTRIBUTING.md`.** README "Development" trims to a short pointer.
   CONTRIBUTING covers: prerequisites (mise + `.mise.toml` / `rust-toolchain.toml`),
   the npm-script workflow (`build`/`test`/`lint`/`fmt`/`codegen`/**`coverage`/
   `coverage:check`**), schema→Rust codegen + the drift guard, the test layers,
   commit/PR conventions, the plans/specs layout, and the release flow.
   (Justified by the imminent public release + GitHub's PR/issue UI integration.)
5. **`SECURITY.md` — reporting-focused, via GitHub Private Vulnerability
   Reporting.** Supported-versions note for a pre-1.0 tool ("latest release /
   `main`") + report-via-GitHub-advisories instructions. **Cross-links** README
   "Safety" / ARCHITECTURE for the safety model rather than restating it (no
   duplication, no personal email in-repo).
6. **CHANGELOG: deferred until the 0.1.0 release is cut.** Nothing has shipped,
   so a changes-between-releases log has nothing meaningful to say yet; git
   history + the PLAN docs cover pre-release work. The 0.1.0 entry gets written
   when the release is tagged. Not in scope here.
7. **npm READMEs (release-blocking).** Launcher (`@six5536/symify`) gets a
   **concise, npm-tailored** README (one-paragraph what-it-is, `npm i -g`,
   platform-support note, **absolute** links to the GitHub repo for full
   usage/config/safety — deliberately NOT a copy of the root README, to avoid a
   second full-doc drift target), added to that package's `files`. Each platform
   package gets a **one-line** README ("internal prebuilt binary for
   `@six5536/symify` on `<platform>`; don't depend on it directly").
8. **`symify-core` crates.io README.** Short `crates/lib/symify-core/README.md`
   framing it as the **internal** core library (one-paragraph purpose, layered-
   model one-liner, links to docs.rs + ARCHITECTURE; "use the `symify` CLI"),
   with `readme = "README.md"` on that package overriding the workspace default.
9. **Boundary calls.** (a) **Man page / shell completions: NON-GOAL** — needs
   new build-deps (`clap_mangen`/`clap_complete`) + packaging; recorded in
   `IDEAS.md`, revisit at release packaging. (b) **No committed CLI reference** —
   keep README's "run `symify <cmd> --help`" pointer; instead **audit** the help
   text for accuracy as part of the consistency pass. (c) **Minimal GitHub
   templates** — `PULL_REQUEST_TEMPLATE.md` + one bug + one feature issue
   template.
10. **The code is the single canonical source of truth.** The running binary's
    actual behavior wins over **every** document — README, clap `--help` strings,
    `schema` descriptions, **and `specs/ARCHITECTURE.md`**. ARCHITECTURE is *not*
    privileged: where it disagrees with the code, ARCHITECTURE is corrected to
    match (config types can't disagree — the schema↔generated drift guard already
    binds them). The consistency pass (G) fixes docs to describe real behavior.
    This plan changes **no behavior**: editing a clap help *string* is a doc edit
    (in scope); changing what a flag *does* is not. Any genuine code bug surfaced
    while auditing is logged as a **Finding** for a follow-up, never patched here.

## Non-goals

- A documentation **website** / mdBook / rendered docs hosting.
- Tutorials or guides beyond the README quickstart and ARCHITECTURE.
- **Man page / shell completions** (decision 9a) — deferred; needs new deps.
- A committed, generated CLI reference (decision 9b).
- **CHANGELOG** (decision 6) — deferred to the 0.1.0 release.
- Any **code/behavior** change. Doc-only, plus: lint attributes, doc examples,
  schema *descriptions* (decision 2), clap help-*string* fixes, and a CI job.
  The running code is canonical (decision 10), so docs (incl. ARCHITECTURE) are
  corrected to match it — never the reverse. A doctest or audit that exposes a
  real code bug yields a **Finding**, not a fix.

## Current state (inventory)

| Artifact | State | Action |
|---|---|---|
| `README.md` | Good; missing coverage in Dev section; recently added Backups note | Polish; trim Dev to a pointer; consistency pass |
| `specs/ARCHITECTURE.md` | Thorough (28 KB); minor drift vs code (e.g. `-V/--version` + no-arg behavior not in CLI block) | Correct to match code (decision 10); cross-link |
| CLI `--help` (clap `about`/`long_about`) | Humanised, present on all verbs/flags; minor unevenness (`status --modify-window` terser) | Audit; fix help *strings* only (no behavior change) |
| Crate `description` (both `Cargo.toml`) | Present | Keep |
| `schema/symify.schema.json` field docs | Present on most fields; ~16 missing `description`s surface as generated-doc gaps | Add missing descriptions, regenerate (decision 2) |
| `symify-core` rustdoc | Hand-written code 100% item-documented; **no `missing_docs` guard**; **no examples/doctests**; 16 gaps in `generated.rs` + 1 module decl | Enable `missing_docs`; add examples; fix gaps at schema source |
| `symify` (binary) rustdoc | Modules `//!`-documented; **0** `missing_docs` gaps | Enable `missing_docs` (free) |
| docs.rs | Will autobuild from rustdoc | Ensure `cargo doc` clean in CI |
| npm `@six5536/symify` README | **MISSING** (blank npm page) | Add concise tailored README (decision 7) |
| npm platform packages README | **MISSING** | Add one-liner each (decision 7) |
| `symify-core` crates.io README | Inherits binary-focused root README | Give it its own (decision 8) |
| `CONTRIBUTING.md` | **MISSING** | Add (decision 4) |
| `CHANGELOG.md` | **MISSING** | Deferred to 0.1.0 (decision 6) |
| `SECURITY.md` | **MISSING** | Add, reporting-focused (decision 5) |
| `LICENSE` | Present (MIT) | Keep |
| `.github` issue/PR templates | **MISSING** | Add minimal set (decision 9c) |
| `cargo doc` / doctest CI guard | **MISSING** | Add (gate 2) |

## Work breakdown

Grouped; exact set depends on the grilled answers above.

### A. Library API docs + `missing_docs` (decisions 1, 2, 3)
- Add `#![warn(missing_docs)]` to **both** crate roots (`symify-core/src/lib.rs`
  and `symify/src/main.rs`); the binary already has 0 gaps.
- Clear the 16 generated-type gaps by adding the missing `description`s to
  `schema/symify.schema.json`, then `npm run codegen` to regenerate (docs flow
  to rustdoc **and** editor validation). Add a one-line `///` to the `pub mod
  generated;` declaration. Use a targeted `#[allow(missing_docs)]` **only** on
  any item typify can't doc from schema (e.g. `LinkValue` `oneOf` arms) — no
  module-wide allow.
- `npm run codegen:check` must still pass (no unexpected drift beyond the new docs).
- Add doctests (decision 3): **one runnable** end-to-end showcase on `lib.rs`
  (`load → resolve → plan → execute(dry_run=true)` on a `tempfile` tree), and
  **`no_run`** examples on `config::load`/`resolve`, `plan::plan`,
  `plan::execute`, `status::status`. Public API only; no new dev-deps.

### B. npm package pages (decision 7)
- Write `packages/symify/README.md` (install, what it is, platform support,
  pointer to the GitHub README/docs); add `"README.md"` to that package's
  `files`.
- Add a minimal `README.md` to each platform package
  (`symify-{linux,darwin}-{x64,arm64}`) explaining it's an internal prebuilt
  artifact pulled in by `@six5536/symify`; include in `files` if needed.

### C. crates.io README for the library (decision 8)
- Add `crates/lib/symify-core/README.md` (internal core library: purpose,
  layered model one-liner, links to docs.rs and ARCHITECTURE, "use the `symify`
  CLI") and set `readme = "README.md"` on that package (overriding the workspace
  default).

### D. Contributor guide (decision 4)
- `CONTRIBUTING.md`: prerequisites (mise + toolchains from `.mise.toml` /
  `rust-toolchain.toml`), the npm-script workflow (`build`/`test`/`lint`/`fmt`/
  `codegen`/**`coverage`/`coverage:check`**), the schema→Rust codegen + drift
  guard, the test layers (from ARCHITECTURE "Testing"), commit/PR conventions,
  the plans/specs layout, and the release flow.
- Trim README "Development" to essentials + a pointer to CONTRIBUTING.

### E. Project meta (decisions 5, 9c)
- `SECURITY.md` — supported-versions note + report via GitHub Private
  Vulnerability Reporting; cross-link README "Safety" / ARCHITECTURE for the
  safety model (no restatement). *(CHANGELOG deferred — decision 6.)*
- `.github/ISSUE_TEMPLATE/bug_report.md` + `feature_request.md` +
  `.github/PULL_REQUEST_TEMPLATE.md` — minimal.

### F. CI guard (gate 2)
- Add a `docs` job (or steps in the existing `test` job): `cargo doc --workspace
  --no-deps` with `RUSTDOCFLAGS=-D warnings`, and `cargo test --doc --workspace`
  for doctests. `missing_docs` is caught by the existing
  `clippy --all-targets -- -D warnings`.
- Add the missing npm `coverage`/doc scripts to the README/CONTRIBUTING so the
  documented commands match `package.json`.

### G. README polish + consistency pass (decision 10 — code is canonical)
- Add coverage commands to (the trimmed) Development section / CONTRIBUTING.
- Verify README, `--help` strings, `schema` descriptions, and **ARCHITECTURE**
  all describe the **actual code behavior**; fix any doc that drifts (incl.
  ARCHITECTURE) to match the running binary. Known minor drift to fix: the
  `-V/--version` flag + no-arg behavior absent from ARCHITECTURE's CLI block;
  uneven `--modify-window` help between `status` and `sync`/`deploy`.
- Any disagreement that turns out to be a **code** bug → log a Finding, don't fix.
- Cross-link: README → ARCHITECTURE/CONTRIBUTING/SECURITY; docs.rs landing
  (`lib.rs` `//!`) → ARCHITECTURE.

## Critical files

- `crates/lib/symify-core/src/lib.rs` — `#![warn(missing_docs)]` + runnable `//!`
  showcase example.
- `crates/app/symify/src/main.rs` — `#![warn(missing_docs)]` (0 gaps today).
- `crates/lib/symify-core/src/{config,plan,status}.rs` — `no_run` entry-point examples.
- `crates/lib/symify-core/src/model/{mod,generated}.rs` — module `///`; targeted
  `#[allow]` only if needed.
- `schema/symify.schema.json` — add ~16 missing field/variant `description`s; then
  `npm run codegen`.
- `crates/app/symify/src/cli.rs` — help-*string* audit fixes only (no behavior).
- `crates/lib/symify-core/Cargo.toml`, `crates/lib/symify-core/README.md` — new.
- `packages/symify/README.md` + `package.json`; `packages/symify-*/README.md`.
- `README.md` (trim Dev + cross-links), `CONTRIBUTING.md`, `SECURITY.md` — root.
- `.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/*` — minimal.
- `specs/ARCHITECTURE.md` — correct drift to match code.
- `.github/workflows/ci.yml` — doc + doctest guard.
- `IDEAS.md` — record man/completions deferral (decision 9a).

## Risks & mitigations

- **`missing_docs` cascade.** Turning it on may flag many items at once. *Mitigate:*
  scope to `symify-core` first (Q1); document real items rather than blanket-allow.
- **Doctests coupling docs to API.** A doctest breaks if the API changes. *That's
  the point* — but keep examples minimal and on stable entry points to limit churn.
- **npm `files`/README packaging mistakes.** A README not listed in `files`
  won't publish. *Mitigate:* verify with `npm pack --dry-run` per package in the DoD.
- **Doc/spec drift reintroduced.** *Mitigate:* the consistency pass (G) plus the
  CI doc build; ARCHITECTURE stays the single design source of truth.

## Findings (surfaced while documenting)

No code **behavior** bugs were found, so there are no deferred Findings. What the
audit surfaced was the reverse — docs lagging the code (decision 10: code is
canonical), all corrected in-place:

1. **Undocumented bare-path shortcut.** `symify <path>` is rewritten to
   `symify add <path>` (`cli::normalize_args`), absent from README and
   ARCHITECTURE. → Documented in both.
2. **Undocumented `-V`/`--version`** (prints the bare version number) and the
   bare-`symify`-prints-help behavior. → Added to ARCHITECTURE's CLI surface and
   the README.
3. **Uneven `--modify-window` help** between `status` and `sync`/`deploy`. →
   Aligned the `status` help string (a clap doc-string edit; no behavior change).

The `cargo doc -D warnings` gate also caught four latent **rustdoc** bugs (not
behavior): a public→private intra-doc link (`entry_paths` → `resolve_paths`), an
unbacktick'd `<path>` read as an HTML tag in the `LinkValue` schema description,
and two ambiguous/unresolved `[plan]`/`[status]` links. All fixed at the source
(the last via the schema). This is exactly the rot the gate exists to stop.

## Definition of done

- [x] Every gap in the inventory table is closed or explicitly deferred (man/completions, CLI ref, CHANGELOG).
- [x] `cargo doc --workspace --no-deps` clean under `RUSTDOCFLAGS=-D warnings`.
- [x] `#![warn(missing_docs)]` active on **both** crates; `cargo clippy --all-targets -- -D warnings` clean.
- [x] `cargo test --doc --workspace` passes (one runnable + 5 `no_run` examples).
- [x] `npm run codegen` reproduces the committed `generated.rs` exactly (codegen:check will pass).
- [x] `npm pack --dry-run` shows a README in `@six5536/symify` and each platform package.
- [x] `CONTRIBUTING.md`, `SECURITY.md`, `symify-core/README.md`, GitHub templates exist and are accurate.
- [x] README, `--help` strings, schema descriptions, and ARCHITECTURE all describe actual code behavior (decision 10).
- [x] No code/behavior change (diff is docs, schema descriptions, lint attrs, doc examples, CI). Findings logged.
- [ ] CI green (verify after push).

## Notes

- **No new runtime/dev dependencies** are required for the core of this plan
  (doctests, `missing_docs`, `cargo doc`, READMEs all need none). Only the
  deferred man/completions work (decision 9a) would — hence it is a non-goal here and any
  dependency would be raised for approval separately, per project rules.
