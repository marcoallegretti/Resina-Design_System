# Slider value policy (candidate, 0.1.0)

Blueprint sections 85, 98, 99 and 177 require Slider input to preserve the
control's admissible values. This contract composes [discrete stops](66-slider-stops.md)
with [cancellable edits](63-slider-edit.md) and [pointer lifecycle](65-slider-pointer-lifecycle.md).
It owns numeric admissibility, independently of toolkit, rendering or input device.

## Explicit policy and ownership

Every edit beginning and edit/pointer delivery supplies an explicit policy:

| Policy | Required state |
| --- | --- |
| continuous | Explicit kind; the checked bounded value defines the range. |
| stops | Complete checked stop domain and explicit lower/higher nearest tie rule. |

There is no default policy, inferred step, backend-specific mode or omitted tie
rule. Beginning a stopped edit requires matching current bounds and exact current
membership. An off-domain current value is an authoring error, not an instruction
to snap or clamp it. Idle pointer resolution validates this pair even when
disabled, read-only or reliable routing is unavailable, after basic event and
Down geometry checks. Owners can validate current-policy coherence directly
before publishing complete control state.

An open edit retains the complete policy alongside revision, baseline and preview.
Policy equality is semantic: equivalent separately checked domains compare equal.
Changing kind, bounds, allowed values or tie rule conflicts on preview or commit,
even if current value and revision remain equal. The owner must still change
revision whenever domain meaning changes, including an intervening change and
return to equal data. A held pointer conflict closes ownership and releases
pending/acquired routing according to spec65. It never commits or silently rebases.

## Preview and completion

Continuous preview keeps the complete spec62 numeric mapping result. Stopped
preview first obtains that same checked numeric candidate, then selects the
nearest allowed numeric stop through spec66 using the declared tie rule. Mapping
diagnostics remain errors; snapping must not conceal unsupported arithmetic.
Distance is in represented numeric value, not rendered pixel distance. Numeric
lower/higher does not change meaning with orientation, text direction or minimum
placement. Endpoint and stationary/no-op behavior follow the existing contracts.

Every accepted stopped preview is a complete allowed value. Rebuild layout,
localized text and accessibility semantics from that selected preview before the
next delivery. Retain the original pointer anchor across these frames; do not
replace it with a new anchor at each snapped thumb position.

Stopped completion applies exact allowed-value selection through spec66 under
live permission. Continuous completion follows spec59. No commit occurs on Down
or Move. Final Up may select a different allowed stop before the single commit.
The termination-only track path supports the same-point Down/Up alternative
without dragging, continuous routing, animation or effects. Permission loss closes
without an intent; accepted no-op remains distinct from a changed value.

## Cleanup and replacement state

Cancel, matching pointer cancellation/routing loss and explicit Abort retain their
existing cleanup priority. Policy/current/revision changes close the abandoned
edit before checking stale preview geometry or replacement-policy admissibility.
This permits cleanup when replacement controller data is itself incoherent.
The returned current numeric value in a closed result is not a certificate that
it belongs to the replacement domain. The owner must validate the replacement
pair before publishing a complete control, and must diagnose invalid data rather
than hiding it behind a default. Basic malformed event guards remain diagnostic.

## Portable representation and evidence

The [policy schema](../schemas/slider-value-policy.schema.json) requires explicit
kind and, for stops, the entire checked domain and tie rule. Sessions retain that
representation, with no native handles or backend tags. Schema shape validation
does not prove relational membership, revision history or domain meaning.

The Rust reference keeps checked domains immutable and shares their storage
across edit/pointer snapshots. This is an implementation choice, not part of IR
or an opaque cross-process reference. Serialization emits complete numeric values;
separately constructed equal domains remain interchangeable.

[Edit traces](../conformance/interaction/slider-edit-cases.json) and
[pointer traces](../conformance/interaction/slider-pointer-cases.json) compare
complete serialized results across both axes, both directions, both minimum
placements and tie rules. They include routing acquisition, release-only clicks,
policy conflict, cleanup and permission loss. Typed tests check invalid baselines,
semantic equality and nested diagnostics. Fixture policies use a
[test-record schema](../schemas/slider-value-policy-fixture.schema.json), not a
production source protocol or unchecked state constructor.

This contract does not establish complete keyboard policy, native pointer or
assistive routing, localized value formatting, styled rendering or visual
certification. Those complete control obligations remain required in every tier.

[Keyboard adjustment](68-slider-keyboard.md) composes explicit key policy with
the same value policy. Accepted keyboard intents must abort an open pointer edit,
including unchanged endpoint intents; value/revision equality is not an abort.
