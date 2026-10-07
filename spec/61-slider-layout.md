# Slider part allocation (candidate, 0.1.0)

Blueprint sections 85, 98 and 177 require authored anatomy, adaptation and
lower-capability rendering. This contract allocates a single Slider's track and
thumb before independent shape/material/paint resolution. It depends on
[bounded value IR](58-slider-value.md); semantics use [spec60](60-slider-accessibility.md).

## Inputs

Supply explicit positive finite allocationSize, thumbSize and trackThickness in
logical px, finite nonnegative logical start/end/top/bottom insets, actual
layoutDirection, horizontal/vertical orientation, minimumPosition start/end and a
checked complete current bounded value IR. No size, axis, direction, endpoint,
value or inset default is inferred. An oversized part is not shrunk to fit.

Both parts fit independently within the inset outer allocation. A thumb may be
larger than the track's cross extent; the Toggle policy of fitting a thumb inside
its track is not used. TrackThickness may also exceed the thumb's cross extent if
both fit the allocation. Insets reserve layout space, not a material-specific gap.

## Allocation

Allocation bounds have origin (0,0) and the supplied size. Horizontal physical
left/right insets follow start/end in LTR and end/start in RTL. Top/bottom stay
physical. The thumb is centered across the inset cross axis; its main-axis origins
range from the physical low inset to total main extent minus high inset minus
thumb main extent. Travel MUST be positive. The track is independently centered
across the same cross axis, has the supplied thickness, and extends between the
two endpoint thumb centers. Track bounds describe the full track allocation,
not an inferred filled-value segment or painted silhouette.

Horizontal axis start is logical start: physical left in LTR and right in RTL.
Vertical axis start is physical top and end is bottom. MinimumPosition explicitly
selects which axis end owns the minimum numeric value; the maximum owns the other.
No numeric direction is inferred from axis, language, a backend or key mapping.
For example, a caller can explicitly choose horizontal minimum at start and
vertical minimum at end. RTL mirrors horizontal placement and vertical cross-axis
insets; it does not reverse vertical main-axis placement or physical lighting.

Normalized progress zero and one select exact minimum/maximum thumb origins.
Interior progress interpolates those origins, preserving the authored thumb size
and cross-axis position. The [result schema](../schemas/slider-layout-ir.schema.json)
requires the complete value IR, axis/direction/minimumPosition, allocation bounds,
track bounds, both numeric endpoint thumb bounds and current thumb bounds.
Geometry remains independent of enabled/readOnly/focus/hover/press state.

The reference uses finite binary64 arithmetic. Positive subtraction/extent and
centering terms must not disappear; endpoint centers must remain distinguishable.
Interior progress must produce a strict interior origin, never a false endpoint.
Overflow, underflow that removes positive extent/slack, failed fit and no travel
produce no result. The shared [exact containment predicate](40-hit-region-ir.md)
retains endpoint addition residuals when comparing all part rectangles against
both the inset interior and the outer allocation. Shape schema validation alone
cannot certify relational containment, interpolation or value invariants.

## Ownership and subsequent resolution

Dimensions are authored or token-resolved inputs. This operation chooses no
capsule, radius, pigment, elevation, material family or response. Resolve each
part's authored shape and material roles through their independent contracts.
Full localized labels/descriptions/value text own measured layout; text scaling
precedes choosing the total control allocation. Labels are not truncated or
resized by this operation. Current semantics and allocation must share one
committed value, axis and control identity.

The owner reserves one stable target using the complete actual painted footprint
across both endpoints and all current/moving parts, including extrusion where
applicable, plus current clipping/neighbors and the existing minimum target rule.
Allocation rectangles alone do not certify painted-footprint coverage. Track or
thumb movement must not recenter a reserved target. Reduced-motion/static paint
can consume exact current allocation; animation and gesture owners still need
stable target, cancellation, current layout mapping and coherent commit contracts.
No pointer routing, capture, filled-track feedback, focus paint, native tree or
component-complete rendering is supplied here.

## Evidence and reference boundary

[Public vectors](../conformance/geometry/slider-layout-cases.json) cover both axes,
both layout directions and explicit minimum positions at five numeric values,
thin tracks with larger thumbs, asymmetric/fractional/zero insets, invalid sizes,
fit, no travel and arithmetic loss. Valid expected rectangles are computed from
exact rational arithmetic on represented inputs. Their fixture dimensions are
not calibrated component defaults. Tests compare complete output, verify mirror
and scaling identities, and reserve one unchanged target containing track/thumb
allocations across values. They do not certify actual paint coverage.

The [case schema](../schemas/slider-layout-cases.schema.json) describes typed test
records, including invalid numeric parameters; it is not a production request
protocol. The Rust reference exposes resolve_slider_layout over checked typed
input. Shared SliderOrientation is owned by the portable model and re-exported
by the resolver for both semantics and layout. The public source boundary below
resolves that same typed contract without backend dependencies.

## Public source boundary

The [request schema](../schemas/slider-layout-request.schema.json) requires
`schemaVersion` `0.1.0` and every authored input above. `value` is a complete
spec58 bounded value request, resolved before layout; supplied progress is
rejected. Its numeric literals retain spec58's exact integer-source policy.
Geometry numbers are interpreted as finite binary64 values by the reference,
as in the existing geometry operations. No direction, inset or size default is
inferred. Unknown, missing, duplicate or wrongly typed members, unsupported
versions, nonfinite values and failed value/geometry validation publish no result.
Requests, sizes, insets and values must be JSON objects; axis, direction and
minimum position must be strings. Positional arrays and object-form enum values
are invalid source representations.

`resina-slider-layout <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits
0 with complete layout IR and no diagnostic; invalid input exits 1 with a
diagnostic and no output; usage exits 2. The executable size guard does not
define a semantic geometry limit. `resolve_slider_layout_source` exposes the
same strict boundary in the reference library.

[Protocol cases](../conformance/geometry/slider-layout-protocol-cases.json)
preserve the typed vectors and cover source shapes and numeric precision. The
[external checker](../tools/check_slider_layout_backend.py) reconstructs all
successful rectangles from exact rational arithmetic on represented inputs.
It compares geometry with the shared absolute or relative `1e-12` tolerance;
retained allocation, thumb sizes and track thickness, numeric values and endpoint
selection are exact. Part bounds must remain inside the authored inset interior
under exact containment, and endpoint centers must remain distinguishable.
Interior progress uses spec58's four binary64 ULPs, and interior thumb placement
must remain strictly between its actual endpoints. Nested duplicates,
nonfinite geometry and values, deterministic publication and failure without
partial output are checked on Linux and Windows. This boundary adds no renderer
dependency and does not certify rendered paint, hit coverage or native input.
