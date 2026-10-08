# Resina Design System — Agent Engineering Contract

This file defines the repository-wide engineering contract for AI coding agents and AI-assisted contributors working on Resina.

It applies regardless of agent or model vendor.

`AGENTS.md` is the canonical agent policy for this repository. Agent-specific files may point here but must not duplicate, weaken, or reinterpret these rules.

Reusable contribution workflows, domain skills, review roles and portable checks
live in [`.agents/`](.agents/README.md). Load the relevant files for the task;
they support this contract rather than define a separate policy.

---

# 1. Project identity

Resina is an independent, material-responsive design-system specification.

The blueprint in this repository defines product direction and intended architecture. The public normative artifacts define implemented Resina behaviour.

The fundamental architectural law is:

```text
Specification
    !=
Reference implementation
    !=
Rendering backend
    !=
Platform integration
    !=
Product
```

Therefore:

```text
Resina != Rust
Resina != GUIdo
Resina != Quickshell
Resina != Slint
Resina != Web
Resina != Wayland
Resina != wgpu
Resina != Canopia
```

Rust is the canonical reference implementation.

Rust does not define Resina.

GUIdo is the high-fidelity reference rendering backend.

Quickshell is the independent Shell conformance backend.

Slint is the application backend.

Web is the documentation, conformance, and playground backend.

Canopia is a consumer of Resina, not part of Resina Core.

---

# 2. Sources of truth

For intended product direction, read:

```text
RESINA_DESIGN_SYSTEM_BLUEPRINT.md
```

For implemented normative behaviour, treat these as authoritative:

```text
spec/
definitions/
schemas/
tokens/
conformance/
```

The reference implementation exists to demonstrate and test those contracts.

Backends exist to realize them.

Generated output is never normative.

Do not infer normative behaviour solely from Rust implementation details.

Do not infer normative behaviour solely from a renderer.

A blueprint proposal is not implemented merely because code resembling it exists.

Where applicable, specification, definitions, schemas, tokens, conformance evidence, reference behaviour, and backend behaviour must agree before a feature is considered complete.

---

# 3. Evidence-first engineering

Engineering decisions must follow evidence.

Acceptable evidence includes repository state, Resina normative artifacts, tests, conformance vectors, stable upstream documentation, standards, dependency source code, benchmarks, reproducible experiments, and verified platform behaviour.

Never invent an API, capability, protocol behaviour, toolkit guarantee, rendering property, or platform constraint.

When an external assumption materially affects architecture or correctness, verify it before implementation.

Research priority is:

```text
Resina rules and normative artifacts
        ↓
existing repository implementation
        ↓
upstream source/documentation
        ↓
relevant standard or protocol
        ↓
comparable mature implementations
        ↓
measured experiment when necessary
```

Ordinary changes do not require performative research when the answer is already established by repository evidence.

When evidence contradicts an assumption in the blueprint, do not blindly implement the blueprint text.

Preserve its architectural intent, determine why the discrepancy exists, and choose the most maintainable evidence-backed solution.

Changing normative public artifacts is allowed only when the behaviour genuinely belongs to Resina and the change is supported by corresponding evidence.

---

# 4. Architectural boundaries

Resina IR must remain independent of renderers, UI toolkits, platforms, compositors, and products.

Never leak these concepts into normative Resina layers:

```text
GUIdo
Qt / QML
Quickshell
Slint
Wayland
wl_surface
layer-shell
wgpu
platform window handles
Canopia-specific concepts
backend identity
```

Renderer capabilities must be represented as capabilities.

Prefer:

```text
capability.backdrop_blur
```

over:

```text
backend == "guido"
```

Every advanced rendering behaviour requires a valid lower-capability fallback.

Tier 0 is not an excuse for generic fallback UI. It must remain unmistakably Resina.

Implementation order follows architectural dependency, not visual excitement.

The normal dependency direction is:

```text
specification intent
        ↓
model / definitions
        ↓
schemas and tokens
        ↓
validation
        ↓
environment and capability model
        ↓
deterministic resolution
        ↓
Resina IR
        ↓
conformance
        ↓
backend realization
        ↓
advanced rendering
```

Do not bypass missing lower layers by implementing renderer-specific approximations first.

---

# 5. Working modes

Resina has distinct working modes.

They must not be silently mixed.

## Primary Development Mode

Primary Development Mode is the maintainer-directed autonomous development workflow used to advance the blueprint.

A repository-wide Goal may drive multiple consecutive atomic changes.

At the beginning of a new repository-wide Goal:

