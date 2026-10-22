# Disclosure command profile (candidate, schema 0.1.0)

Blueprint sections 85, 98 and 99 require complete component semantics and
accessibility relationships. A disclosure is a Command profile that shows or
hides one controlled content node. It reuses the [Command activation
lifecycle](42-command-activation.md), content, anatomy and focus policy. It is
not a separate component, Toggle value, popup contract or content container.

## Inputs and ownership

The strict [request](../schemas/disclosure-request.schema.json) requires schema
version `0.1.0`, the complete accessible `name`, optional `description`, current
activation state, explicit `focusable` and `expanded` values, one
`controlledContent` record and one Command activation event. Every member is
required. Unknown or duplicate members, positional records and unsupported
versions fail.

The name, description, activation state and focusability follow the ordinary
[Command accessibility](47-command-accessibility.md) rules. The name MUST be the
complete resolved Command name. A supplied description MUST be nonblank.
Enabled disclosures MUST be focusable, and actual focus requires focusability.
These invariants apply to both the supplied state and the state produced by the
event. Enabling or focusing an explicitly non-focusable disclosure therefore
fails instead of publishing an incoherent snapshot.

`controlledContent.identity` is a stable, nonempty opaque identity owned by the
producer. It is compared exactly, is not trimmed or normalized (a
whitespace-only identity is nonempty and accepted) and MUST NOT be derived from
the label, a renderer object or a platform handle. `present` MUST be true:
before resolution, the producer confirms that this identity resolves to the current semantic
content node even when that node is visually hidden. Missing, empty and stale
targets fail rather than publishing a broken relationship. The presence bit is
evidence supplied at this public boundary; headless resolution cannot inspect a
native accessibility tree or prove that a backend later publishes the target.
Staleness is therefore not independently detectable by a headless
implementation: `present` is the producer's assertion, the schema fixes it to
`true`, and a `false` value is rejected only because the producer declared it.

## Transition

The resolver first validates the complete disclosure request, then applies the
event through spec 42 without changing its timing, capture, cancellation,
repeat or supersession rules. When the nested activation result has `activate:
true`, the next expanded value is the Boolean inverse of the input. Otherwise it
is unchanged. Disabled, cancelled, interrupted, repeated, competing and orphaned
operations therefore cannot change expansion.

Expansion and collapse do not request, clear or redirect focus. The nested
activation state is authoritative. In particular, an Enter activation on a
focused expanded disclosure collapses it while keeping actual focus on the
Command. This profile does not move focus into controlled content on expansion.
A larger popup, dialog or navigation contract owns any such movement.

Focus inside the controlled content is outside this profile. That content, or a
larger contract that hosts it, owns that focus; this profile neither moves nor
reports it. The hosting contract that owns the controlled content is
responsible for resolving focus inside content that becomes hidden; this profile
supplies no input for it and states no conformance rule about it. Not
redirecting focus does not forbid that contract from restoring focus to the
Command. Moving focus out of collapsed content is an explicit non-goal of this
profile.

Commit the complete next state before changing content visibility or delivering
the activation side effect. The committed state is the producer-owned semantic
visibility state, the value published in `expanded`. One activation changes visibility once. Do
not feed both raw events and a synthesized invocation for the same gesture.
Content removal or replacement requires a new coherent target record before
another snapshot is published.

## Output

The [result](../schemas/disclosure-result.schema.json) contains the complete
Command activation result, next `expanded` value and a coherent
[accessibility snapshot](../schemas/disclosure-accessibility-ir.schema.json):

| Dimension | Disclosure semantics |
| --- | --- |
| role | `button` |
| name | Complete supplied Command name |
| description | Supplied complete description or null |
| value | null |
| state | Next enabled, actual focused and expanded values |
| actions | One `invoke` action whose availability equals enabled |
| focusability | Explicit focusable value |
| relationships | Exactly one `controls` relationship to the supplied identity |

The relationship remains present in collapsed and expanded snapshots. The
expanded state describes the committed semantic visibility state, not an
animation target; presentation may lag it while content animates.
Momentary press remains interaction feedback and is not exposed as an
accessibility state.

A backend that cannot publish both expanded state and the controls relationship
MUST report the unsupported mapping. It MUST NOT silently expose the profile as
an ordinary relationship-free Command. Static headless output proves only the
portable contract. Native tree inspection, assistive-technology delivery and
screen-reader speech are separate backend evidence.

## Conformance

`resina-disclosure <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits 0
with complete JSON; invalid input exits 1 with a diagnostic and no result; usage
errors exit 2. [Public cases](../conformance/interaction/disclosure-cases.json)
cover pointer, Space, Enter and semantic invocation; collapsed and expanded
states; disabled and cancelled operations; focus retention; localized content;
and missing, empty or stale targets. The external checker independently applies
the transition and snapshot rules rather than importing the Rust reference.

This contract follows the
[WAI-ARIA disclosure pattern](https://www.w3.org/WAI/ARIA/apg/patterns/disclosure/),
which uses a button, Space/Enter activation and an expanded state, and the
[WAI-ARIA controls relationship](https://www.w3.org/TR/wai-aria-1.2/#aria-controls).
Those Web standards inform the portable semantics; native backends map the same
meaning through their actual accessibility APIs.

Failure diagnostics follow the [public backend diagnostic
policy](32-headless-conformance.md#failure-diagnostics). Negative public cases
use `errorContains` only to prove the intended validation path; exact diagnostic
wording is not a conformance requirement.
