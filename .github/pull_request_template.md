## Problem and scope

<!-- Describe the concrete problem and its present impact. Link the issue (Closes #...) or explicit accepted scope. For an architectural change, link the maintainer decision that approved it before implementation. -->

## What changed

<!-- Describe the resulting behavior or structure for a reader who has no access to the task conversation. State relevant exclusions. -->

## Architectural ownership

<!-- Identify the owning layer and relevant normative artifacts, reference implementation, conformance and backends. Explain why public semantics and ownership are preserved, or how the approved contract change is made coherent across those surfaces. -->

## What proves it

<!-- Name the regression test, independently evaluable vector, schema acceptance/rejection cases, public protocol checker, deterministic rendering scenario or measurement. Passing the test suite alone does not explain what proves this change. For changed expected output, explain why the contract changed and attach relevant before/after evidence; do not re-bless output merely to make a test green. -->

## Validation performed

<!-- List actual commands and results, including cargo test --workspace --locked and applicable CI checks. Schema work also needs python tools/check_schemas.py with tools/requirements-schema.txt installed. Public command/backend contracts need their applicable tools/check_*_backend.py checker. Renderer changes need their backend evidence; GUIdo changes need deterministic raster/GPU evidence. Record checks not run and the concrete reason. -->

## Manual verification

<!-- Identify checks automation cannot establish, the environment/scenario, and observed results. For visual work cover relevant state/scale combinations, accessibility and capability fallback. If no manual check is needed, say why automated evidence covers the scope. -->

## Material limitations and related issues

<!-- State any limitation that affects evaluation or use. A separately actionable present problem needs an issue if it meets the issue threshold; link it here. A problem within this PR's acceptance criterion must be resolved here. Do not use this section to store future work or leave a production path knowingly incomplete. -->

<!-- Keep published text focused on the project problem, change and evidence. Do not include internal agent orchestration, review-status narration, model/persona names, attribution trailers or generated-with footers. Maintenance PRs remain subject to maintainer review and are not automatically entitled to merge. -->
