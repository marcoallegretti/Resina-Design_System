# Contributing to Resina

Start with [AGENTS.md](AGENTS.md), the canonical engineering contract for manual and AI-assisted contributions. This guide helps you find the relevant workflow and evidence; the contract defines the requirements.

The [contributor toolkit](.agents/README.md) provides reusable workflows, domain
skills, review roles and portable verification commands. Its instructions work
with manual development, Codex, Claude Code, Cursor, Copilot and other assistants
by loading the same files. See its setup table for client discovery differences.

## Describe a verifiable change

Search [existing issues](https://github.com/marcoallegretti/Resina-Design_System/issues) before opening a [new issue](https://github.com/marcoallegretti/Resina-Design_System/issues/new/choose). Choose **Defect or contribution** for an ordinary change, **Maintenance** for a present simplicity or structural debt problem, or **Architectural proposal** for a decision about public semantics, ownership or a shared mechanism. The forms ask for observed and expected behavior, evidence, affected boundary, present impact, acceptance criteria and expected proof.

Use the [issue threshold](AGENTS.md#11-issue-threshold) to distinguish a concrete current problem from a cleanup preference. Architectural changes require a [maintainer decision before implementation](AGENTS.md#9-architectural-decision-boundary). Opening an issue does not itself approve a proposed architecture.

## Find the owning layer

The [blueprint](RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes intended direction. Implemented normative behavior lives in [spec/](spec/), [definitions/](definitions/), [schemas/](schemas/), [tokens/](tokens/) and [conformance/](conformance/). [reference/rust/](reference/rust/) demonstrates those contracts; [backends/](backends/) realizes them. Follow the [sources of truth](AGENTS.md#2-sources-of-truth) and [architectural boundaries](AGENTS.md#4-architectural-boundaries) when deciding where a change belongs.

## Implement and prove the change

Follow the [contributor workflow](AGENTS.md#23-contributor-contract): issue or explicit accepted scope, isolated branch/worktree, proof, implementation, validation, review and pull request. Maintenance starts from current `master` in an isolated worktree using a concrete `maintenance/<change-description>` branch; see [maintenance isolation](AGENTS.md#10-maintenance-isolation). Preserve the maintainer's active checkout and unrelated work.

Identify the [proof surface](AGENTS.md#7-proof-before-implementation) before or alongside implementation. The required baseline is:

```sh
cargo test --workspace --locked
```

Run applicable formatting and lint checks from [the CI workflow](.github/workflows/reference.yml), including:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
```

For schema changes, also run:

```sh
python -m pip install -r tools/requirements-schema.txt
python tools/check_schemas.py
```

For public command boundaries and backend realization, run the applicable external checker in [tools/](tools/); direct Rust unit tests do not replace protocol evidence. The [README](README.md) documents commands and checkers. Rendering and visual changes also need the evidence described in the relevant backend README, [shared scenes](conformance/scenes/README.md), [raster evidence](conformance/raster/README.md) and CI workflow. Select checks according to [validation scope](AGENTS.md#19-validation).

Before running the contributor toolkit checks or the complete Python test suite,
install `tools/requirements-toolkit.txt` as shown in the
[verification instructions](.agents/README.md#verification-and-optional-hooks).

## Submit a reviewable pull request

Follow the [commit rules](AGENTS.md#21-git-rules), [independent maintenance review requirement](AGENTS.md#15-independent-review) and [PR requirements](AGENTS.md#16-pull-requests). The PR template asks for the problem, linked scope, resulting change, ownership, specific proof, actual validation, manual verification and material limitations. Explain changed expected output rather than merely accepting new snapshots.

Keep published text about the project and concrete evidence. Record separately actionable remaining problems as issues when they meet the threshold. Maintainers decide whether to merge maintenance PRs and whether to advance a release stage.
