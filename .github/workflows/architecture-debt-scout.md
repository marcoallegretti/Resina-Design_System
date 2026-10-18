---
name: Architecture Debt Scout
description: Find one concrete architectural debt and record it as an issue.
on:
  workflow_dispatch:
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
    deduplicate-by-title: true
imports:
  - shared/maintenance-protocol.md
---

# Architecture Debt Scout

Read AGENTS.md sections 10, 11 and 12, and .agents/agents/architecture-debt-scout.md.
Inspect only the current default branch. Challenge the boundaries listed in AGENTS.md section 12.
Follow the imported scope: this lane files at most one architecture issue and makes no source changes.

Apply the issue threshold, then the imported duplicate, template, fact-check,
hostile self-review and language rules. If nothing survives, call `noop` with the reason.
