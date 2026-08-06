# AOKF — Agent Open Knowledge Format

**Version:** 0.1
**Status:** Draft
**Date:** 2026-08-04

AOKF is a format for a project knowledgebase: a directory of markdown
files with YAML frontmatter, kept inside the project's repository and
maintained largely by AI agents. Content is unrestricted; architecture
notes, decisions, conventions, and playbooks are typical. AOKF is a
superset of the Open Knowledge Format (OKF) v0.2; familiarity with OKF
is not assumed, and this document stands alone.

The frontmatter identifies each document and carries its trust state:
what kind of document it is, what it describes, what it derives from,
how it relates to other documents, who has verified it. These fields
are constrained because the writers are mostly LLMs: an LLM cannot be
relied on to record a fact (an author, a timestamp, a review)
truthfully, and a validator cannot detect a fabricated value that
parses. The format therefore:

1. Assigns every frontmatter field a **write class** (§4) stating who
   may write it. Fields an agent cannot produce truthfully are closed
   to agents; a git diff check enforces this (§10).
2. Stores no metadata that git already records. Authorship, change
   times, and prior content come from version control, not from
   frontmatter, where a stored copy could diverge.

Frontmatter is limited to fields that are checkable against the
repository, fields that are explicitly judgements, and one field,
closed to agents, that records verification (§7).

> The key words MUST, MUST NOT, SHOULD, and MAY are used as defined in
> RFC 2119.

---

## 1. Terminology

- **Bundle**: the directory tree of knowledge documents.
- **Concept**: one unit of knowledge, one markdown file.
- **Frontmatter**: the YAML block delimited by `---` at the top of a
  file. **Body**: everything after it.
- **Manifest**: the optional `manifest.aokf.yaml` at the bundle root,
  describing the bundle as a whole (§2).
- **Source**: a material a concept derives from, recorded in `sources`.
- **Link**: a directed, typed relationship from one concept to another,
  declared in `links` (§8).
- **Actor**: a string identifying who did something: `human:<id>` for a
  person, `process:<id>` for a deterministic automated process,
  `<producer>/<version>` for an agent or tool (§7).
- **Agent**: an LLM-driven writer. The write classes exist to bound what
  it may touch.

## 2. Bundle structure

The bundle is a directory inside the repository (for example
`/knowledge`). Subdirectories group concepts however suits the
project; paths carry no mandated meaning, and identity does not depend
on them (§5).

```
<bundle>/
  manifest.aokf.yaml  # Optional bundle manifest.
  index.md            # Optional directory listing (§9).
  <concept>.md
  <subdirectory>/
    index.md
    <concept>.md
```

Reserved files are `manifest.aokf.yaml` (bundle root only) and
`index.md` (any directory); neither is a concept. Every other `.md`
file is a concept. There is no change-log file: git log is the change
history.

The manifest declares the bundle:

```yaml
aokf: "0.1"                 # spec version the bundle targets
name: symify-knowledge
description: Knowledgebase for the symify project.
```

`producer`, `generated`, and `counts` keys are stamped (§4): written by
tooling when a bundle is exported for use outside the repository, never
present in the working tree.

## 3. Concept documents

A concept is a UTF-8 markdown file: a YAML frontmatter block, then a
markdown body. `type` is the only required field. Type values are
free-form and unregistered; consumers must tolerate unknown types. For a
code project, expect values like `Module`, `Subsystem`, `Decision`,
`Convention`, `Playbook`, `Reference`.

```markdown
---
type: Module
id: planner
title: Planner
description: Pure planning stage; computes actions without touching the filesystem.
resource: /crates/lib/symify-core/src/planner.rs
tags: [core, planning]
sources:
  - id: config-src
    resource: /crates/lib/symify-core/src/config.rs
    title: Config source
links:
  - rel: depends-on
    to: config
    note: Reads mappings and the conflict policy.
---

# Role

The planner reads [config](config.md) and filesystem state and emits a
list of actions.[^config-src] It never writes.

[^config-src]: Config source
```

The `depends-on` entry is mirrored by the `[config](config.md)` body
link, as §8 requires.

The body is standard markdown. Prefer structure (headings, lists,
tables, fenced code) over freeform prose; there are no required
sections. Per-claim attribution uses footnotes keyed to `sources[].id`
(§6).

Knowledge MUST NOT be duplicated between the bundle and the rest of the
repository. When a document has to exist outside the bundle (a README,
contributor docs), the concept covering that ground carries a concise
summary and cites the file in `sources`; it does not copy the content.
Knowledge with no such external home lives in the bundle only.

## 4. Write classes

Every frontmatter field and manifest key belongs to one class:

