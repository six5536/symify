---
name: humanise
description: Make human-facing text read like it was written by a person, not generated. Use this whenever writing or editing prose a human will read — documentation, READMEs, UI copy, error messages, emails, release notes, blog posts, commit messages, marketing copy — even if the user doesn't say "humanise". Also use when reviewing existing text for tone.
---

# Humanise

Generated text has recognisable tells. This skill names them so you can avoid them.
The goal is not "casual" or "quirky" — it's text that sounds like one specific person
wrote it for one specific context.

## First: match the register

Before styling anything, decide what the text is:

- **Commit message / changelog**: terse, factual, no adjectives. "Fix race in session refresh" not "Resolved a critical issue to enhance session reliability."
- **Error message**: what happened, what to do next. Nothing else.
- **README / docs**: plain, direct, second person. Assume a competent reader in a hurry.
- **Email**: however the sender actually talks. Short. One ask per email.
- **Marketing**: concrete claims and specifics beat superlatives. "Deploys in under 90 seconds" not "blazing-fast, seamless deployment."

Content can be wrong for a context too, not just tone: a README doesn't need a
mission statement; an error message doesn't need an apology; a commit message
doesn't need to sell the change.

## Patterns to avoid

### 1. Antithesis / "not just X, but Y"

The single strongest tell. Also its cousins: "It's not about X. It's about Y."
and "X isn't just a Y — it's a Z."

- ❌ "This isn't just a config change — it's a rethink of how we handle state."
- ✅ "This changes how we handle state."

Use at most once in a long document, never in short text.

### 2. Rule of three

Generated text compulsively lists in threes, often with parallel adjectives.

- ❌ "The API is fast, flexible, and easy to use."
- ✅ "The API is fast. Most endpoints return in under 50ms."

If you catch yourself writing a triple, cut to the one item that matters and make it concrete.

### 3. Inflated vocabulary

Words that almost never appear in text people write by hand:
_delve, leverage, robust, seamless, streamline, harness, foster, elevate, unlock,
crucial, vital, pivotal, landscape, realm, tapestry, journey, navigate (metaphorically),
utilize, comprehensive, holistic, cutting-edge, game-changing, empower._

- ❌ "Leverage our robust SDK to seamlessly integrate payments."
- ✅ "Add payments with the SDK. Three lines of code."

Plain verbs: use, help, make, build, fix, run.

### 4. Hedging and filler

- ❌ "It's worth noting that...", "It's important to remember that...", "Generally speaking...", "In many cases..."
- ✅ Delete the phrase. Say the thing.

### 5. Empty transitions and wrap-ups

"Moreover", "Furthermore", "Additionally", "In conclusion", "Overall", "Ultimately".
Real writers mostly just start the next sentence. And they stop when they're done —
no closing paragraph that restates what was just said.

### 6. Bolded-lead bullet lists for everything

Generated text turns any explanation into bullets with a **Bold Label:** prefix.
Prose is the default. Use a list only when the content is genuinely a list
(steps, options, requirements), and even then the bullets don't need bold labels.

### 7. Uniform rhythm

Generated sentences tend to be the same length with the same structure.
Vary it. Short sentences land. Then follow with a longer one that carries the
detail, the qualification, the reason. Fragments are fine sometimes.

### 8. Em-dash overuse

One or two per document, not per paragraph. Commas and full stops usually do the job.

### 9. Throat-clearing openers and closers

- ❌ "Great question!", "I'd be happy to help with that.", "Let's dive in."
- ❌ "Whether you're a startup founder or an enterprise architect..."
- ❌ Ending with "Feel free to reach out if you have any questions!"
- ✅ Start with the substance. End when the substance ends.

### 10. Over-symmetry

Perfectly parallel headings, every section the same length, every point with
exactly one example. Humans are lopsided: they spend four paragraphs on the thing
they care about and one sentence on the thing they don't.

## What to do instead

- **Commit to claims.** "This is slower" not "this may potentially introduce some performance considerations."
- **Prefer concrete detail over abstraction.** Numbers, filenames, actual examples.
- **Use contractions** in anything conversational (docs, emails, UI). Skip them in legal or formal text.
- **Cut 20–30% on a second pass.** Generated text pads; humans compress.
- **Have a viewpoint.** If two options exist, say which one you'd pick and why, rather than presenting a balanced matrix nobody asked for.

## Worked example

Before (recognisably generated):

> It's worth noting that our new caching layer isn't just a performance improvement —
> it's a fundamental shift in how the application handles data. By leveraging Redis,
> we've been able to seamlessly reduce latency, streamline database load, and enhance
> the overall user experience. Whether you're serving ten users or ten thousand, the
> robust architecture ensures your application remains fast, reliable, and scalable.

After:

> We added a Redis cache in front of the sessions table. p95 latency on /dashboard
> dropped from 480ms to 90ms and database load is down about 60%. The cache is
> write-through, so there's no invalidation logic to get wrong.

## Self-check before finishing

1. Does it contain "not just", "delve", "leverage", "seamless", or "robust"? Rewrite.
2. Any list of exactly three parallel items? Cut or make concrete.
3. Could the first sentence be deleted with no loss? Delete it.
4. Does it end by summarising itself? Stop earlier.
5. Read it aloud: would a person actually say this to a colleague?
