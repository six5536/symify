# Validation

After any change under `knowledge/`, run the AOKF validator and fix every
error before moving on:

```
python3 .agents/aokf/tools/validator.py knowledge
```

It checks the bundle against `.agents/aokf/SPEC.md` (document check plus the
conformance ladder) and must PASS at level 2. Warnings don't fail the run but
usually mean a rename the bundle missed; fix the reference, not the target.

A PostToolUse hook in `.claude/settings.json` runs this automatically after
every Edit/Write under `knowledge/` in Claude Code and blocks on errors. The
hook does not cover scripted or manual edits — run the command yourself after
those.
