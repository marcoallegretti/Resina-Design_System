# Resina contributor toolkit

Reusable workflows, skills, review roles and checks for manual and AI-assisted
contributions. Start with [the engineering contract](../AGENTS.md) and
[CONTRIBUTING.md](../CONTRIBUTING.md). The contract remains authoritative; these
files help apply it and do not grant permissions or approve architectural changes.

## Start a contribution

1. Read `AGENTS.md`, identify the accepted scope and choose the owning layer.
2. Load only the relevant skills from the table below.
3. Follow [spec](commands/spec.md) for an unverified report or
   [implement](commands/implement.md) for accepted, verifiable work.
4. Use [harness](commands/harness.md) to select evidence, run the checks and
   [reviewer](agents/reviewer.md) to inspect the resulting change.

Example prompt, usable in any assistant:

```text
Read AGENTS.md and .agents/README.md. Follow .agents/commands/implement.md
for this accepted scope: <issue URL or concrete task and acceptance criterion>.
Load the relevant skills. Report the actual evidence and validation results.
```

## Skills

Each skill has a standard `SKILL.md` with `name` and `description` metadata.
The prefix distinguishes these skills from unrelated libraries or installed skills.

| Skill | Load when working on |
| --- | --- |
| [resina-contracts](skills/resina-contracts/SKILL.md) | specifications, definitions, schemas, public semantics or IR |
| [resina-reference](skills/resina-reference/SKILL.md) | deterministic Rust models, tokens, environment, color, motion or resolution |
| [resina-conformance](skills/resina-conformance/SKILL.md) | vectors, strict public protocols, external checkers or proof gaps |
| [resina-backends](skills/resina-backends/SKILL.md) | CPU, GUIdo, Slint, Quickshell or Web realization |
| [resina-visual-verification](skills/resina-visual-verification/SKILL.md) | material appearance, text, motion, raster/GPU evidence or Lab captures |
| [resina-documentation](skills/resina-documentation/SKILL.md) | public documentation, examples or the contributor toolkit |

## Workflows and roles

Commands are portable task instructions, not executable slash commands. Supply
their inputs in your prompt or use them as a manual checklist.

| Workflow | Input | Result |
| --- | --- | --- |
| [spec](commands/spec.md) | report or proposal | evidence-backed issue draft and acceptance criterion |
| [implement](commands/implement.md) | issue or explicit accepted scope | isolated, verified contribution ready for a PR |
| [harness](commands/harness.md) | scope or diff | proof coverage and actual check results |
| [maintenance](commands/maintenance.md) | one named maintenance lane | one justified issue or one isolated change, according to the lane |

Roles are tool-independent instructions. Use them in a separate reviewer session,
an agent that supports explicit file loading, or a human review:

- [Reviewer](agents/reviewer.md): inspect a specified diff without editing it.
- [Architecture Debt Scout](agents/architecture-debt-scout.md): establish present
  architectural debt without implementing it.
- [Continuous Simplicity](agents/continuous-simplicity.md): remove one demonstrated
  source of accidental complexity.
- [Continuous Refactoring](agents/continuous-refactoring.md): resolve one concrete
  structural problem without changing semantics or ownership.

No role runs, schedules itself, creates issues or publishes a PR just because its
file exists. Those actions require an invoked task and the contract's conditions.

## Using different tools

| Contributor tool | How to use the toolkit |
| --- | --- |
| Codex | Read root `AGENTS.md`; repository skills live in `.agents/skills/`. Ask explicitly to read a skill if the client does not discover it. Load command and role files by path. |
| Claude Code | Point a local `CLAUDE.md` to `AGENTS.md` and `.agents/README.md`. Load the shared files by path, or configure local command/skill adapters for your client version. |
| Cursor, Copilot or another assistant | Add a local instruction that reads `AGENTS.md` and `.agents/README.md`, then provide the relevant skill, workflow and role paths. |
| Manual contribution | Follow the same Markdown checklists and run the same verification commands. |

Automatic discovery of commands, agents and hooks varies by client. The portable
path-based workflow requires no plugin, paid service, subagent feature or provider
configuration. Keep optional client adapters local; do not copy the contract or
maintain separate versions of skills. Existing `.gitignore` entries keep local
Claude/Codex configuration out of commits.

## Verification and optional hooks

From the repository root, with Python 3.12+ and the Rust toolchain installed:

```sh
python .agents/scripts/verify.py toolkit
python .agents/scripts/verify.py baseline
python -m pip install -r tools/requirements-schema.txt
python .agents/scripts/verify.py schemas
python .agents/scripts/verify.py baseline schemas --dry-run
```

The runner works from any directory, invokes argument arrays without a shell,
stops on failure, and never formats files, installs packages, commits or updates
expected output. `toolkit` checks links/metadata and tests the runner; `baseline`
runs workspace formatting, tests and Clippy; `schemas` invokes the existing schema
checker. Select additional protocol, release-mode and backend gates with
[harness](commands/harness.md). These profiles do not replace the complete
[CI workflow](../.github/workflows/reference.yml).

An optional local stop or pre-commit hook can invoke the runner using an absolute
path, for example `python /path/to/Resina-Design_System/.agents/scripts/verify.py
baseline`. Configure when it runs in your client or Git setup. No hooks are
installed automatically. A check runner is not a command sandbox: use your
client's actual permission controls and inspect commands and diffs. Do not treat
a successful hook as approval to publish, merge or promote a release.

## Maintaining the toolkit

Add domain knowledge in a skill, repeatable task steps in a command, review
criteria in a role, and executable checks in scripts. Link to the existing
contract and source of truth instead of restating policy or freezing current
implementation status. Keep paths and commands verifiable. Toolkit checks run in
CI; new runner behavior needs a test showing what happens when execution fails.

This structure is informed by [GUIdo's contributor toolkit](https://github.com/MalpenZibo/guido/tree/main/.claude).
Resina's contract, layers and evidence requirements determine its contents.
