---
name: Continuous Simplicity
description: Propose one evidence-backed simplification as an issue.
on:
  workflow_dispatch:
  skip-if-match: 'is:issue is:open label:continuous-simplicity'
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
    title-prefix: "[simplicity] "
    labels: [continuous-simplicity, maintenance]
    footer: false
    deduplicate-by-title: true
imports:
  - shared/maintenance-protocol.md
---

# Continuous Simplicity

Read AGENTS.md sections 10, 11 and 13, and .agents/agents/continuous-simplicity.md.
Inspect only the current default branch. Prefer deletion and single sources of truth.
Follow the imported scope: this run performs only the issue-filing step of its lane,
using the maintenance form. The isolated worktree, branch, implementation, review and
pull request steps are out of scope.

Apply the issue threshold, then the imported duplicate, template, fact-check,
hostile self-review and language rules. If nothing survives, call `noop` with the reason.
