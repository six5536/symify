---
type: Reference
id: directory-structure
title: Directory Structure
description: What lives where in the repository.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (project layout)
links:
  - rel: references
    to: software-components
    note: What the crates and packages contain.
---

```
crates/lib/symify-core/   # all domain logic (no arg parsing)
crates/app/symify/        # the binary: CLI parsing, wiring, output
packages/                 # npm launcher + per-platform binary packages
schema/                   # hand-authored JSON Schema, source of truth
knowledge/                # this AOKF bundle — canonical project knowledge
plans/                    # PLAN-NNN change plans; retained permanently
.agents/                  # AOKF spec and agent-facing config (PROSE.md)
.github/workflows/        # checks.yml (reusable), ci.yml, release.yml, audit.yml
.devcontainer/            # dev container definition
```

Crate and package contents are detailed in
[software-components](software-components.md). Top-level docs (README,
CONTRIBUTING, CHANGELOG, SECURITY, CODE_OF_CONDUCT) are the public,
GitHub-surfaced files; AGENTS.md is the agent entry point that loads this
bundle.
