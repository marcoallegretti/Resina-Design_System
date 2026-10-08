---
name: implement
description: Carry accepted scope through proof, implementation, validation and review.
---

# Implement accepted scope

Input: an issue or an explicit accepted task with an acceptance criterion.
Read [AGENTS.md](../../AGENTS.md), [CONTRIBUTING.md](../../CONTRIBUTING.md) and
the relevant [skills](../README.md#skills).

1. Verify scope from repository evidence. An issue is not architectural approval.
   Resolve missing acceptance criteria with [spec](spec.md). If a reserved
   architectural decision is unresolved, prepare the evidence and stop dependent
   implementation as required by the contract.
2. Inspect Git status, branch, worktrees and remotes. Use an isolated contribution
   branch/worktree; never commit external contributions directly to `master` or
   overwrite unrelated work. Use [maintenance](maintenance.md) for maintenance.
3. Identify the owning layer and required proof. Reproduce a bug before fixing
   it. For contract additions, establish an independently evaluable vector,
   schema case or invariant before or alongside implementation.
4. Implement the smallest complete change. Keep normative contracts, reference
   behavior, conformance and affected backends coherent. Inspect documentation
   claims and current capability/fallback behavior.
5. Run [harness](harness.md). Inspect changed expected output and explain its
   contractual basis. Fix failures within scope; do not approve new output merely
   because the implementation produces it.
6. Inspect the complete diff and Git status. Commit atomically on the isolated
   branch, following the contract's commit rules. Keep generated evidence and
   scratch material out of source commits.
7. Have [reviewer](../agents/reviewer.md) inspect the committed range against the
   acceptance criterion. Independent review is required for maintenance when
   supported; otherwise use the contract's fresh adversarial pass and report the
   limitation honestly. Address blocking findings and revalidate changed paths.
8. Prepare the [PR template](../../.github/pull_request_template.md). Check
   automatic-review configuration before any publication that can trigger it, as
   required by the contract. If pushing/opening a PR is authorized and available,
   do so; otherwise return the local result and concrete publication limitation.

Report what changed, what proves it, checks actually run and material limitations.
Separately actionable present problems go through the issue threshold; do not
store future work solely in a PR body. Completion of a contribution is not
authorization to merge or promote a release.