| Class        | Who may write                                            | Enforced by            |
|--------------|----------------------------------------------------------|------------------------|
| `open`       | Anyone: humans, agents, tooling.                         | Nothing to enforce.    |
| `restricted` | Humans and deterministic processes. **Agents MUST NOT add, edit, reorder, or delete.** | Diff check (§10). |
| `stamped`    | Deterministic export tooling, at export time. **MUST NOT appear in the working tree.** | Document check (§10). |

Field reference:

| Field                          | Class      | Notes                                   |
|--------------------------------|------------|-----------------------------------------|
| `type`                         | open       | Required. Kind of concept.              |
| `id`                           | open       | Stable identity slug; immutable once assigned (§5). |
| `title`                        | open       | Display name; consumers may fall back to the filename. |
| `description`                  | open       | One-line summary, used by indexes and previews. |
| `tags`                         | open       | Short labels for grouping concepts across directories. |
| `resource`                     | open       | Repo path or URL of the thing the concept describes. Absent for abstract concepts. |
| `status`                       | open       | `draft` \| `stable` \| `deprecated`; absent ⇒ `stable`. |
| `sources`                      | open       | Entries carry `resource`, `id`, `title` only (§6). |
| `links`                        | open       | Typed relationships to other concepts (§8). |
| `verified`                     | restricted | §7.                                     |
| `generated`                    | stamped    | `{ by, at }`, derived from git history at export. Never hand-written. |

Producer-defined extension keys are permitted and default to `open`.
Consumers must not reject documents over unknown keys. A project that
adds an extension carrying a factual claim an agent cannot verify should
declare it `restricted` in its own conventions.

