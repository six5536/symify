# Plan: Per-machine mappings (`os` / `host` conditions)

> Working plan, kept as a record once landed. Written for a fresh session — it
> assumes no prior context.

## Goal

Let a mapping declare which machines it applies to, so one store serves a
mixed fleet (work vs home, Linux vs macOS) without templating. Two optional
keys on `[mappings.<name>]`:

```toml
[mappings.linux]
os = "linux"            # string or array; matched against the running OS

[mappings.work]
host = ["wrk-*"]        # string or array; matched against the hostname
```

A non-matching mapping is **inactive**: excluded from planning entirely.
This is deliberately not templating — chezmoi owns that space; symify's
pitch is staying simple.

## Resolved design decisions (grilled 2026-08-05 — do not re-litigate)

1. **Hostname via direct syscall.** Declare POSIX `gethostname(2)` in the
   binary exactly like `geteuid` (`main.rs:136` pattern): no `libc` dep, no
   new crate, raw wrapper coverage-excluded. No dependency approval needed.
   *Amended in review (2026-08-05):* Windows uses `GetComputerNameExW`'s DNS
   hostname (declared directly, `COMPUTERNAME` as fallback) — the NetBIOS
   `COMPUTERNAME` alone is uppercase and capped at 15 chars, so `host`
   patterns that match on Unix would silently miss on Windows.
2. **Host matching: case-insensitive exact, plus `*` at start and/or end**
   (`wrk-*`, `*.local`). A mid-pattern `*` is a config error; the schema
   description states the rule. No glob engine. FQDN variance is handled by
   suffix globs; the raw nodename is compared, undoctored.
3. **Inactive UX: one-line note.** `status`/`sync`/`deploy` print one line
   per inactive mapping — `mapping work: inactive (host)` — never per-entry
   rows; the summary counts skipped mappings, not fake ok entries. `list`
   marks inactive mappings. Nothing inactive enters the entry state machine.
4. **Explicit `-m <inactive>`**: `add`/`remove` refuse with a message (their
   adopt/restore half cannot act on this machine; hand-edit the TOML for
   cross-machine config changes). Run/query verbs print the note and exit 0,
   so shared fleet scripts stay clean everywhere.
5. **Both keys present ⇒ AND.** Absent keys match everything.
6. **`os` is a free string** compared verbatim against `std::env::consts::OS`
   (`linux` | `macos` | `windows` documented; an unknown value never matches
   — permissive, and source builds on other Unixes still work).
7. **No negation syntax** (`os != …`) in v1; write the complement set.
8. **No `[settings]`-level `os`/`host`** — conditions are a mapping concern.

## Functional requirements

- `os` / `host`: string or non-empty array; semantics per decisions 2/5/6.
- Inactive mappings: reporting per decisions 3/4.
- `--json`: inactive mappings appear as `{ mapping, inactive: true,
  reason: "os" | "host" }` objects, not entry arrays.
- Schema updated; editor validation covers the new keys.

## Non-functional requirements

- **Planner purity/testability**: matching runs once, at config resolve
  time, from an injected `MachineContext { os, host }` (like the clock).
  Production fills it in the binary; tests pin it. No env/hostname reads in
  `symify-core`.
- Windows-aware (keys must work when PLAN-012 ships).

## Work breakdown

- **A. Schema + regen**: `os`/`host` on `Mapping`; `npm run codegen`.
- **B. Core**: `MachineContext` param on `resolve()`; match logic (incl. the
  edge-glob matcher, pure and unit-tested); inactive state on
  `ResolvedMapping`; `add`/`remove` refusal.
- **C. Binary**: `gethostname` wrapper beside `is_root`; build the context at
  startup; inactive one-liners in `output` (human + `--json`); `list` marker.
- **D. Tests**: table-driven matching (string/array/edge-glob/mid-glob
  error/AND/absent/case); planner cases with pinned contexts; CLI cases for
  the `add` refusal and the exit-0 note; `--json` shape.
- **E. Docs/KB**: README ("Per-machine setups" section with a worked
  two-machine example), schema descriptions, knowledgebase (`configuration`,
  `api-contracts`, `glossary`: *inactive mapping*).

## Critical files

- `schema/symify.schema.json`, `crates/lib/symify-core/src/{config,plan}.rs`,
  `crates/lib/symify-core/src/model/`
- `crates/app/symify/src/{main,cli,output}.rs`
- `README.md`, `knowledge/{configuration,api-contracts,glossary}.md`

## Definition of done

- [x] Matching semantics exactly as decided, covered by table-driven tests.
- [x] A shared config with mixed `os`/`host` mappings runs clean (exit 0) on
      a machine matching only some of them, including with explicit `-m`.
- [x] `add -m <inactive>` refuses with the explanatory message (test).
- [x] No hostname/OS reads outside the injected context; syscall wrapper in
      the binary only.
- [x] All gates green; knowledgebase validator passes.