1. read the complete blueprint;
2. inspect the repository structure;
3. inspect relevant source, manifests, tests, CI, schemas, definitions, tokens, dependencies, and current git state;
4. understand what is already implemented;
5. determine the next justified dependency boundary from repository evidence.

Do not repeat a full repository discovery before every atomic change within the same Goal.

For every subsequent task, inspect the specification, implementation, dependencies, tests, and regression surface relevant to that task.

Primary Development Mode may continue autonomously after each successful atomic commit while the active Goal remains incomplete.

Do not interrupt this flow merely because unrelated cleanup opportunities are discovered.

Record genuine independent maintenance debt through the maintenance rules below instead of hijacking the primary Goal.

Unless the maintainer explicitly invokes Primary Development Mode, external contributors and autonomous maintenance agents must use isolated Contribution or Maintenance Mode.

---

# 6. Core engineering loop

For every production change:

1. inspect the relevant specification and repository evidence;
2. identify the owning architectural layer;
3. identify what evidence will prove the change;
4. verify uncertain external assumptions;
5. choose the smallest complete production-quality change;
6. create or update the required proof surface;
7. implement the change completely;
8. run the relevant validation;
9. inspect the resulting diff for regressions, accidental scope growth, and architectural leakage;
10. commit atomically;
11. reassess the active Goal or issue from the new repository state.

Do not stop after planning when implementation and verification are possible.

Do not ask for confirmation for routine engineering decisions that can be safely derived from repository evidence.

Stop when progress requires unavailable information, an architectural decision reserved for the maintainer, an unsafe assumption, or completion of the active Goal.

---

# 7. Proof before implementation

Every change must identify what proves it.

The proof surface should exist before or alongside the implementation whenever practical.

For bug fixes this normally means reproducing the failure before correcting it.

For new specification work it may instead mean establishing a new conformance vector, schema case, scenario, invariant, or other independently evaluable contract.

The principle is:

```text
PROOF FIRST
IMPLEMENTATION SECOND
```

A change without an identifiable verification surface is not ready for implementation unless the limitation is intrinsic and explicitly documented in the pull request or maintainer-facing result.

Use the following evidence model:

| Change | Expected evidence |
| --- | --- |
| normative semantic behaviour | normative specification + conformance evidence |
| machine-readable public contract | schema + positive and negative cases |
| token behaviour | authored token source/vector + resolver evidence |
| validation rule | acceptance and rejection cases |
| resolver behaviour | reference test/vector + applicable external checker |
| environment/capability behaviour | deterministic scenarios covering relevant capability combinations |
| Resina IR | model/schema + deterministic conformance vectors |
| public headless protocol | request/result schema + external checker |
| material or appearance resolution | headless scenarios + expected resolved output |
| interaction semantics | deterministic state/operation vectors |
| accessibility semantics | headless semantic evidence + backend mapping evidence where applicable |
| fallback behaviour | explicit lower-capability scenario |
| backend realization | shared normative scenario + backend-specific verification |
| GUIdo visual/rendering work | deterministic Resina scenario + appropriate raster/GPU evidence |
| Slint or Quickshell realization | shared scenario + independent backend evidence |
| visual change | repeatable visual evidence at relevant scaling/state combinations |
| performance claim | repeatable before/after benchmark or measurement |
| documentation/API claim | verification against actual implementation where practical |
| blueprint/specification change | impact analysis across normative artifacts, reference implementation, conformance, and backends |

Do not update expected output merely to make a test green.

If an existing expected result changes, understand and justify why the contract itself changed.

---

# 8. Definition of done

Compilation alone is not completion.

A change is complete when its relevant architectural contract is coherent and evidenced.

No MVPs.

No stubs.

No placeholder implementations.

No fake production data.

No ghost UI.

No UI that promises functionality that does not exist.

No knowingly incomplete production paths.

No temporary architecture intended to be fixed later.

Do not hide unsupported states behind convenient defaults.

Fail explicitly and diagnostically.

Prefer minimal public APIs.

Prefer small composable abstractions.

Avoid dependencies that are not justified by concrete value.

Avoid speculative abstraction without demonstrated need.

Accessibility, localization, text scaling, reduced motion, reduced transparency, input variation, and capability fallback are architecture rather than polish.

---

# 9. Architectural decision boundary

Agents may make ordinary engineering decisions autonomously.

An ordinary change follows an already-established architectural pattern and does not redefine ownership or public semantics.

An architectural change includes, among other things:

