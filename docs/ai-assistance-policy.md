# Using AI assistants on this codebase

AI assistants are useful here and I use them daily. They are also wrong in ways
ordinary tools are not, and those ways matter more as the consequences of a
defect grow. This document states how they are used in this repository, why the
rules are what they are, and how the rules should loosen as confidence grows.

## What makes an assistant different from a compiler

A compiler that mis-compiles is a bug, reproducible and eventually fixed. An
assistant fails differently:

- **It is confident while wrong.** Output carries no signal about its own
  reliability. Plausible and correct look identical.
- **It is not deterministic.** The same prompt can produce different answers,
  so a result that was checked once is not guaranteed on the next run.
- **It loses detail over length.** In a long file or a long conversation,
  constraints stated early quietly stop being honoured.

None of these are reasons not to use one. They are reasons the output cannot be
the last step. Something has to check what went in, constrain how it is used,
verify what came out, and keep a record of all three.

## The rules

**1. Declare it.** A commit developed with AI assistance carries a trailer:

```
AI-Assisted: claude-code
```

Not as a confession — as metadata. A reviewer should know which changes warrant
closer reading, and six months later it should be possible to ask which parts
of the codebase came from where.

**2. A human reviews it.** The same commit carries:

```
Reviewed-by: Arsham Hosseini
```

The reviewer is stating that they understand the change and can explain it
without the assistant present. If that is not true, it is not reviewed.

**3. Record it.** The approved change is appended to
[`ai-assistance-log.md`](ai-assistance-log.md). A pull request discussion
disappears into a closed tab; the log does not. If this codebase ever had to be
audited, the question "which changes were machine-written, and who accepted
them?" should have a written answer.

**4. The build enforces all three.** `tools/ai_review_gate.py` runs on every
pull request, in both GitHub Actions and GitLab CI. A commit declared as
AI-assisted with no reviewer fails. Reviewed but unrecorded fails. The gate does
not judge the code — it only insists the process happened.

## What the gate does not do

It is worth being explicit, because a safeguard that is trusted beyond its
actual reach is worse than none.

- It cannot detect undeclared AI use. Rule 1 is honour-based; the gate enforces
  what follows from the declaration, not the declaration itself.
- It does not assess code quality. Tests, linting and review do that.
- It does not distinguish a one-line completion from a generated module, though
  those carry very different risk. A later revision should.

## Staged rollout

The rules above are stage 2. The intended progression:

| Stage | Assistant's role | Human's role | Move on when |
|---|---|---|---|
| 1 | Explains, reviews, suggests. Writes nothing that ships. | Writes all code. | The team can tell useful suggestions from plausible ones. |
| 2 *(current)* | Drafts code and tests. | Reviews every line; declares, reviews, records. | The log shows a sustained period with no defect traced to unreviewed assistance. |
| 3 | Drafts in low-consequence areas unsupervised — docs, test fixtures, tooling. | Reviews normally; full scrutiny stays on anything shipped. | Low-consequence areas stay clean and the boundary holds. |
| 4 | Drafts anywhere. | Reviews proportionally to blast radius rather than to origin. | — |

Each stage widens only on evidence, and the previous way of working stays
available throughout. Nothing here requires going further than stage 2; stopping
is a legitimate outcome.

## Why bother on a personal project

Because the habit is the point. A rule adopted when nothing is at stake is one
you already have when something is.
