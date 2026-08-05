---
type: Procedure
id: release-procedure
title: Release Procedure
description: The changelog gate, the release command, the irreversible push, and the tag-driven pipeline.
status: stable
sources:
  - id: contributing
    resource: /CONTRIBUTING.md
    title: Contributing guide (releasing, authoritative walkthrough)
  - id: release-yml
    resource: /.github/workflows/release.yml
    title: Release workflow
links:
  - rel: references
    to: software-components
    note: The pipeline's job-by-job description.
---

Releases are tag-driven; the authoritative walkthrough is
[CONTRIBUTING → Releasing](/CONTRIBUTING.md). The invariants:

1. **Changelog first.** A `## [X.Y.Z]` section must exist in `CHANGELOG.md`
   before cutting a release. `npm run release` and the release workflow both
   refuse a version with no section; the section becomes the GitHub release
   notes.
2. **`npm run release X.Y.Z`** sets the version everywhere in lockstep (16
   locations including both lockfiles), verifies, commits, and tags — it
   never pushes.
3. **Review, then `git push --follow-tags`.** Pushing the tag triggers the
   publish, which cannot be undone (crates.io never; npm after 72 hours).
4. The workflow verifies, runs the full check gate, cross-builds, dry-runs
   every publish, publishes (platform packages → launcher → cargo), and
   creates the GitHub Release — see
   [software-components](software-components.md) for the job breakdown.
   Prerelease tags (`vX.Y.Z-rc.N`) publish under npm's `next` dist-tag and
   never become `latest`.

Credentials: npm uses trusted publishing (OIDC) — no token; the packages'
`0.0.0` placeholder versions exist so trusted publishers could be attached
and **must not be unpublished** (removing a package's only version can take
its trusted-publisher configuration with it). crates.io uses
`CARGO_REGISTRY_TOKEN`.
