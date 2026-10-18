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
Inspect only the current default branch. Follow the imported scope: this run performs
only the issue-filing step of its lane, using the maintenance form.
The isolated worktree, branch, refactoring, review and pull request steps are out of scope.
If the change needs an architectural decision, call `noop`; the architecture lane files those.

Apply the issue threshold, then the imported duplicate, template, fact-check,
hostile self-review and language rules. If nothing survives, call `noop` with the reason.
