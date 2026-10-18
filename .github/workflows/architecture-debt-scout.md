---
name: Architecture Debt Scout
description: Find one concrete architectural debt and record it as an issue.
on:
  workflow_dispatch:
  schedule: weekly
  skip-if-match: 'is:issue is:open label:architecture-debt-scout'
permissions:
  contents: read
  issues: read
  pull-requests: read
engine:
  id: copilot
  model: small
max-turns: 25
max-ai-credits: 300
timeout-minutes: 20
tools:
  github:
    toolsets: [issues, pull_requests, repos]
network: defaults
safe-outputs:
  create-issue:
    max: 1
    title-prefix: "[architecture] "
    labels: [architecture-debt-scout, maintenance]
    footer: false
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
      checked-out repository and confirm it exists and says what is claimed. Confirm the issue body has
      exactly the headings of the matching .github/ISSUE_TEMPLATE form, in order. Reject if any claim is
      unverifiable, any reference is wrong, the problem is intended behaviour documented in AGENTS.md,
      spec/ or conformance/, the issue duplicates an existing one, it needs an architectural decision but
      uses the maintenance form, or it mentions agents, models, workflows or automation. Approve only
      when every claim was verified by you now.
    steps:
      - name: Checkout repository for verification
        uses: actions/checkout@v6
        with:
          persist-credentials: false
imports:
  - shared/maintenance-protocol.md

---

# Architecture Debt Scout

Read AGENTS.md sections 10, 11 and 12, and .agents/agents/architecture-debt-scout.md.
Inspect only the current default branch. Challenge the boundaries listed in AGENTS.md section 12.

Apply the issue threshold. Search open and closed issues for the same underlying
problem first and stop without output if one exists. Then follow the imported
template, fact-check and hostile self-review rules. If nothing survives, stop without creating an issue.
