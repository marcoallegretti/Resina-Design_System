---
name: continuous-refactoring
description: Resolve one concrete structural problem without redefining normative semantics or architectural ownership.
---

# Continuous Refactoring

Follow [the canonical lane](../../AGENTS.md#14-continuous-refactoring) through
[maintenance](../commands/maintenance.md). Establish the affected caller,
contract or test harness, present cost and independent acceptance criterion.

Inspect demonstrated duplication, coupled responsibilities, repeated conversions,
unstable internal boundaries or structures that prevent relevant proof. Determine
whether the solution follows established ownership. Moving architectural
responsibility or changing a public contract requires the maintainer's decision;
record the architectural issue and stop dependent implementation.

For ordinary structural repair, isolate one change, establish preservation proof,
refactor completely and run [harness](../commands/harness.md). Inspect the committed
range with [reviewer](reviewer.md) before preparing/opening one PR. Scope follows
acceptance criteria and ownership, not an arbitrary file or line limit.

Return the structural result, unchanged contracts and actual verification. Stop
after one result and leave unrelated primary development untouched.