The classes aren't general permissions; an agent edits `status` and
`links` freely. The line they draw is narrower: an `open` field is
either checkable by reading the repo (`resource` points somewhere, or
it doesn't) or openly a judgement (`description`, `status`). A field
that asserts a fact nobody can check is `restricted` or `stamped`.

## 5. Identity

`id` gives a concept an identity that survives file moves.

- An `id` is a slug: lowercase, words separated by `-`. It MUST be
  unique within the bundle.
- Once assigned, an `id` MUST NOT change, even when the file is renamed
  or moved. For agent commits the diff check enforces this (§10); a
  human may change one deliberately and take responsibility for the
  broken references.
- When `id` is absent, the concept's identity is its repo-root-relative
  file path.
- `id` is the preferred target for typed links (§8), precisely because
  it is stable where paths are not.

## 6. Sources

`sources` records what a concept derives from:

```yaml
sources:
  - id: planner-src
    resource: /crates/lib/symify-core/src/planner.rs
    title: Planner source
  - id: clap-docs
    resource: https://docs.rs/clap/latest/clap/
    title: clap documentation
```

- `resource` (required): a repo-root-relative path or a URL.
- `id`: stable key for footnote attribution, local to the file. It is
  unrelated to the concept `id` of §5. Required when the body cites the
  source.
- `title`: optional display label.

To attribute a specific claim, use a markdown footnote whose label is a
`sources[].id`:

```markdown
The planner never writes to the filesystem.[^planner-src]

[^planner-src]: Planner source
```

The footnote label is the join key into `sources`. Keys, not positions:
agents constantly rewrite these documents, and a positional reference
misattributes silently the moment the list is reordered.

There is no per-source author, usage count, or last-modified date. For
repo paths, git supplies author and recency on demand; for URLs, the
writer cannot verify them, so they must not be recorded.

## 7. Verification

`verified` records who has confirmed a concept's content against the
things it describes. It is a list of `{ by, at }` entries; a bare
mapping is read as a one-element list.

```yaml
verified:
  - { by: human:rsewell, at: 2026-08-04T09:00:00Z }
  - { by: process:link-checker, at: 2026-08-04T02:00:00Z }
```

- `by`: a `human:<id>` or `process:<id>` actor. The agent actor form
  never appears here — an agent verifying its own output is not
  verification.
- `at`: an ISO 8601 datetime.

Rules:

- An entry may be added only by the actor it names.
- Agents MUST NOT touch the field at all. When an agent rewrites a
  concept's content, it leaves existing `verified` entries in place;
  whether a verification still applies is derived, not edited (below).
- A verification covers the file as it stood at `at`. A consumer or
  validator compares each entry's `at` against the file's last content
  change in git: verification older than the last change is **lapsed**
  and confers no trust. This comparison is deterministic and needs no
  field an agent could corrupt.

**Trust tiers**, derived from the non-lapsed entries, lowest to highest:

- none ⇒ **unverified**
- `process:` actors only ⇒ **machine-confirmed**
- any `human:` actor ⇒ **human-reviewed**

Tiers are advisory signals, not access control. A concept with no
`verified` key is still consumable.

## 8. Relationships

Typed relationships are declared in a `links` frontmatter array. Each
entry is a map:

| Key    | Rule | Meaning                                                          |
|--------|------|------------------------------------------------------------------|
| `rel`  | MUST | The relationship type (below).                                   |
| `to`   | MUST | Target concept: an `id` (preferred) or a `/` repo-root path.     |
| `note` | MAY  | One-line explanation of this specific edge.                      |

A link asserts a directed edge from the containing concept to `to`.
Consumers resolve `to` as an `id` first, then as a path.

**Relationship vocabulary**, with defined inverses:

| `rel`         | Inverse           | Meaning                                                    |
|---------------|-------------------|------------------------------------------------------------|
| `relates-to`  | `relates-to`      | Generic association (symmetric).                           |
| `part-of`     | `has-part`        | Composition or containment.                                |
| `depends-on`  | `depended-on-by`  | Requires the target to function.                           |
| `references`  | `referenced-by`   | Cites or points at the target.                             |
| `supersedes`  | `superseded-by`   | Replaces the target; the target is deprecated.             |
| `contradicts` | `contradicts`     | Known conflict (symmetric); resolution belongs in prose.   |

Producers SHOULD use a core value where one fits and MAY introduce
custom values (lowercase kebab-case) where none does. Consumers MUST
read an unknown `rel` as `relates-to` rather than reject it. There is
no `derived-from` value: derivation is recorded in `sources` (§6), and
a consumer treats each repo-internal source as a derivation edge.

Producers SHOULD declare each edge once, from whichever side is more
natural; a consumer building a graph SHOULD synthesise the inverse
edge, so backlinks exist without writing every edge twice.

**Body mirroring.** For every `links` entry the body MUST contain at
least one plain markdown link to the same target, so the edge is
visible to a reader of the markdown alone. A body link with no
corresponding `links` entry is an untyped `relates-to` edge; the
meaning of such a link lives in the surrounding prose.

## 9. Paths and indexes

- Paths beginning with `/` resolve from the **repository root**. This is
  the recommended form for links and for path-valued fields
  (`resource`, `sources[].resource`), since it survives moving a concept
  between subdirectories.
- Relative paths resolve from the containing file, as standard markdown.
- Absolute URLs work as anywhere else.

Consumers tolerate broken links, but the validator warns on them
(§10), since a broken link usually means the target was renamed and the
knowledgebase has not caught up.

An `index.md` may appear in any directory to list its contents, so a
reader can see what exists before opening files. It contains no
frontmatter. The body is one or more heading-grouped link lists:

```markdown
# Core

* [Planner](planner.md) - pure planning stage; no filesystem writes.
* [Executor](executor.md) - applies planned actions.
```

Entries should carry the linked concept's `description`. Indexes may be
generated; consumers may synthesise one when absent.

## 10. Validation

Two deterministic layers. Both run without an LLM.

**Document check** — any time, per file:

1. Frontmatter parses as YAML; `type` is present and non-empty. The
   manifest, when present, parses as YAML.
2. `stamped` fields are absent.
3. `restricted` fields, when present, are well-formed (`verified`
   entries each have `by` in `human:`/`process:` form and an ISO 8601
   `at`).
4. `id` values are valid slugs and unique across the bundle. `links`
   entries each have `rel` and `to`.
5. Warn on: broken `/`-paths and relative links, a `links` `to` that
   resolves to nothing, a `links` entry with no mirroring body link,
   `sources` entries the body cites but that lack an `id`, footnote
   labels with no matching source, `index.md` entries pointing at
   missing files.

**Diff check** — per commit, in CI or a hook:

1. Classify the commit's author as agent or not, by whatever identity
   convention the repository uses (committer identity, a trailer, a bot
   account).
2. In an agent commit, every `restricted` field must be byte-identical
   before and after, in every touched concept.
3. In an agent commit, a modified concept keeps its `id`. An agent may
   assign an `id` to a new concept; it must not change an existing one.

A violation of either rule fails the check. The diff check is what
enforces the write classes; without it they are only a convention.

## 11. Conformance

Bundle conformance is a ladder; a bundle's level is the highest it
fully satisfies.

| Level | Requirements |
|-------|--------------|
| **0** | Every non-reserved `.md` file passes the document check (§10). |
| **1** | Level 0, plus every concept has a unique `id`, plus a manifest declaring `aokf` and `name`. |
| **2** | Level 1, plus every `links` entry has a valid `rel` and a `to` that resolves, and is mirrored by a body link (§8). |

A repository conforms if, additionally, its agent commits pass the diff
check (§10). This is independent of the bundle's level.

Consumers must be permissive. In particular, never reject a bundle for
missing optional fields, unknown `type` values, unknown frontmatter
keys, unknown `rel` values, broken links, or a missing `index.md` or
manifest.

## 12. Versioning

This document specifies AOKF **0.1**. Minor version bumps are
backward-compatible additions; major bumps may break. A bundle declares
the version it targets with the manifest's `aokf` key (§2).
