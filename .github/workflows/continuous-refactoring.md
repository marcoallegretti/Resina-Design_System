---
name: Continuous Refactoring
description: Propose one concrete structural refactoring as an issue.
on:
  workflow_dispatch:
  skip-if-match: 'is:issue is:open label:continuous-refactoring'
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
    title-prefix: "[refactoring] "
    labels: [continuous-refactoring, maintenance]
    footer: false
    deduplicate-by-title: true
imports:
  - shared/maintenance-protocol.md
---

# Continuous Refactoring

Read AGENTS.md sections 10, 11 and 14, and .agents/agents/continuous-refactoring.md.
Inspect only the current default branch. If the change needs an architectural decision, it belongs to the architecture lane: stop and produce nothing.

Apply the issue threshold. Search open and closed issues for the same underlying
problem first and stop without output if one exists. Then follow the imported
template, fact-check, hostile self-review and language rules. If nothing survives, stop without creating an issue.
