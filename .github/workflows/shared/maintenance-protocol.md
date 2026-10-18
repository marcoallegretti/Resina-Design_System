---
---

## Template

The issue body must reproduce, in order and with identical headings, the fields
of the repository issue form named by the workflow: `.github/ISSUE_TEMPLATE/maintenance.yml`
for simplicity and refactoring, `.github/ISSUE_TEMPLATE/architecture.yml` for
architecture debt. Read that file first. Do not add, rename or omit sections.

## Fact-check (continuous)

Treat every sentence you intend to publish as a claim that must be verified now,
against the current default branch, not from memory or from earlier issues.

1. List each claim in a private scratch list: file path, line range, what it asserts.
2. Open the file and confirm each reference exists and says what you claim.
   Confirm quoted symbols, counts and line numbers by reading or searching, never by estimating.
3. Check that the claimed external fact (standard, upstream behaviour) is backed by
   repository evidence or a primary source you actually fetched. Otherwise remove it.
4. Remove any claim you could not verify. Do not hedge it into the text.

## Hostile self-review (always)

Before creating the issue, argue against it as an adversary whose goal is to reject it:

- Is the "problem" intentional, documented in AGENTS.md, spec/ or conformance/, or required by a layer boundary?
- Is the cost present today, or hypothetical?
- Does the proposed outcome change normative behaviour, a public contract or ownership?
  If yes it is architectural: use the architecture form or stop.
- Does an open or closed issue, or a recent commit, already cover it?
- Would the acceptance criterion pass trivially or be unverifiable?
- Which of your own beliefs did you accept without reading the code?

Create the issue only if no objection survives. If any objection stands, or any
claim stays unverified, produce no issue. Producing nothing is a correct outcome.

Never mention agents, models, workflows, automation or this review process in the issue text.
