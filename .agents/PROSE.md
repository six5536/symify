# Writing Prose

Write concisely, like a person, not a generator. Follow these rules for all
prose (comments, docs, commit messages, explanations):

- One idea per sentence. Prefer sentences under 20 words.
- Delete filler: "In order to" → "To". "It is worth noting that" → delete.
- No preamble or postamble. Start with the answer. Don't summarize what you
  just said.
- Don't restate the code in comments. Comment only the _why_, never the _what_.
- Don't hedge ("might", "could potentially") unless genuinely uncertain.
  Commit to claims: "This is slower", not "may impact performance".
- Don't narrate your process ("Now I will...", "Let me...", "First, I'll...").
- Never write "not just X, but Y". Say Y.
- Banned words: delve, leverage, robust, seamless, streamline, comprehensive,
  crucial, utilize, holistic, elevate. Use plain verbs: use, fix, make, run.
- No triples ("fast, flexible, and easy"). Pick the one that matters, make it
  concrete: "Most endpoints return in under 50ms."
- No empty transitions ("Moreover", "Furthermore") or wrap-ups ("Overall").
  Just start the next sentence. Stop when done.
- Prose is the default. Use a list only for genuine lists (steps, options),
  without bold-label prefixes.
- Prefer concrete detail: numbers, filenames, real examples.
- Match the register: commit messages terse and factual; error messages say
  what happened and what to do next; docs assume a competent reader in a hurry.
- Target: could a busy senior engineer skim this in 10 seconds and get everything?

Note: Concise means no filler — never omit warnings, caveats about breaking
changes, or assumptions you made.
