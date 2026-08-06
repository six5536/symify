# PLAN-007: Migrate project knowledge to an AOKF bundle

Move canonical project knowledge into an AOKF knowledgebase at
`/knowledge`, make AGENTS.md the entry point that loads it, and delete
the documents whose content moved.

Spec: `.agents/aokf/SPEC.md`. NEW_AGENTS.md's "AOFK" is a typo for
AOKF; normalise everywhere.

## Decisions (from planning, 2026-08-04)

- Bundle lives at `/knowledge` (top level).
- `specs/ARCHITECTURE.md` content moves into concepts; the file and
  `specs/` are deleted.
- Public docs (README, CONTRIBUTING, CHANGELOG, SECURITY,
  CODE_OF_CONDUCT) keep their role. Concepts summarise them concisely
  and cite them in `sources`; no duplication (now a SPEC.md MUST).
- No validator tooling in this plan.
- Already done during planning: SPEC.md bundle-path example changed to
  `/knowledge`; no-duplication rule added to SPEC.md §3.

## Functional requirements

### FR1 — Bundle skeleton

- `/knowledge/manifest.aokf.yaml`: `aokf: "0.1"`, `name:
symify-knowledge`, one-line `description`.
- `/knowledge/index.md`: root listing per SPEC §9, one entry per
  concept with its `description`.
- Layout is flat (all concepts at the bundle root). `id`s make later
  regrouping safe, so no directory taxonomy is decided now.

### FR2 — Concepts

One concept per Core Concepts entry in NEW_AGENTS.md. Every concept
carries `type`, `id`, `title`, `description`, `sources`; `links` with
body mirrors where a real relationship exists (no forced graph).

| Concept                       | `id`                    | `type`     | Content from                                                                                                   |
| ----------------------------- | ----------------------- | ---------- | -------------------------------------------------------------------------------------------------------------- |
| Project Overview              | `project-overview`      | Overview   | README (summary + cite); Project Status from AGENTS.md                                                         |
| Architecture                  | `architecture`          | Reference  | ARCHITECTURE: Mental model, Two orthogonal axes, Overview pipeline, Per-entry state machine, Link resolution   |
| Architectural Rules           | `architectural-rules`   | Convention | ARCHITECTURE: Safety; planner purity, statelessness, additive-only sync                                        |
| Technology Stack              | `technology-stack`      | Reference  | ARCHITECTURE: Dependencies; CONTRIBUTING prerequisites (cite); `.mise.toml`, `rust-toolchain.toml`             |
| Software Components           | `software-components`   | Reference  | ARCHITECTURE: Rust crate layout, Publishing, Packaging & distribution                                          |
| Coding Standards              | `coding-standards`      | Convention | AGENTS.md TypeScript rule and Human Language rules; fmt/clippy conventions                                     |
| Domain Glossary               | `glossary`              | Glossary   | ARCHITECTURE terminology: live/store, mapping, entry, drift, mode, conflict, status labels                     |
| Configuration & Environments  | `configuration`         | Reference  | ARCHITECTURE: Configuration, Config schema codegen                                                             |
| Error Handling & Logging      | `error-handling`        | Convention | ARCHITECTURE: error/exit-code model, broken-pipe rule                                                          |
| Dependency Policy             | `dependency-policy`     | Policy     | AGENTS.md Dependency Rules                                                                                     |
| Security Requirements         | `security-requirements` | Policy     | SECURITY.md (summary + cite); root refusal, protected roots                                                    |
| API Contracts                 | `api-contracts`         | Reference  | ARCHITECTURE: CLI surface, config mutation, auto-init, status reporting, exit codes; `symify-core` instability |
| Testing Strategy              | `testing-strategy`      | Reference  | ARCHITECTURE: Testing; CONTRIBUTING test layers (cite)                                                         |
| Directory Structure           | `directory-structure`   | Reference  | Repo tree: crates/, packages/, knowledge/, plans/, .agents/                                                    |
| Development Procedure         | `development-procedure` | Procedure  | CONTRIBUTING workflow (summary + cite)                                                                         |
| Development Commands          | `development-commands`  | Reference  | CONTRIBUTING pre-push list (cite); package.json scripts                                                        |
| Definition of Done            | `definition-of-done`    | Convention | PR template checklist; per-crate 90% coverage gate                                                             |
| Known Constraints & Non-Goals | `constraints-non-goals` | Reference  | ARCHITECTURE: Deferred (post-v1); Windows unshipped; pre-1.0                                                   |
| Release Procedure             | `release-procedure`     | Procedure  | AGENTS.md Releasing; CONTRIBUTING release section (cite)                                                       |

