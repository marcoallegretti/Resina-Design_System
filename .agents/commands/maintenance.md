---
name: maintenance
description: Run one explicit maintenance lane without interfering with primary development.
---

# Run one maintenance lane

Input: explicitly select Architecture Debt Scout, Continuous Simplicity or
Continuous Refactoring. Read [AGENTS.md](../../AGENTS.md), especially its
maintenance isolation, issue threshold, lane lifecycles and independent review.

Load the matching role:

- [Architecture Debt Scout](../agents/architecture-debt-scout.md)
- [Continuous Simplicity](../agents/continuous-simplicity.md)
- [Continuous Refactoring](../agents/continuous-refactoring.md)

Inspect current `master` and current issues. Verify that the base is current;
fetch the configured remote when available. If it cannot be verified, report the
local revision and the limitation rather than claiming to inspect current master.
Never switch or modify the maintainer's Primary Development checkout.

Apply the issue threshold before creating work. Select one independently
actionable present problem, search for duplicates and create/update an issue
only within the invoked task's authority. Scout remains read-only with respect
to source changes. If a simplicity/refactoring change needs an architectural
decision, record the evidence and stop implementation.

For approved implementation-bearing maintenance, use one separate worktree and
one concrete `maintenance/<change-description>` branch based on current `master`.
Establish proof, implement, validate, commit, obtain independent review and
prepare/open one PR through [implement](implement.md). Check the public automatic
review configuration before publication. Stop after this one result; do not
continue collecting opportunities or merge autonomously.
