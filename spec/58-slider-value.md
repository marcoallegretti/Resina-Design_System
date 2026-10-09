# Bounded Slider values (candidate, 0.1.0)

Blueprint sections 98, 99, 177 and 179 require Slider and RangeSlider controls.
Their numeric values must be validated before layout, interaction or accessibility
publication. This contract resolves one bounded scalar value. Each RangeSlider
thumb can use this value contract, but ordering, separation and joint ownership
require a separate component contract.

## Ownership and validation

The [request](../schemas/slider-value-request.schema.json) requires `schemaVersion`
`0.1.0` and explicit numeric minimum, maximum and value. All three must be finite;
minimum must be strictly less than maximum; value must lie in the inclusive
interval. Unknown, missing, duplicate or wrongly typed members and unsupported
versions fail. The request MUST be an object with these named members; positional
arrays MUST NOT be interpreted as value records. Member order is immaterial,
and duplicate decoded names fail even when written with different JSON escapes.
The Rust reference reads floating JSON numbers as binary64 and
rejects integer-form numbers that would lose significant bits during conversion,
including literals larger than 64 bits. The duplicate-safe source gate precedes
raw-literal precision checks; no rounded value is published.
Representable integers above the usual interoperable JSON integer range remain
valid; no blanket magnitude cutoff is imposed. [RFC 8259 section 6](https://www.rfc-editor.org/rfc/rfc8259#section-6)
describes binary64 interoperability and exact integer agreement. Invalid current
values are never clamped, rounded, replaced with an endpoint or given a default.
A collapsed interval has no meaningful normalized position and fails rather than
pretending to be an adjustable control.

This validation does not establish discrete step admissibility. A component's
explicit step policy must separately validate permitted values before publishing
a complete control. This operation does not infer a step, orientation, RTL
mapping, input capability, availability, focus, localized display or unit.

## Resolution

Normalized progress is (value - minimum) / (maximum - minimum). Exact minimum and
maximum resolve to 0 and 1 respectively. Every accepted interior value resolves to
finite progress strictly between those endpoints. The result retains the complete
validated minimum, maximum and value unchanged alongside progress. Increasing
numeric progress does not prescribe increasing physical x or y; placement owns
orientation and direction. No renderer or toolkit concepts enter the result.

An implementation must detect arithmetic loss that makes an interior value
indistinguishable from an endpoint. It returns a diagnostic and no partial result;
it must not hide that loss with clamping or an arbitrary epsilon. The Rust
reference uses binary64, distance from the nearer endpoint and scaled differences
when the interval span overflows. An otherwise finite bounded input whose
progress underflows or rounds to an endpoint returns `NumericRange`. This is a
reference arithmetic limit, not a restriction on the mathematical range format.

The [IR schema](../schemas/slider-value-ir.schema.json) checks structural numeric
fields and progress bounds. Relational range/value constraints and endpoint versus
interior agreement require runtime validation and conformance; Draft 2020-12 does
not compare arbitrary sibling numbers. The public numeric comparison allows four
binary64 ULPs for interior progress, exact endpoints and exact retained input
values; this tolerance never accepts an interior value mapped to an endpoint.

## Standards and component obligations

The [W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) requires
bounded current values, labeling and keyboard changes, with optional readable
value text. [WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria-1.2/#slider) defines its
semantic role and value properties. Those are evidence for separating numeric
value from localized description, native action delivery and layout; they do not
make this numeric result a complete accessible Slider.

[HTML range controls](https://html.spec.whatwg.org/multipage/input.html#range-state-(type=range))
sanitize missing and invalid values and have default bounds/step. Resina's strict
authoring operation deliberately fails malformed requests, consistent with its
existing source contracts. A Web backend must preserve validated authoring data
before native-control sanitization; HTML defaults must not define portable IR.

Discrete steps, key/controller changes, pointer/touch gestures, cancellation,
stable hit targets, focus, localized labeling/value text, complete material paint,
reduced-motion behavior and native accessibility remain component obligations.
This operation publishes no UI and certifies none of those paths.

## Evidence and protocol

[Public cases](../conformance/interaction/slider-value-cases.json) cover exact
endpoints, negative and fractional intervals, opposite extreme bounds, adjacent
binary64 bounds, near-endpoint progress, subnormal intervals/progress and diagnostic
failures. Expected progress is independently derived from exact rational arithmetic
on the represented numeric inputs. Rust tests check typed/source equivalence,
monotonicity across positive scaling/translation, strict CLI failure and absence
of partial output. The [external checker](../tools/check_slider_value_backend.py)
verifies the rational oracle, retained values, endpoint distinction, schema shape,
deterministic output and duplicate/nonfinite source rejection on Linux and Windows.

`resina-slider-value <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits 0
with complete JSON and no diagnostic; invalid input exits 1 with a diagnostic and
no output; usage exits 2. The size guard belongs to the reference executable.
