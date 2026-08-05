# Overview

* [Project Overview](project-overview.md) - what symify is, how it ships, and its current status.
* [Domain Glossary](glossary.md) - the project's terms of art, each defined in one or two lines.
* [Known Constraints & Non-Goals](constraints-non-goals.md) - what symify deliberately does not do, what is deferred past v1, and the accepted limitations.
* [Backlog & Decided Ideas](backlog.md) - ideas under consideration and ideas decided against, with the reasoning.

# Design

* [Architecture](architecture.md) - locations, verbs × modes, the pure-planner pipeline, the per-entry state machine, and link resolution.
* [Architectural Rules](architectural-rules.md) - invariants that changes must preserve — planner purity, statelessness, never discovering files, additive copies — and the safety guards.
* [Software Components](software-components.md) - the Rust crates and their modules, the npm launcher and platform packages, the platform matrix, and the CI/CD workflows.
* [Configuration & Environments](configuration.md) - config file structure, discovery and merge order, os/host machine conditions, backup retention, and the JSON Schema that generates the Rust model and drives editor validation.
* [API Contracts](api-contracts.md) - the CLI surface and flags, config-mutation and auto-init behaviour, status labels, JSON output, and the stability promises.
* [Error Handling & Logging](error-handling.md) - exit codes, continue-on-error execution, per-entry outcome reporting, and the broken-pipe rule.
* [Directory Structure](directory-structure.md) - what lives where in the repository.
* [Technology Stack](technology-stack.md) - languages, runtime and dev dependencies, and the pinned toolchain set.

# Process

* [Coding Standards](coding-standards.md) - prose rules, Rust and TypeScript conventions, and the code-is-canonical principle.
* [Security Requirements](security-requirements.md) - the vulnerability policy in brief, and why the safety model counts as the security surface.
* [Dependency Policy](dependency-policy.md) - when a dependency may be added and how its version is chosen.
* [Testing Strategy](testing-strategy.md) - the test layers from planner units to CLI end-to-end, the key choices behind them, and the CI platforms.
* [Development Procedure](development-procedure.md) - setup, the plan-driven change workflow, and what to run before a PR.
* [Development Commands](development-commands.md) - the npm-script command set and the pre-PR check list's shape.
* [Definition of Done](definition-of-done.md) - what a change must satisfy before it merges.
* [Release Procedure](release-procedure.md) - the changelog gate, the release command, the irreversible push, and the tag-driven pipeline.
