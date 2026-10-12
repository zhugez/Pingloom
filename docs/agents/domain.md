# Domain Docs

## Layout

Single-context:
- `CONTEXT.md`: root domain glossary.
- `docs/adr/`: architectural decisions.

## Before exploring

Read root `CONTEXT.md` and ADRs relevant to the area being explored.
If a root `CONTEXT-MAP.md` exists, follow it to relevant context docs.

If any of these files are absent, proceed silently. Domain-modeling
creates them lazily when terms or decisions are resolved.

## Vocabulary

Use the glossary’s terms in issues, proposals, hypotheses, and tests.
For missing concepts, reconsider the terminology or note the gap for
domain-modeling.

## ADR conflicts

Explicitly flag recommendations that contradict an existing ADR,
identifying the decision and why it is worth reopening.
