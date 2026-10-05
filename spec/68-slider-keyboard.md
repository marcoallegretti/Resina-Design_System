# Slider keyboard adjustment (candidate, 0.1.0)

Blueprint sections 85, 98, 99 and 177 require keyboard-operable Slider controls.
This contract composes checked [values](58-slider-value.md),
[adjustments](59-slider-adjustment.md), [stops](66-slider-stops.md) and
[value policy](67-slider-value-policy.md). It resolves an already selected logical
key against live state, without native key codes, event routing or callbacks.

## Checked configuration

Supply a checked policy with explicit increaseOnRight and increaseOnUp booleans,
a step mode matching the value policy, and an explicit page field:

| Mode | Step | Page |
| --- | --- | --- |
| continuous | Finite strictly positive amount in value units. | Explicit null, or a finite amount strictly greater than step. |
| stops | Explicit count of exactly one adjacent allowed index. | Explicit null, or an integer count greater than one. |

Stopped arrows must reach every declared stop; larger basic index jumps fail.
Larger movement belongs to optional Page keys. Counts saturate through spec66,
including unsigned 64-bit maximum counts in the Rust reference. No numeric step,
percentage, Page action or arrow direction is guessed. Construction validates
the whole configuration, including Page data unused by an endpoint key.

Each delivery supplies current checked value, complete value policy, checked
keyboard policy, logical key and actual enabled/readOnly/focused flags. Step
mode mismatch is diagnostic. Stopped bounds/membership are checked before
permission and unsupported-Page handling. Invalid current data is never snapped
into a domain or ignored because focus was lost.

## Key mapping

| Key | Intent |
| --- | --- |
| Right / Left | Increase / decrease when increaseOnRight is true; reverse when false. |
| Up / Down | Increase / decrease when increaseOnUp is true; reverse when false. |
| Home / End | Numeric minimum / maximum, the first / last allowed stop. |
| Page Up / Page Down | Numeric increase / decrease by the configured larger amount or count. |

Both arrow pairs are available regardless of visual orientation. Numeric
endpoints and Page direction are independent of physical minimum placement and
text direction. Owners choose and communicate arrow policy; resolution does not
detect a backend, locale or layout to guess it. The
[W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/)
defines arrow/endpoint intents, optional larger Page changes and circumstances
where reversed arrows are intuitive. It supplies no universal numeric amount.
Page null means unsupported; it does not fall back to arrow movement.

## Resolution and publication

Supported keys use spec59/66 under enabled AND actually focused AND NOT readOnly
permission. Valid resolution returns complete current/next value, explicit commit
(accepted adjustment or null), version and outcome:

- adjusted: one accepted intent, including an endpoint no-op;
- unavailable: supported key, unchanged value and no intent;
- unsupported: unconfigured Page key, unchanged value and no intent.

Errors produce no result and retain numeric/domain causes. Unsupported follows
domain coherence, independently of permission. Native propagation/consumption
belongs to the adapter's actual binding/routing contract, not enum membership.
Owners commit complete next state before notifying once when changed. Every
delivered repeated key-down uses the latest value and permission; do not replay
a cached result. Saturated or endpoint no-ops do not notify. Key-up does not
adjust. Resolution neither generates repeats nor controls their timing. Adapters
select owned bindings and preserve shortcut/modifier policy; arbitrary key events
must not be reclassified as Slider commands.

An accepted keyboard intent interrupts an open pointer edit through explicit
Abort, even for an accepted no-op. Changed value/revision followed by Refresh
also closes the old pointer through conflict before stale layout checks, but
unchanged data cannot replace Abort. Commit the keyboard value, update revision
for value/meaning changes, close ownership and rebuild layout, localized value
text and semantics as one controller transition before callbacks. The old Up
must not overwrite the keyboard choice. Pure resolution executes no effects.

Keyboard focus gating does not define native semantic action permission. Direct
assistive adjustment uses spec59/66 without inventing keyboard focus.

## Representation and evidence

[Policy](../schemas/slider-key-policy.schema.json) and
[result](../schemas/slider-key-ir.schema.json) schemas retain explicit configuration
and complete results; an accepted commit exists only for adjusted. Shape checks
cannot prove relational Page size, membership, actual focus or notification order.
[Public cases](../conformance/interaction/slider-key-cases.json) cover all keys,
independent arrow choices, focus/permission, unsupported Page, endpoints,
nonuniform stops, large counts, repeats and spec59 numeric regressions. Their
[schema](../schemas/slider-key-cases.schema.json) defines test records, not a
production source protocol. Typed tests check nonfinite configuration, explicit
Page presence, fractional stop replay, current replacement and nested diagnostics.
Integration tests close pending/acquired pointer edits after changed values and
explicitly abort accepted endpoint no-ops.

Native keyboard routing, focus navigation, localized formatting, assistive
delivery, styled rendering and visual certification remain complete control
obligations in every tier.
