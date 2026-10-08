---
name: harness
description: Map changed contracts to proof and run the applicable repository checks.
---

# Select and run the evidence

Input: accepted scope and a diff or explicit base/head range. Read the
[proof model](../../AGENTS.md#7-proof-before-implementation),
[validation rules](../../AGENTS.md#19-validation) and
[CI workflow](../../.github/workflows/reference.yml).

Identify observable changes rather than counting changed lines. For each one,
name the test/vector/checker that would fail if it were wrong. Distinguish direct
proof, incidental coverage and missing coverage. A passing compilation does not
prove a public protocol, pixels, native routing or assistive technology delivery.

| Affected surface | Checks and evidence to select |
| --- | --- |
| Workspace Rust | `python .agents/scripts/verify.py baseline` |
| Schemas, definitions, tokens, vectors | `python .agents/scripts/verify.py schemas` and relevant positive/negative cases |
| Public command or backend protocol | Applicable `tools/check_*_backend.py` against the actual executable; find the pairing in the root/backend README and CI |
| Numerical guards | Relevant debug and release-mode tests from CI; boundary, representability and diagnostic cases |
| CPU raster | Workspace baseline, relevant raster checkers and CI's no-default-features tests/Clippy |
| GUIdo | Separate-manifest formatting, tests with `testing`, Clippy, prepared scenes, explicit test font and required GPU readback from its README/CI |
| Native Material Board | Separate `lab/guido` manifest checks with prepared scenes, font and required native PNG/GPU evidence |
| Slint runtime | Separate-manifest checks including no-default-features and `testing`; applicable native measurement/runtime evidence |
| Quickshell or Slint generated integration | Protocol checker plus applicable runtime probes; emitted source alone does not prove native delivery |
| Visual behavior | Shared scenarios, actual image inspection, relevant state/device/text scales, accessibility and capability fallback |
| Contributor toolkit | `python .agents/scripts/verify.py toolkit` and manual consistency with `AGENTS.md` |

Install declared Python dependencies before the checks that need them. Backend
and rendering requirements are in the relevant READMEs linked from
[resina-backends](../skills/resina-backends/SKILL.md). Several native crates are
outside the root workspace: workspace success cannot stand in for their gates.

Run selected checks and record actual exit results. For a missing dependency,
font, native runtime or GPU, say which evidence is unavailable; never count a skip
as proof. Choose expensive checks according to scope. Release promotion requires
the complete release validation and a maintainer decision.

Return a compact coverage table, actual commands/results, remaining proof gaps
and the most valuable missing test if one exists. Do not mutate production code
to probe coverage unless that experiment is explicitly authorized and isolated.
