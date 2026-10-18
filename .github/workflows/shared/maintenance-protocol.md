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
    steps:
      - name: Checkout repository for verification
        uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
        with:
          persist-credentials: false
      - name: Check proposed issue
        env:
          GH_TOKEN: ${{ github.token }}
        run: python3 tools/check_maintenance_issue.py --agent-output /tmp/gh-aw/threat-detection/agent_output.json
---

## Template

The issue body must reproduce, in order and with identical headings, the fields
of the repository issue form named by the workflow: `.github/ISSUE_TEMPLATE/maintenance.yml`
for simplicity and refactoring, `.github/ISSUE_TEMPLATE/architecture.yml` for
architecture debt. Read that file first. Render each field label as a level-3 heading (### Label), in form order.
Do not add, rename or omit sections, do not put text before the first heading,
and do not copy the form's introductory markdown text.

## Fact-check (continuous)

Treat every sentence you intend to publish as a claim that must be verified now,
against the current default branch, not from memory or from earlier issues.

1. List each claim in a private scratch list: file path, line range, what it asserts.
2. Open the file and confirm each reference exists and says what you claim.
   Confirm quoted symbols, counts and line numbers by reading or searching, never by estimating.
3. Cite files by their full repository-relative path, and line ranges only as
   `path:start-end` or `path:line`.
4. External network access is unavailable. Claims about standards or upstream behaviour
   must be backed by repository evidence; otherwise remove them.
5. Remove any claim you could not verify. Do not hedge it into the text.

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

## Language

Describe only the repository problem and its evidence. Never describe how the
issue was produced: no mention of agents, models, AI, Copilot, LLMs, gh-aw,
automation, this run or this review process. Repository files such as AGENTS.md,
`.agents/` or `.github/workflows/reference.yml` may be cited by path when they are evidence.

Before publication, `tools/check_maintenance_issue.py` rejects an issue whose
headings differ from the form, that cites a missing path or line range, that
uses the vocabulary above, or whose title matches an existing issue.
