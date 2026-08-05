---
type: Procedure
id: development-procedure
title: Development Procedure
description: Setup, the plan-driven change workflow, and what to run before a PR.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide
links:
  - rel: references
    to: development-commands
    note: The commands this procedure runs.
  - rel: references
    to: definition-of-done
    note: The bar a change must meet.
---

Setup is `mise install` + `npm install`; detail in
[CONTRIBUTING](/CONTRIBUTING.md). A plain `cargo build` needs neither Node
nor `typify` (the generated config model is checked in).

# Workflow

1. Significant changes start as a plan: `plans/PLAN-NNN-<name>.md`, worked
   through before implementation. Plans are retained permanently as a record
   once landed.
2. Implement with focused commits, using
   [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
   `fix:`, `docs:`, `test:`, `refactor:`, `chore:`).
3. Update this knowledgebase when behaviour or design changes.
4. Before a PR, run the full CI-equivalent check list (see
   [development-commands](development-commands.md)) and meet
   [definition-of-done](definition-of-done.md). CI runs tests on macOS and
   Windows, and the coverage gate on Linux.
