---
name: resina-documentation
description: Use for public Resina documentation, examples, contributor workflows, skills and verification scripts.
---

# Public documentation

Read [AGENTS.md](../../../AGENTS.md) and the affected source/contract before making
a capability or API claim. Consult [README.md](../../../README.md),
[CONTRIBUTING.md](../../../CONTRIBUTING.md) and current backend evidence as needed.

Keep blueprint intent, implemented normative behavior, reference evidence,
backend realization and product/component completeness distinct. Verify a named
command or API from actual source and run examples where practical. A generated
artifact is not normative, and a capture does not establish interaction or native
accessibility support.

Link durable decisions and requirements to their owning public artifact.
`AGENTS.md` remains the single policy source. Toolkit files offer procedures and
area knowledge rather than a competing policy, provider-specific permissions,
internal planning log or conversation transcript.

For toolkit changes, update [the catalog](../../README.md) and any affected links,
then run `python .agents/scripts/verify.py toolkit`. Check instructions manually
against the contract and CI; link/metadata validation cannot establish their
semantic truth. For executable verification changes, test failure propagation,
unknown inputs and execution from another working directory.

Write issues, PRs and commit descriptions for future contributors: concrete
problem, resulting behavior and evidence. Apply the contract's publication rules
and keep temporary orchestration, scratch output and agent attribution out of
published contribution text.