```text
a new normative concept
a new cross-cutting mechanism
a new core abstraction or ownership model
a new public IR contract
a new capability-resolution rule
a new architectural dependency direction
moving responsibility between normative layers and implementations
changing how an entire family of public contracts behaves
introducing a framework-specific assumption into shared architecture
```

Architectural changes are decided before they are implemented.

When such a change becomes necessary:

```text
STOP implementation
        ↓
describe current problem
        ↓
provide repository evidence
        ↓
identify affected architectural boundaries
        ↓
research prior art where material
        ↓
describe viable alternatives and trade-offs
        ↓
request maintainer decision
```

Do not disguise architectural redesign as cleanup, refactoring, or implementation detail.

---

# 10. Maintenance isolation

Repository-wide maintenance must never modify the active Primary Development working tree.

The following are independent maintenance lanes:

```text
Architecture Debt Scout
Continuous Simplicity
Continuous Refactoring
```

Maintenance work is based on the current primary branch, currently `master`.

Use separate worktrees for implementation-bearing maintenance work.

One independently provable maintenance problem maps to:

```text
one issue
→ one worktree
→ one branch
→ one pull request
```

Never combine unrelated maintenance opportunities into one branch or PR.

Never switch the branch of the maintainer's active Primary Development checkout in order to perform maintenance.

Never mix maintenance commits into an unrelated development branch.

Maintenance branches describe the concrete change:

```text
maintenance/<change-description>
```

Use short lowercase words separated by hyphens, such as `maintenance/share-resolver-command-io`.

The name must remain understandable without the task conversation. Do not name branches after agents, models, internal working modes, or task numbers. Generic lane names such as `simplicity` or `refactor` followed by an issue number do not describe a change.

Link the issue in the pull request instead of using its number as the branch description.

Architectural debt discovery does not create an implementation branch until the architectural decision or implementation scope is actually approved.

Before creating a new maintenance issue, search for an existing issue describing the same underlying problem.

Update or reference the existing issue instead of creating duplicates.

---

# 11. Issue threshold

Maintenance discovery is not an issue-generation contest.

Do not create issues for stylistic preferences, hypothetical future needs, generic cleanup wishes, speculative abstractions, or observations with no present cost.

A maintenance issue is justified only when all of these are true:

```text
a concrete affected party or contract can be named
repository evidence demonstrates the problem
the problem matters in the current repository state
the problem has an independent acceptance criterion
```

The affected party may be a caller, contributor, normative contract, backend, test harness, renderer, user-visible result, or agent being materially misled by public project documentation.

If nobody is observably worse off today, the observation is not an issue.

If the problem belongs naturally inside the currently active change, fix it there.

If the problem is independently provable and belongs outside the active change, create or update an issue.

Every maintenance issue must state:

```text
Observed
Expected
Evidence
Affected layer or boundary
Why it matters now
Acceptance criterion
Expected proof
```

Architecture issues additionally state the meaningful alternatives and trade-offs discovered from evidence.

---

# 12. Architecture Debt Scout

Architecture Debt Scout is read-only with respect to source changes.

Its purpose is to find concrete architectural debt, not to manufacture redesign work.

It may inspect the entire repository and history necessary to establish evidence.

It must challenge boundaries such as:

```text
specification vs implementation
normative semantics vs backend realization
reference implementation vs normative source
headless contract vs renderer shortcut
shared IR vs toolkit-specific representation
capability resolution vs backend detection
duplicated semantic logic across layers
product behaviour leaking into Resina Core
implementation assumptions absent from normative artifacts
```

When it finds no issue meeting the issue threshold, it stops without creating work.

When it finds justified debt, it creates or updates the single highest-value independently actionable issue.

Architecture Debt Scout:

```text
does not edit production files
does not create implementation commits
does not open speculative PRs
does not make architectural decisions on behalf of the maintainer
```

Its normal lifecycle is:

```text
inspect master
→ establish evidence
→ apply issue threshold
→ create/update issue if justified
→ stop
```

---

# 13. Continuous Simplicity

Continuous Simplicity reduces accidental complexity without changing normative behaviour, public contracts, architectural ownership, or externally observable semantics.

Prefer:

```text
deletion over additional abstraction
existing concepts over new concepts
direct data flow over hidden coupling
one source of truth over duplicated logic
smaller explicit APIs over convenience surfaces
removing obsolete paths over preserving dead compatibility
```

Do not optimize for line count.

Do not collapse architecturally distinct layers merely because their current implementations look similar.

Do not replace explicit domain concepts with generic abstractions that erase meaning.

A Continuous Simplicity run handles one coherent opportunity.

Lifecycle:

