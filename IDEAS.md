# Backup

- Grandfather, father, son - auto backup
- Decided against (2026-06): out of scope. The store is meant to be a git repo,
  and git already gives you full history; restic and borg cover the non-git
  case. A built-in GFS snapshotter would duplicate both and fight the stateless
  "never discovers files" design. README points at git/restic instead.
  It would have fixed one real wart: `.bak` files accumulate without bound on
  repeated conflicts. A small bounded-retention setting could still be worth it.

# Docs / tooling

- ~~Man page + shell completions, generated from the clap definition
  (`clap_mangen` / `clap_complete`).~~ **Done (PLAN-006).** Shipped as a runtime
  `symify completions <shell>` verb, which is the only form that reaches npm and
  `cargo install` users — release archives reach neither. The man page comes
  from a hidden `symify man` and goes into those archives.
