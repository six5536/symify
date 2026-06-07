# Security Policy

## Supported versions

symify is pre-1.0 and not yet released. Security fixes target the latest release
(once published) and the `main` branch; there are no supported older versions.

## Reporting a vulnerability

Please report security issues **privately** through GitHub's private
vulnerability reporting:

1. Open the [Security tab](https://github.com/six5536/symify/security) of the
   repository.
2. Click **Report a vulnerability** to start a private advisory.

Don't open a public issue for a suspected vulnerability. We'll acknowledge the
report, investigate, and coordinate a fix and disclosure with you.

## Scope

symify moves, links, and (under `conflict = "replace"`) deletes files, so its
safety model is part of its security posture: it never discovers files, refuses
protected roots, refuses to run as root without `--allow-root`, and confirms
unrecoverable deletes. That model is documented in the README "Safety" section
and in [specs/ARCHITECTURE.md](specs/ARCHITECTURE.md#safety). Reports that
demonstrate a way around those guarantees are in scope.
