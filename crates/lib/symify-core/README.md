# symify-core

Core library for [**symify**](https://crates.io/crates/symify) — a dotfiles-style
file manager that keeps a working location in sync with a backing repository, as
symlinks or copies.

> This crate is the internal engine behind the `symify` CLI. Most users want the
> command-line tool — install [`symify`](https://crates.io/crates/symify), not
> this crate. The library API is published mainly so the binary can depend on a
> released version, and is **not** yet considered stable.

## What it does

`symify-core` is layered and the planner is pure — a function of *(merged config
+ current filesystem state)* that emits an ordered list of actions but never
mutates:

- **`config`** — discover, load, and merge TOML; expand `~`/env; resolve to an
  absolute model.
- **`plan`** — turn the resolved config plus filesystem state into a pure
  `Vec<Action>` for a `sync` or `deploy`.
- **`fs`** — the executor and the platform abstraction for link/copy/move/backup.
- **`status`** — derive per-entry status labels from the plan.
- **`model`** — config types generated from `schema/symify.schema.json`.

See the [API docs on docs.rs](https://docs.rs/symify-core) and the
[architecture overview](https://github.com/six5536/symify/blob/main/specs/ARCHITECTURE.md)
for the full design.

## License

MIT — see [LICENSE](https://github.com/six5536/symify/blob/main/LICENSE).
