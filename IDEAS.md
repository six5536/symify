# Backup
- Grandfather, father, son - auto backup
- Decided against (2026-06): out of scope. symify is a dotfiles manager whose
  store is meant to be a git repo — git already provides full history, and
  restic/borg cover non-git stores. A built-in GFS snapshotter would duplicate
  those, fight the stateless "never discovers files" design, and add a large
  surface for little gain. README points users at git/restic instead.
  (The one real wart it would have fixed — unbounded `.bak` accumulation on
  conflict — could still be worth a small bounded-retention feature later.)

# Docs / tooling

- ~~Man page + shell completions, generated from the clap definition
  (`clap_mangen` / `clap_complete`).~~ **Done (PLAN-006).** Shipped as a runtime
  `symify completions <shell>` verb — the only form that reaches npm and
  `cargo install` users, since release archives reach neither — plus a hidden
  `symify man` used to generate the man page into the release archives.

# Brand

- symc
- symk
- symsync
- slynk