Rules:

- Content whose old home is deleted (ARCHITECTURE.md, AGENTS.md
  sections, IDEAS.md) moves in full.
- Content whose home remains (README, CONTRIBUTING, SECURITY, PR
  template) is summarised concisely and cited in `sources`; never
  copied.
- Every ARCHITECTURE.md section must land in exactly one concept
  (table above covers all its `##` headings).
- `verified` is written by no one in this migration; concepts start
  unverified. `status: stable` for migrated fact, `draft` where newly
  synthesised.

### FR3 — AGENTS.md switch

- Replace AGENTS.md with NEW_AGENTS.md's content, corrected:
  - AOFK → AOKF; "this files" → "these files".
  - Spec reference → `@.agents/aokf/SPEC.md`.
  - Bundle reference → `@knowledge/index.md`.
  - Core Concepts list → actual file references
    (`@knowledge/<id>.md`), so a reader agent loads them.
- Delete NEW_AGENTS.md. CLAUDE.md (`@AGENTS.md`) needs no change.
- Current AGENTS.md sections must be absorbed into concepts (per FR2
  table) in the same change; nothing may exist only in the old file.

### FR4 — Cross-reference sweep

- CONTRIBUTING.md: 4 references to `specs/`/`specs/ARCHITECTURE.md`
  (lines ~100, ~108, ~125, ~133) → point at the relevant concept.
- `.github/PULL_REQUEST_TEMPLATE.md`: "README / specs / rustdoc" →
  "README / knowledge / rustdoc".
- Grep for remaining `specs/` references after deletion (rustdoc
  comments included).

### FR5 — Deletions (after content is migrated)

- NEW_AGENTS.md
- specs/ARCHITECTURE.md and the `specs/` directory
- IDEAS.md (content → a `Backlog`-type concept or
  `constraints-non-goals`, whichever fits its entries)

## Non-functional requirements

- **No knowledge loss.** Every section of every deleted file is
  accounted for in FR2's table; verify by checklist before deleting.
- **No duplication** (SPEC §3): summaries + `sources` citations for
  surviving docs.
- **Conformance Level 2 by inspection**: unique valid `id`s, manifest
  present, every `links` entry resolvable and body-mirrored. No
  tooling; review manually.
- **Prose rules apply**: concise, plain language, British English.
- **Commit hygiene**: skeleton, concept batches, AGENTS switch, and
  deletions land as separate commits so history shows what moved where.

## Execution order

1. Bundle skeleton (FR1).
2. Concepts sourced from ARCHITECTURE.md; then delete `specs/` (FR2,
   FR5).
3. Concepts summarising surviving public docs (FR2).
4. Process/policy concepts from AGENTS.md, CONTRIBUTING, IDEAS.md;
   delete IDEAS.md (FR2, FR5).
5. Root `index.md` complete; AGENTS.md switch; delete NEW_AGENTS.md
   (FR3).
6. Cross-reference sweep and remaining deletions (FR4, FR5).

## Out of scope

- SPEC §10 validator tooling (document check, diff check) — future
  plan.
- Export/stamping tooling (`generated`, manifest export keys).
- A verification workflow (populating `verified`).
