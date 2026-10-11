# Delivery tracking

[AGENTS.md](../../AGENTS.md) defines engineering policy. The [blueprint](../../RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes direction; component requirements belong to the normative sources listed in the engineering contract.

The first milestone targets a minimal real Wayland shell using Resina through GUIdo. A separate later milestone covers Canopia adoption. Start from [Epic 1](https://github.com/marcoallegretti/Resina-Design_System/issues/108) and the [shell testbed epic](https://github.com/marcoallegretti/Resina-Design_System/issues/110). An epic issue records a user outcome, normative requirements, scope limits, dependencies and acceptance evidence. Child issues define executable work. Pull requests link to their accepted issue and name any prerequisite PR. Issues and the milestone track delivery status. An optional Project may provide another view; it does not define component behavior.

## Status and acceptance

Backlog contains work awaiting scope or prerequisites. Ready means a maintainer accepted the issue's scope and its ordinary implementation may proceed under AGENTS.md. In progress means implementation is active. In review means a concrete PR or acceptance result is ready for human review. Blocked names the actual dependency, owner and work that can still proceed. Done means the issue's evidence-backed acceptance criteria are satisfied.

Architectural decisions still require maintainer acceptance before implementation. Ordinary implementation can proceed autonomously within accepted scope. If every remaining requirement depends on an external repair, report the residual acceptance ledger and stop the run; keep the affected issue open and Blocked.

A task covering several PRs must list its remaining work. Use closing keywords only on the final PR that actually satisfies the task. Epic parents close after a maintainer accepts the integrated outcome. A green check, merged probe or closed child issue cannot substitute for native runtime qualification.

## Evidence and scope

Each acceptance row records its requirement, implementation paths, commit, executed commands or manual procedure, artifacts, environment and unresolved blocker. Distinguish in-process injection, compositor-delivered events and physical input. Separate diagnostic accessibility reports from strict acceptance checks.

Keep GUIdo, standalone Qt and released Quickshell qualification separate. Advertise only the capabilities demonstrated on the named runtime. Retest the complete stacked candidate after integration when acceptance depends on interactions between changes.

Keep detailed requirements and the live acceptance ledger in the issue. Document durable contributor guidance in the repository and reproducible validation alongside the owning implementation. Avoid copying mutable issue status into repository documentation.

## Human review and integration

Follow the review queue limit in [AGENTS.md](../../AGENTS.md#21-git-rules): at most three implementation PRs ready for review unless the maintainer explicitly changes it. State each PR's base and blocking dependency. Preserve unrelated work and keep changes atomic and revertible.

Development uses branches and PRs under the contribution rules in AGENTS.md. GitHub branch protection must be configured and verified separately; repository documentation does not enforce it. Maintainers decide merges, epic acceptance and release-stage progression. Shell milestone acceptance does not itself authorize release publication.
