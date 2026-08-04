# Human Language

- Be concise without losing information
- Use plain language

# Architecture

- See /specs/ARCHITECTURE.md

# Contributing

- See /CONTRIBUTING.md for setup, the test layers, and the release procedure.

# Releasing

- Add a `## [X.Y.Z]` section to /CHANGELOG.md **before** cutting a release.
  `npm run release` and the release workflow both refuse a version with no
  section, and that section becomes the GitHub release notes.
- Then `npm run release <version>` (bumps, verifies, commits, tags — never
  pushes) and review before `git push --follow-tags`. Pushing the tag publishes
  irreversibly.

# Dependency Rules

- Always ask before adding dependencies
- Always check for the latest version (from 7 days ago) and use that unless instructed otherwise

# TypeScript Rules

- Only use index.ts when necessary, otherwise name files with descriptive names.

# Project Status

- Released; pre-1.0, so minor versions may carry breaking changes.
- `symify-core`'s API is not stable yet.
