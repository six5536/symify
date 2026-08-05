---
type: Policy
id: dependency-policy
title: Dependency Policy
description: When a dependency may be added and how its version is chosen.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (dependencies)
links:
  - rel: references
    to: technology-stack
    note: The current approved set.
---

- **Always ask before adding a dependency.** A new one needs a clear reason;
  reach for the standard library or a crate already in the tree first. The
  current set is in [technology-stack](technology-stack.md).
- **Always check the latest version (as of 7 days ago)** and use that, unless
  instructed otherwise.
- `cargo-deny` gates licences, bans, and sources in CI; advisories run on a
  schedule and open an issue rather than failing unrelated builds.