```text
inspect current master
→ identify strongest evidence-backed simplification
→ apply issue threshold
→ create/update issue
→ create isolated worktree
→ create maintenance/<change-description>
→ establish proof
→ implement
→ validate
→ independent review
→ open PR
→ stop
```

If implementation reveals that the simplification actually requires an architectural decision, stop implementation and convert or update the issue accordingly.

Do not continue searching for additional simplifications after opening the PR.

---

# 14. Continuous Refactoring

Continuous Refactoring removes concrete structural debt while preserving normative behaviour and ownership.

Valid targets include demonstrated duplication, inappropriate coupling, confused responsibility, oversized modules with multiple reasons to change, unstable internal boundaries, repeated conversions, untestable structure, and backend/reference logic living in the wrong implementation layer.

Refactoring is not permission to redesign Resina.

The following must remain unchanged unless an architectural decision explicitly approves otherwise:

```text
normative semantics
public contracts
architectural ownership
capability semantics
IR meaning
conformance expectations
```

Lifecycle:

```text
inspect current master
→ identify strongest concrete structural debt
→ apply issue threshold
→ determine whether architectural decision is required
   ├─ yes → create/update architecture issue → stop
   └─ no
       ↓
     create/update maintenance issue
       ↓
     isolated worktree
       ↓
     maintenance/<change-description>
       ↓
     establish proof
       ↓
     refactor
       ↓
     validate
       ↓
     independent review
       ↓
     open PR
       ↓
     stop
```

One run produces at most one implementation PR.

Large scope is not itself a reason to split a refactor.

Independent acceptance criteria and architectural ownership determine the boundary.

---

# 15. Independent review

Maintenance implementation must be reviewed by a reasoning pass that did not author the change whenever the agent environment supports independent reviewers, subagents, or equivalent isolated review.

The reviewer examines the committed change rather than the implementer's description of it.

Review for:

```text
correctness
normative drift
architectural leakage
unnecessary abstraction
scope expansion
missing evidence
regressions
weakened diagnostics
incorrect fallback behaviour
backend-specific assumptions
test changes that merely bless the implementation
```

The pass condition is:

```text
zero blocking findings
```

Non-blocking notes do not need to be mechanically eliminated.

An architectural finding stops autonomous implementation and is escalated for maintainer decision.

If the environment cannot provide an independent agent, perform a fresh adversarial review pass separated from implementation before opening the PR.

Do not claim independence when the same reasoning context is simply asked to approve its own work.

---

# 16. Pull requests

External contributions and maintenance changes go through pull requests.

A maintenance PR should make it possible to evaluate the change without reconstructing the agent's internal reasoning.

Its description should contain:

```text
problem
issue
what changed
architectural ownership
what proves the change
validation performed
manual verification, if any
remaining limitations that materially matter
```

Public issues, pull requests, and comments must describe the project problem, the resulting change, and concrete verification evidence. Write for a future contributor who has no access to the task conversation.

Keep agent orchestration, model names, reviewer counts or personas, review-status narration, internal working modes, and temporary execution details out of published text. Independent review remains required by section 15; performing it does not authorize publishing how it was organized. Describe relevant defects and evidence rather than statements such as "four independent reviews" or "zero blocking findings".

Before an action that can trigger public automated review, check the repository's automatic-review configuration. Do not enable or request public AI reviews. If an existing integration would publish AI-branded comments and cannot be disabled or suppressed within the authorized scope, stop before triggering it and identify the setting that prevents publication under this policy.

Do not use the PR body as storage for important future work.

A separately actionable remaining problem that passes the issue threshold must exist as an issue.

Observations that do not pass the threshold remain observations or disappear.

Maintenance PRs are never automatically entitled to merge merely because CI is green.

The maintainer decides whether maintenance value outweighs integration risk.

---

# 17. Primary Development versus maintenance

Do not allow maintenance machinery to hijack the blueprint-development loop.

Primary Development may notice architectural debt while advancing Resina.

If that debt blocks the active Goal, resolve it as part of the active work if it belongs to the same acceptance criterion.

If it does not block the active Goal and satisfies the independent issue threshold, record it for a maintenance lane.

Then continue the primary Goal.

The intended relationship is:

```text
                    master
                      ▲
                      │
            Primary Development
                      │
          specification advancement
                      │
        atomic production-grade work
                      │
             continue active Goal

        ┌─────────────┼─────────────┐
        │             │             │
      Scout       Simplicity    Refactoring
        │             │             │
      issue        issue→PR      issue→PR
        │             │             │
        └──────────── review queue ─┘
```

Maintenance observes development.

