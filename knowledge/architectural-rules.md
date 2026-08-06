---
type: Convention
id: architectural-rules
title: Architectural Rules
description: Invariants that changes must preserve — planner purity, statelessness, never discovering files, additive copies — and the safety guards.
status: stable
sources:
  - id: plan-src
    resource: /crates/lib/symify-core/src/plan.rs
    title: Planner source (guards in plan::guard_reason)
  - id: confirm-src
    resource: /crates/app/symify/src/confirm.rs
    title: Delete-confirmation gate
links:
  - rel: references
    to: architecture
    note: The mechanisms these rules constrain.
---

Rules that hold across the codebase, in force wherever the mechanisms of
[architecture](architecture.md) are touched:

- **The planner is pure.** A function of *(merged config + FS state)* to an
  ordered action list; it reads the filesystem but never mutates. Anything
  interactive (prompts) or destructive lives in the binary or executor.
- **Stateless.** No manifest, no persisted record of what symify created;
  every run derives everything from config + FS.
- **symify never discovers files.** The planner touches only the exact keys
  written in config — no glob, no directory walk, no "track everything under
  `live`". One narrow, signed-off carve-out (2026-08-05): when writing a new
  backup with `backup_keep` set, the planner reads the target's parent
  directory for names matching exactly `<leaf>.<14-digit-timestamp>.bak` of
  the entry's own leaf, to prune the oldest beyond the cap. The pattern can
  never match user files, other entries' backups, or hand-renamed backups,
  and the prune runs only while a new backup is being written.
- **Copies are additive.** Destination-only files are never deleted; the only
  deletes symify emits are governed by the `conflict = replace` policy on a
  listed entry, or by a `backup_keep` prune of the entry's own stale backups
  (the carve-out above).
- **Backups protect the side being overwritten**, whichever verb runs.
- **Continue on error, no rollback.** Entries are independent; backups are the
  recovery path.

# Safety guards

symify moves, links, and (under `replace`) deletes files, so it constrains
what it will act on.

**Planner guards** — refused as a per-entry `Failed` (so `status`, `sync`,
`deploy`, and `add` all agree; `add` aborts before editing config). Shared by
`plan` and `status` via `plan::guard_reason`:

- **Protected roots (sentinels).** Refuse an entry whose resolved `live` or
  `store` equals `/`, `$HOME`, or the mapping's own `live`/`store` root. Kills
  `symify add ~` and similar whole-home captures.
- **Store containment.** Refuse an entry whose `live` equals or is an ancestor
  of the `store` root — adopting it would pull the store into itself. (This is
  the one ancestor check kept, because containing the store is never safe; it
  has no false positives for ordinary entries.)
- **Out-of-root ⇒ file-only.** Anything resolving *outside* the live root must
  be a single file, not a directory, on either side. A leaf like `/etc/hosts`
  is fine; `/etc` (a tree) is refused. This preserves the absolute-key feature
  while preventing system-tree captures, even under `sudo`.

**Config validation** — at resolve time, a mapping whose `live` and `store`
resolve to the same directory is rejected outright (every entry would be both
source and destination). Note the default layout deliberately *nests* `store`
under `live` (`~` / `~/dotfiles`); that is fine — only equality is refused.

**Refuse to run as root.** The mutating verbs (`sync`/`deploy`/`add`/`remove`)
refuse when `euid == 0` unless `--allow-root` is passed; `status`/`list` are
always allowed. (`geteuid` is declared directly — no `libc` dependency.
Windows is a no-op pending a future milestone.)

**Confirmation for unrecoverable deletes.** The only unrecoverable op symify
emits is a recursive delete of a non-empty directory, produced by
`conflict = replace` or by a `backup_keep` prune of a stale directory backup.
Before executing such a plan it requires confirmation:
an interactive `[y/N]` prompt (default No) on a TTY, or `--yes`.
Non-interactive runs (piped, `--json`, CI) are *refused* unless `--yes` is
given, so a script can never silently trigger one. `--dry-run` never prompts
and shows the deletes in its preview. A `relink` (removing content
byte-identical to the other side) is recoverable and therefore never gated.
This lives in the binary (`confirm.rs`); the planner stays pure.
