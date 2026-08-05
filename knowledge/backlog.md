---
type: Backlog
id: backlog
title: Backlog & Decided Ideas
description: Ideas under consideration and ideas decided against, with the reasoning.
status: stable
sources:
  - id: plan-006
    resource: /plans/PLAN-006-release-readiness.md
    title: Release-readiness plan
links:
  - rel: references
    to: architectural-rules
    note: The design constraints that decided the backup question.
---

# Open

- **Same-second `.bak` name collision.** Two backups of the same path within
  one second get the same `<name>.<timestamp>.bak` name, and the second
  rename silently overwrites the first (observed 2026-08-05 with two entries
  sharing a store path, both conflicting in one run). Timestamps are
  second-granular; a uniquifying suffix on collision would fix it.

# Decided against

- **Grandfather–father–son auto backup** (2026-06): out of scope. The store
  is meant to be a git repo, and git already gives full history; restic and
  borg cover the non-git case. A built-in GFS snapshotter would duplicate
  both and fight the stateless, never-discovers-files design
  ([architectural-rules](architectural-rules.md)). The README points at
  git/restic instead. The one wart it would have fixed is the `.bak`
  accumulation above.

# Done (kept for the reasoning)

- **Bounded `.bak` retention** (PLAN-011, 2026-08): the `backup_keep = N`
  setting, applied only when a new backup is written. A setting rather than a
  `clean-backups` verb (a verb is manual — the mess returns), opt-in
  (absent/`0` keeps all: no upgrade silently deletes a recovery path), and
  per-path rather than global. Needed a signed-off carve-out to "never
  discovers files" — see
  [architectural-rules](architectural-rules.md).
- **Man page + shell completions** (PLAN-006): shipped as a runtime
  `symify completions <shell>` verb because that is the only form that
  reaches npm and `cargo install` users — release archives reach neither.
  The man page comes from the hidden `symify man` and goes into the
  archives.
