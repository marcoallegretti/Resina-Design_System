---
name: spec
description: Establish a reported problem or proposal and draft verifiable contribution scope.
---

# Establish the scope

Input: a report, desired behavior or proposal. Read [AGENTS.md](../../AGENTS.md)
and the relevant [skills](../README.md#skills). This workflow produces a draft;
it does not implement a proposal or authorize publication.

1. Inspect the relevant normative artifacts and implementation. Reproduce the
   reported behavior where practical. Separate observations from assumptions.
2. Name the owning layer, present affected party and acceptance criterion. Use
   the contract's issue threshold; stop without inventing work when it is unmet.
3. Search existing issues when access is available. If access is unavailable,
   say duplicate checking is incomplete rather than claiming no duplicate exists.
4. For an architectural decision, establish repository evidence and verify
   material external assumptions before writing alternatives and trade-offs.
   Link the applicable specification and upstream evidence. Mark implementation
   as awaiting the maintainer's decision.
5. Draft against the appropriate form: [ordinary change](../../.github/ISSUE_TEMPLATE/change.yml),
   [maintenance](../../.github/ISSUE_TEMPLATE/maintenance.yml), or
   [architecture](../../.github/ISSUE_TEMPLATE/architecture.yml).

Include observed and expected behavior, evidence, affected layer/boundary,
present impact, acceptance criterion and expected proof. State non-goals when
needed to bound the task. Name the failing test, independent vector or manual
check that would demonstrate the difference. A claim without a verification
surface is unfinished scope.

Return the draft and any missing evidence or required architectural decision.
Create or update an issue only when that action is part of the authorized task.
