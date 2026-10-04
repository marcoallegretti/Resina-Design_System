# Slider value adjustment (candidate, 0.1.0)

Blueprint sections 85, 98, 99, 177 and 179 require operable, adaptable Slider
contracts. This operation changes a validated bounded value under explicit live
permission. It does not dispatch native events, manage focus/capture or publish
an accessibility tree.

## Ownership

Inputs are the current [bounded value IR](58-slider-value.md), current enabled
and readOnly booleans, and one explicit intent: setValue with an absolute number,
increase/decrease with a numeric amount, or minimum/maximum. Amounts use the
value's units, must be finite and strictly positive, and have no default. Absolute
values must be finite and within current bounds. Invalid parameters fail even
when unavailable; unavailable permission is not a validation shortcut.

The owner provides current value and permission at delivery, not cached action
availability. Disabled or read-only controls reject adjustment without changing
any current value/progress. The operation does not infer focus, source device,
locale, unit, direction, step admissibility, gesture lifecycle or repeat policy.
Discrete owners validate their allowed grid and choose explicit amounts before
calling this operation. A pointer gesture still needs its own complete capture,
cancellation, stable target and current-layout mapping contract.

## Resolution and commit

When enabled and writable, setValue uses the explicit absolute value;
minimum/maximum use exact endpoints. Increase/decrease move by the explicit
amount toward the corresponding bound, saturating at that bound when the amount
reaches or exceeds the remaining distance. This intentional action policy is
separate from spec58's rejection of invalid authored current values. Crossing a
bound does not wrap around. Numeric increase is independent of physical RTL or
vertical placement.

An interior adjustment whose arithmetic produces no change or falsely reaches
an endpoint fails diagnostically. Endpoint decisions compare the exact sum of the
represented current value and amount, rather than a rounded remaining distance.
Overflow in the direction of movement reaches the finite bound; cancellation
across extreme opposite signs is resolved from the actual candidate. Resolve the
candidate through spec58 before publishing the complete result. No partial value
or arbitrary epsilon is used.

The result retains the complete next bounded value IR and explicit accepted and
changed flags. Accepted is exactly enabled and not readOnly for valid parameters;
changed is exact numeric inequality with the current value. A valid invocation
at an endpoint, or setting the existing value, can be accepted without a change.
A rejected invocation is unchanged. Owners commit the complete next value before
notifying product code once when changed; this pure operation invokes no callback.
External current-value changes are respected on every delivery.

## Standards and evidence

The [W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) specifies
increment/decrement and endpoint keyboard intents. [WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria-1.2/#slider)
includes read-only Slider state. These support explicit semantic value intents and
permission ownership; this contract does not prescribe key mappings, toolkit
callbacks or native constants.

The [public vectors](../conformance/interaction/slider-adjustment-cases.json)
contain complete expected results computed from exact rational arithmetic on the
represented numbers. The [case schema](../schemas/slider-adjustment-cases.schema.json)
validates typed-operation test records; it does not define a production request
protocol. The [result schema](../schemas/slider-adjustment-ir.schema.json) requires
complete next value and flags, and rejects a changed result when not accepted.
Relational numeric invariants still require resolution, as in spec58.

The Rust reference exposes resolve_slider_adjustment over typed validated input.
Tests cover all intents, bound saturation, accepted no-ops, external value changes,
live disabled/read-only delivery, malformed parameters while unavailable, both
arithmetic directions, extreme opposite signs and propagation of spec58 guards.
Boundary regression tests exercise both directions when subtracting the current
value from the endpoint would round the remaining distance down. The reference
uses magnitude-ordered FastTwoSum only for finite endpoint candidates, following
[Algorithm 2.5, Rump, Ogita and Oishi](https://www.tuhh.de/ti3/paper/rump/RuOgOi07I.pdf).
No standalone source/command protocol is provided for this operation. A future
adapter must preserve source precision and validate its complete input boundary;
the typed operation does not certify a permissive JSON adapter.

Full semantic snapshots, discrete steps, key/controller adapters, pointer/touch
lifecycle, visual/material feedback and native accessibility remain component
requirements. No Slider UI is published by this operation.
