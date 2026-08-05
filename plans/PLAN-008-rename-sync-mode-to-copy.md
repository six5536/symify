# Plan: Rename `sync` mode to `copy`

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

Rename the config value `mode = "sync"` to `mode = "copy"` as a clean break —
no alias. The mode/verb collision (`mode = "sync"` vs the `sync` verb) is the
most confusing thing in the tool; every doc pays a tax explaining it. `copy`
is also simply more descriptive. The `sync` **verb** is untouched.

## Why now

Pre-1.0 is the cheapest time to break a config value.
[constraints-non-goals](/knowledge/constraints-non-goals.md) lists the
collision as "accepted"; this plan retires that acceptance.

## Resolved design decisions (grilled — do not re-litigate)

1. **Hard break, no alias** (grilled 2026-08-05). `"sync"` is removed from the
   schema enum; an old config fails to load. No deprecation window, no
   warning plumbing from core to binary, no third enum variant.
2. **Break UX = stock serde error.** `unknown variant 'sync', expected
   'symlink' or 'copy'` plus TOML line info already names the fix. No bespoke
   rename hint. A **Breaking** CHANGELOG entry covers the rest.
3. **`--json` breaks in the same release**: the per-entry `mode` field
   (asserted in `output.rs:762`) reports `"copy"`. One release note covers
   both breaks.
4. **Internal naming**: the generated variant becomes `Mode::Copy`
   automatically; hand-written copy-mode terminology follows. Verb-derived
   names (`plan_sync_copy` refers to the verb) stay. Surgical.
5. **Not renamed**: the `sync` verb, `Action` kinds (`Push`/`Pull`), the
   `differs` status label, `.symify-tmp.*` artifacts.

## Functional requirements

- `mode = "copy"` is the only spelling; behaviour identical to today's
  `"sync"` mode.
- `mode = "sync"` fails config load with the stock unknown-variant error.
- The default (`symlink`) and the starter template are unaffected.
- `--json` `mode` field reports `"copy"`.

## Non-functional requirements

- CHANGELOG **Breaking** entry covering the config value and the JSON field;
  ships in a minor bump (pre-1.0 contract allows it).
- All gates green (fmt, clippy `-D warnings`, doctests, `codegen:check`,
  coverage ≥ 90%/crate, nextest, `check:aokf`).

## Work breakdown

- **A. Schema + regen**: mode `oneOf` consts become `symlink`/`copy`;
  descriptions updated; `npm run codegen` (drops `Mode::Sync`, adds
  `Mode::Copy`).
- **B. Core + binary**: fix what the rename breaks; `mode_str` and JSON/human
  output emit `"copy"`; update tests/snapshots.
- **C. Tests**: `"copy"` accepted everywhere `"sync"` was; a config test
  asserts `mode = "sync"` fails with the unknown-variant error.
- **D. Docs/KB**: README, schema descriptions, CHANGELOG Breaking entry, and
  the knowledgebase (`architecture`, `glossary`, `configuration`,
  `api-contracts`, `constraints-non-goals` — drop the "collision accepted"
  entry). "copy mode" becomes the term of art.

## Critical files

- `schema/symify.schema.json`, `crates/lib/symify-core/src/model/`
- `crates/app/symify/src/output.rs` (mode_str, test at :762)
- `README.md`, `knowledge/*.md`, `CHANGELOG.md`

## Definition of done

- [x] `mode = "copy"` works everywhere `"sync"` did; behaviour unchanged.
- [x] `mode = "sync"` fails load with the unknown-variant error (test).
- [x] Repo grep: no doc, schema, or output text presents `"sync"` as a mode
      name (the verb excepted).
- [x] CHANGELOG Breaking entry covers config + JSON.
- [x] All gates green; knowledgebase validator passes.
