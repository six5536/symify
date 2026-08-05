---
type: Policy
id: security-requirements
title: Security Requirements
description: The vulnerability policy in brief, and why the safety model counts as the security surface.
status: stable
sources:
  - id: security-md
    resource: /SECURITY.md
    title: Security policy
links:
  - rel: references
    to: architectural-rules
    note: The guards that make up the security surface.
---

The full policy is [SECURITY.md](/SECURITY.md). In brief: vulnerabilities are
reported privately via GitHub's private vulnerability reporting, never as
public issues; fixes target the latest release and `main` only (pre-1.0, no
backports).

symify's attack surface is the filesystem, not the network: it takes no
network input and makes no connections. Because it moves, links, and (under
`conflict = "replace"`) deletes files, the safety model **is** the security
model — the guards in [architectural-rules](architectural-rules.md) (no file
discovery, protected roots, root refusal, delete confirmation) are security
guarantees, and anything that demonstrates a way around them is in scope for
a vulnerability report.

Requirements for changes:

- No change may weaken a guard without explicit sign-off; guard behaviour is
  part of the public contract.
- New verbs or flags that mutate the filesystem must go through the planner
  guards and the delete-confirmation gate.
