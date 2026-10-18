---
safe-outputs:
  report-failure-as-issue: false
  report-failed-jobs: false
  noop:
    report-as-issue: false
  missing-tool:
    create-issue: false
  missing-data:
    create-issue: false
  report-incomplete:
    create-issue: false
  threat-detection:
    enabled: true
    engine:
      id: copilot
      model: small
      max-turns: 12
    continue-on-error: false
    report-as-issue: false
    prompt: |
      You are an adversarial fact-checker. You did not write the proposed issue; reject it if you can.
      For every file path, line range, symbol, count and quoted text in the issue, open the file in the
      checked-out repository and confirm it exists and says what is claimed. The body must contain, in
      this order, the level-3 headings (### Label) of the fields of the form
      .github/ISSUE_TEMPLATE/maintenance.yml (Observed, Expected, Evidence, Affected layer or boundary,
      Why it matters now, Acceptance criterion, Expected proof) or of architecture.yml for an architecture
      issue, and no other sections. Reject if any claim is unverifiable, any reference is wrong, the problem
      is intended behaviour documented in AGENTS.md, spec/ or conformance/, a maintenance-form issue needs
      an architectural decision, or the text mentions agents, models, workflows or automation.
      Approve only when every claim was verified by you now.
    steps:
      - name: Checkout repository for verification
        uses: actions/checkout@v6
        with:
          persist-credentials: false
---

## Template

The issue body must reproduce, in order and with identical headings, the fields
of the repository issue form named by the workflow: `.github/ISSUE_TEMPLATE/maintenance.yml`
for simplicity and refactoring, `.github/ISSUE_TEMPLATE/architecture.yml` for
architecture debt. Read that file first. Render each field label as a level-3 heading (### Label), in form order.
Do not add, rename or omit sections, and do not copy the form's introductory markdown text.

## Fact-check (continuous)

Treat every sentence you intend to publish as a claim that must be verified now,
against the current default branch, not from memory or from earlier issues.

1. List each claim in a private scratch list: file path, line range, what it asserts.
2. Open the file and confirm each reference exists and says what you claim.
   Confirm quoted symbols, counts and line numbers by reading or searching, never by estimating.
3. External network access is unavailable. Claims about standards or upstream behaviour
   must be backed by repository evidence; otherwise remove them.
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