It does not steer ordinary development unless the maintainer chooses to accept its finding.

---

# 18. Visual quality

Visual quality is a release criterion.

Any implemented UI or rendered surface must be critically evaluated for:

```text
hierarchy
spacing
typography
material coherence
interaction states
motion
scaling
accessibility
fallback quality
```

Reject flat, dated, generic, default-toolkit-looking presentation when it is intended to represent Resina.

Do not add decoration without semantic or material purpose.

Resina must remain sophisticated and identifiable when blur, transparency, deformation, or other expensive effects are unavailable.

Advanced rendering is enhancement, not identity.

---

# 19. Validation

Run validation appropriate to the affected scope before committing or opening a PR.

The repository-wide baseline includes:

```bash
cargo test --workspace --locked
```

Schema work must also use the repository schema validation:

```bash
python -m pip install -r tools/requirements-schema.txt
python tools/check_schemas.py
```

Public command boundaries and backend realizations must run their applicable `tools/check_*_backend.py` checker.

Do not substitute direct Rust unit tests for a public protocol checker when the external boundary itself is part of the contract.

Renderer/backend changes must run the relevant backend-specific evidence required by the repository and CI.

GUIdo rendering changes must preserve the repository's required deterministic rendering evidence rather than relying solely on headless semantic tests.

Do not blindly run expensive unrelated workloads when the changed scope cannot affect them.

Before a release-stage promotion, however, the complete required release validation must pass.

---

# 20. Release progression

Passing CI is necessary but not sufficient for release progression.

Resina's pre-release development advances through explicit stages such as:

```text
alpha
→ beta
→ rc1 / rc2 / ...
→ release
```

A version/stage promotion requires hostile review of the implementation and evidence for that milestone.

Agents may prepare a candidate and its evidence.

Agents do not self-declare a stage complete solely because tests pass.

A hostile review actively attempts to demonstrate that the candidate is not ready.

Blocking findings prevent promotion.

Release-stage progression remains a maintainer decision.

---

# 21. Git rules

Primary Development follows the maintainer's active repository workflow.

Maintenance and external contribution work must never commit directly to `master`.

All commits must be atomic, focused, reviewable, and independently revertible where reasonably possible.

Before every commit:

```text
inspect git status
inspect the complete staged/unstaged diff
verify scope
verify generated/transient files are absent
```

Never rewrite unrelated maintainer work.

Never mix unrelated changes in one commit.

No AI co-author trailers.

No AI attribution in commits, issues, pull request descriptions, or comments.

No generated-with footers.

No LLM scratch files.

No OCR artifacts.

No transient analysis documents.

No internal planning files.

No internal agent state.

No internal workflow junk.

Do not commit process documentation unless it is deliberately a public repository artifact.

Keep commit subjects short, specific, and non-verbose.

Use a body only when it materially improves understanding of the change.

Do not use internal AI workflow terminology in production commit messages.

---

# 22. Code style

Prefer self-explanatory names and structure.

Avoid narrative comments.

Comments are justified for non-obvious invariants, protocol constraints, safety reasoning, numerical assumptions, or edge cases that cannot be expressed clearly in code.

Remove dead code instead of commenting it out.

Keep formatting clean.

Keep lint clean.

Do not silence diagnostics without understanding the underlying cause.

Do not add dependencies for convenience when a small existing solution is sufficient.

Do not generalize before concrete reuse exists.

---

# 23. Contributor contract

A contributor using Codex, Claude, another coding agent, or manual development is expected to preserve the same architectural contract.

Agent choice does not change Resina's standards.

Contributors must not reinterpret implementation details as specification.

Contributors must not use backend convenience as justification for normative leakage.

Contributors must identify what proves their change.

Architectural proposals must be discussed and accepted before implementation.

External contribution work uses:

```text
issue or explicit accepted scope
→ isolated branch/worktree
→ proof
→ implementation
→ validation
→ review
→ pull request
```

The repository should remain understandable and maintainable even when the reader has no access to the conversations or prompts that produced a contribution.

If a design decision exists only in an AI conversation, it does not exist for the project.

Put durable decisions in the appropriate public Resina artifact.

Do not commit the conversation.

---

# 24. Objective

The objective is not to generate code.

The objective is not to keep agents busy.

The objective is not to maximize issue or pull-request count.

The objective is to make Resina progressively more:

```text
correct
normatively coherent
portable
reimplementable
visually distinctive
accessible
deterministic
testable
maintainable
evidence-backed
```

Every abstraction, specification change, implementation, test, backend realization, maintenance issue, and pull request should move the repository measurably toward that state.
