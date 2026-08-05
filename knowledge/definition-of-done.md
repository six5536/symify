---
type: Convention
id: definition-of-done
title: Definition of Done
description: What a change must satisfy before it merges.
status: stable
sources:
  - id: pr-template
    resource: /.github/PULL_REQUEST_TEMPLATE.md
    title: PR checklist
links:
  - rel: references
    to: testing-strategy
    note: The layers behind the test and coverage requirements.
---

A change is done when (the enforced form is the
[PR checklist](/.github/PULL_REQUEST_TEMPLATE.md)):

- Formatting, clippy (`--all-targets -- -D warnings`), tests, and doctests
  pass.
- Line coverage stays ≥ 90% **per crate** — see
  [testing-strategy](testing-strategy.md).
- The schema codegen drift check passes if the schema changed; launcher and
  version-consistency checks pass.
- Documentation is updated wherever behaviour changed: README, this
  knowledgebase, rustdoc.
- New behaviour carries tests at the appropriate layer; bug fixes carry a
  regression test that fails on the unfixed code.
