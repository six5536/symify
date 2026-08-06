---
type: Reference
id: constraints-non-goals
title: Known Constraints & Non-Goals
description: What symify deliberately does not do, what is deferred past v1, and the accepted limitations.
status: stable
sources:
  - id: plan-src
    resource: /crates/lib/symify-core/src/plan.rs
    title: Planner source
links:
  - rel: references
    to: software-components
    note: Platform-matrix detail behind the Windows constraint.
---

# Non-goals

- **No file discovery.** symify acts only on config-listed keys; "track
  everything under `live`" will not be added.
- **No deletion propagation.** Copies are additive; removing a stale store
  copy after deleting its source is a manual `rm` / `git rm`.
- **Stow-style per-file directory folding** (one link per file inside a
  directory) is out of scope for v1; directories link as one unit.
- **No auto-download or build-from-source fallback** in the npm launcher on
  unsupported platforms; the message points at `cargo install symify`.

# Constraints

- **Windows symlink mode needs privilege** (Developer Mode or elevation) and
  NTFS; without it, symlink entries fail with guidance and `copy` mode is the
  fallback. x64 only — native arm64 waits until CI can execute it; detail in
  [software-components](software-components.md).
- **Pre-1.0**: minor versions may carry breaking changes; `symify-core`'s
  API is not stable.
- **Cross-registry publishing is not atomic**; the release pipeline is
  ordered, dry-run-gated and recoverable instead.
- zigbuild's musl output is non-PIE; accepted for a local CLI with no network
  input.

# Deferred (post-v1)

- `clean` verb + optional state manifest for orphan removal (currently
  stateless).
- Stow-style per-file directory folding.
- Native Windows arm64 (blocked on a CI runner that can execute it); relative
  symlink targets as an opt-in.
- `--json`-driven richer tooling; npm download/build fallback.
