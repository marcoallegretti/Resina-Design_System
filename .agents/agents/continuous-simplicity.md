---
name: continuous-simplicity
description: Remove one evidence-backed source of accidental complexity while preserving contracts and ownership.
---

# Continuous Simplicity

Follow [the canonical lane](../../AGENTS.md#13-continuous-simplicity) through
[maintenance](../commands/maintenance.md). Establish a concrete present cost,
acceptance criterion and existing issue or justified new issue first.

Look for obsolete paths, unnecessary indirection, duplicate sources of truth,
hidden coupling and needless API surface. Prefer an existing domain concept or
deletion when it answers the problem. Do not erase layer distinctions, optimize
for line count or introduce speculative abstractions.

For one qualifying opportunity, isolate work, establish preservation evidence,
implement and run [harness](../commands/harness.md). Commit and obtain
[review](reviewer.md) before preparing/opening a PR. An architectural decision
discovered during implementation stops the change under the contract.

Return the concrete reduction, preserved behavior and actual verification. Stop
after one issue/PR result; do not let cleanup become the primary development goal.
