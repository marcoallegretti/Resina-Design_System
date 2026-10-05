# Discrete Slider stops (candidate, 0.1.0)

Blueprint sections 85, 98, 99 and 177 require an operable Slider contract.
[Bounded values](58-slider-value.md) and [adjustments](59-slider-adjustment.md)
alone do not establish discrete admissibility. This contract defines a finite
ordered set of permitted numeric values. It supports two distinct needs:
keyboard/semantic movement by stop index and nearest-stop selection from a
pointer's numeric candidate. No toolkit, device, renderer or product enum defines
the domain.

## Domain and ownership

Supply explicit finite minimum, maximum and an explicit list of numeric stops.
Minimum must be strictly less than maximum. At least two stops are required;
every stop is finite, within bounds and strictly greater than its predecessor.
The first and last stops must equal the exact minimum and maximum. Duplicates,
including numerically equal negative and positive zero, fail. Missing endpoints,
unordered data and invalid values are never repaired by sorting, deduplication,
clamping or inserting guessed stops.

The domain retains each declared value unchanged, together with its complete
spec58 bounded-value IR. Each interior value must resolve to progress strictly
between the endpoints; a reference arithmetic limit fails the entire domain,
identifying the failing stop index and retaining its underlying diagnostic.
There is no partially usable domain or public unchecked state constructor.

Different interior values may share represented progress, as in spec58. Numeric
index movement still distinguishes their exact values; unchanged progress does
not mean unchanged value. This domain does not certify that a given physical
allocation visually separates every stop. Complete controls must retain keyboard
and semantic access to those values and communicate their localized value text.

Uniform and nonuniform scales use the same explicit list. This contract does not
infer a step size, base, interval count or default from bounds or a backend.
An authoring tool may generate a list, but must validate the complete result.
For example, `[0, 0.1, 0.3, 0.7, 1]` retains those declared values instead of
repeatedly adding a floating-point amount. Continuous controls continue to use
spec59 and have no implicit stop list. This is a finite discrete domain, not a
claim to represent infinitely many points or a decimal step-expression grammar.

Every adjustment supplies the checked domain, checked current value IR and live
enabled/readOnly flags. Current bounds must match the domain and current value
must equal a permitted stop. An off-domain current value fails even when disabled
or read-only; nearest selection is an explicit edit intent, not authoring-data
sanitization. Bounds or stop-meaning changes require a newly checked domain and
the component revision change required by spec63/65. Exact equality has no
arbitrary tolerance or locale-dependent comparison.

## Intents and resolution

| Intent | Behavior |
| --- | --- |
| setValue(value) | Require a finite in-range value equal to an allowed stop. |
| nearest(value, tieBreak) | Require a finite in-range candidate, select its closest allowed numeric stop, and use explicit lower/higher policy only for a true equal-distance tie. |
| increase(count), decrease(count) | Require a positive integer count; move by that many indices, saturating at the corresponding domain endpoint. |
| minimum, maximum | Select the first or last allowed value. |

Counts are numbers of stops, not amounts in value units. The Rust typed reference
accepts unsigned 64-bit counts; a count greater than remaining indices reaches
the endpoint, including when it exceeds a target's native indexing capacity.
No wrapping or skipped final partial step exists. Repeated accepted index edits
retain exact declared numbers instead of accumulating arithmetic drift.

Nearest selection preserves an exact existing stop. Otherwise search the adjacent
lower and higher stops and compare their exact distances from the represented
candidate. A rounded subtraction that makes unequal distances look equal must
not invoke tie policy. Overflow in one distance cannot make that farther stop
win; unsupported arithmetic fails explicitly. Lower/higher refer to numeric
value, independently of orientation, RTL, minimum placement or pointer movement.
The Rust binary64 reference uses magnitude-ordered FastTwoSum residuals for equal
finite rounded distances, sharing the existing spec59 arithmetic primitive.
Decimal-looking literals follow spec58's represented numeric semantics; their
displayed decimal midpoint need not be a true equal-distance binary64 tie.

Validate current-domain coherence and intent parameters before permission guards.
Then apply the selected absolute allowed value through spec59. Enabled and
writable edits return accepted complete adjustment IR; disabled/readOnly edits
return unchanged current IR with accepted false. Accepted endpoint/no-op is
distinct from changed. No default value, count or tie policy exists.

## Integration and fallback

Owners commit complete next value before notifying once when changed. This pure
operation executes no callback and owns no focus, revision, edit session or native
input routing. It is usable in every capability tier without animation, shaders,
transparency or backend detection.

The [W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/)
describes step movement and first/last allowed values, with optional larger page
changes. Its keyboard examples inform separate key policy; they do not prescribe
a default amount or validate a particular native input route.
[HTML step semantics](https://html.spec.whatwg.org/multipage/input.html#the-step-attribute)
use a step base and allowed step. HTML's authoring defaults and sanitization do
not define this explicit portable domain.

Keyboard owners map actual keys to explicit stop-count intents and retain actual
focus and shortcut policy. Pointer owners first resolve a bounded numeric candidate
from checked geometry, then choose nearest with explicit tie policy before
publishing a discrete preview. Complete stepped edit/pointer composition must
preserve the existing cancellation/revision/routing contracts and revalidate the
domain on every delivery. The continuous-only edit/pointer operations do not
automatically enforce this domain. Connecting that policy and native keyboard/AT
delivery remains required before publishing a complete discrete Slider.

## Conformance boundary

[Domain cases](../conformance/interaction/slider-stops-cases.json) verify complete
checked domains and failures; [adjustment cases](../conformance/interaction/slider-stop-adjustment-cases.json)
verify complete existing adjustment results, permissions, exact membership,
fractional/nonuniform scales, index saturation, true ties, false rounded ties,
overflowed distances and extreme/subnormal domains. Expected distances and
progress derive independently from exact rational represented inputs.

The [IR schema](../schemas/slider-stops-ir.schema.json) requires complete values
and rejects duplicates structurally. [Domain](../schemas/slider-stops-cases.schema.json)
and [adjustment](../schemas/slider-stop-adjustment-cases.schema.json) schemas
describe explicit typed-operation test records, including negative inputs; they
are not production source protocols. Relational ordering, shared bounds,
progress, membership and nearest-distance decisions require runtime conformance.
The Rust reference has checked construction and serialization, with no production
deserialization/source adapter for the domain. Native input, assistive technology
and styled material/fallback controls remain full component requirements.
