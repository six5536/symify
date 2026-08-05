---
type: Overview
id: project-overview
title: Project Overview
description: What symify is, how it ships, and its current status.
status: stable
resource: /README.md
sources:
  - id: readme
    resource: /README.md
    title: README
links:
  - rel: references
    to: architecture
    note: The design behind the summary.
---

symify is a CLI tool that keeps files in a working location (`live`, e.g.
`~`) in sync with a managed backing repository (`store`, e.g. `~/dotfiles`),
using symlinks or incremental copies. Managing dotfiles is the common use;
it applies to any files to be mirrored, backed up, or deployed across
machines. How it works is in [architecture](architecture.md); the
user-facing introduction is the [README](/README.md).

Seven verbs: `add`, `remove`, `list`, `sync`, `deploy`, `status`, `diff`.
Safe by
default: overwrites are backed up, copies are additive, and nothing outside
the config is ever touched.

Ships two ways:

- **npm** (`npm i -g symify`) — prebuilt binaries for Linux and macOS, x64
  and arm64.
- **cargo** (`cargo install symify`) — build from source; also the route on
  platforms without a prebuilt binary.

# Status

Released; pre-1.0. Minor versions may carry breaking changes, and
`symify-core`'s API is not stable yet. Windows is designed for but not
shipped.
