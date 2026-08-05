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

- **Bounded `.bak` retention.** `.bak` files accumulate without bound on
  repeated conflicts; a small retention setting could cap them.

# Decided against

- **Grandfather–father–son auto backup** (2026-06): out of scope. The store
  is meant to be a git repo, and git already gives full history; restic and
  borg cover the non-git case. A built-in GFS snapshotter would duplicate
  both and fight the stateless, never-discovers-files design
  ([architectural-rules](architectural-rules.md)). The README points at
  git/restic instead. The one wart it would have fixed is the `.bak`
  accumulation above.

# Done (kept for the reasoning)

- **Man page + shell completions** (PLAN-006): shipped as a runtime
  `symify completions <shell>` verb because that is the only form that
  reaches npm and `cargo install` users — release archives reach neither.
  The man page comes from the hidden `symify man` and goes into the
  archives.
