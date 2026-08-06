---
name: aokf-maintain
description: Audit the AOKF knowledgebase in knowledge/ for spec conformance, accuracy against the code, structure, and wording, then fix what is found. Run regularly to maintain the bundle. Use when the user asks to maintain, audit, tidy, or check the knowledgebase or KB.
---

Audit and repair the AOKF bundle at `knowledge/`. The format spec is
`.agents/aokf/SPEC.md`; wording rules are `.agents/PROSE.md`. Read both
before editing. Work through the phases in order; each produces findings
the next phase uses.

# Hard limits (from SPEC §4, §5, §7 — never break these)

- Never add, edit, reorder, or delete a `verified` entry, even when
  rewriting the rest of the file. Lapsed verification is reported, not
  edited.
- Never change an existing `id`. Assigning an `id` to a concept that
  lacks one is allowed.
- Never write `generated` in a concept or `producer`/`generated`/`counts`
  in the manifest — those are export-time stamps.

# Phase 1 — deterministic checks

Run the reference validator first:

```
python3 .agents/aokf/tools/validator.py knowledge
```

It performs the SPEC §10 document check and grades the §11 conformance
ladder: frontmatter and `type`, slug-valid unique `id`s, no stamped
fields, well-formed `verified` entries, `links` entries with `rel` and a
resolving `to` mirrored by a body link, existing `/`-rooted and relative
paths, footnote labels matching `sources[].id`, and `index.md` entries
pointing at real files. The bundle must PASS at level 2 with zero errors;
treat warnings as work items too.

Then script the checks the validator does not cover (a throwaway Python
script in the job tmp dir is fine); don't eyeball them:

1. `knowledge/index.md` lists every concept, and each entry's text
   matches the concept's `description` (the index lowercases the first
   word; ignore that difference).
2. The core-concepts list in `AGENTS.md` references only files that
   exist.
3. Flag lapsed verification: a `verified.at` older than the file's last
   content change (`git log -1 --format=%cI -- <file>`) confers no
   trust. Report it; do not touch the field.

Fix what the checks find, then re-run the validator until it passes.
Broken links usually mean a rename the bundle missed — fix the
reference, not the target.

# Phase 2 — accuracy against the code

The code is canonical (see `knowledge/coding-standards.md`). For each
concept, compare its last content change against its `resource` and
repo-path `sources`:

```
git log -1 --format=%cI -- knowledge/<concept>.md
git log -1 --format=%cI -- <each source path>
```

Where a source changed after the concept, read the changed source and
verify the concept's claims still hold; correct any that don't. For
concepts without repo sources, spot-check the two or three most
load-bearing claims. When a doc and the code disagree, fix the doc —
unless the code is wrong, in which case say so and stop for direction.

# Phase 3 — structure

- **Duplication.** No knowledge duplicated between concepts, or between
  the bundle and README/CONTRIBUTING/rustdoc. The concept summarises and
  cites via `sources`; move detail to whichever single home fits and
  cross-reference.
- **Placement.** Content sitting in the wrong concept moves; a concept
  covering two unrelated things splits; near-empty concepts merge into a
  neighbour (keep the surviving file's `id`).
- **Links.** Where prose in one concept leans on another's content,
  ensure a typed `links` entry with the right `rel` plus the mirroring
  body link. Prefer `id` targets. Declare each edge once, from the more
  natural side.
- **Descriptions.** Each `description` is an accurate one-liner; update
  it when the body has drifted, then re-sync the `index.md` entry.

# Phase 4 — wording

Apply `.agents/PROSE.md` to every body you touched and skim the rest:
banned words, filler, hedging, bold-label list prefixes, British English
spelling. Tighten without losing warnings, caveats, or stated
assumptions. Don't rewrite a passage that already conforms — surgical
changes only.

# Phase 5 — report

Finish with a summary: what was fixed (grouped by phase), what was found
but needs a human (lapsed verifications, code-vs-doc conflicts, judgement
calls), and anything intentionally left alone. Leave changes uncommitted
unless the user asked for a commit; if they did, commit as `docs:` per
Conventional Commits.
