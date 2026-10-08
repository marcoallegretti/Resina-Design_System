---
name: resina-reference
description: Use for deterministic Rust reference models, tokens, environment, color, motion and resolver behavior.
---

# Rust reference implementation

Read [AGENTS.md](../../../AGENTS.md), the affected normative specification and
the relevant crate in [reference/rust](../../../reference/rust/). Inspect its
manifest, callers, tests and applicable external checker before editing APIs.

The reference crates separate model, tokens, environment, color, motion and
resolution. Trace current ownership rather than assuming every numerical or
semantic operation belongs in the resolver. Keep rendering, native clocks, input
routing and product state outside the portable reference boundary.

Preserve deterministic inputs and results, finite/representable arithmetic,
diagnostic failures and explicitly authored state. Do not hide unsupported inputs
behind defaults. For capability or preference changes, include lower-capability,
reduced-motion/transparency and actual environment cases where relevant.

Establish a regression case or independent vector. Check public command behavior
as a separate boundary: stdin/file handling, strict parsing, acceptance/rejection,
diagnostics and result structure must match the contract. Use the independent
checker rather than treating unit-test success as protocol proof.

Run `python .agents/scripts/verify.py baseline` and applicable checkers. Inspect
[CI](../../../.github/workflows/reference.yml) for release-mode numerical cases
when guards or arithmetic change. Root workspace checks do not cover the separate
native backend/Lab workspaces; load [backends](../resina-backends/SKILL.md) if needed.
