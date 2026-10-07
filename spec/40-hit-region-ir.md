# Rectangular hit region IR (candidate, 0.1.0)

Blueprint §§34–35 and the [minimum target contract](15-targets.md) require an
actual interactive region distinct from the visual body. A circular or clipped
24-unit bounding box does not contain the required square, as illustrated by
[W3C target-size guidance](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html#size-requirement).
This operation resolves an axis-aligned rectangular hit region. Its complete
rectangle MUST remain active; clipping it to the painted contour invalidates
this result. It supplies geometry, not activation, focus order or a widget.

The [request](../schemas/hit-region-request.schema.json) supplies version `0.1.0`,
one explicit environment snapshot, `visualBounds`, `availableBounds`, an explicit
positive `componentMinimum` size and `occupiedRegions`. Bounds use physical x/y
axes in logical `px`, with one shared coordinate origin. Supply body bounds,
excluding a separate navigation ring; for a swept body include its visible
footprint. Supply the actual available rectangle after ancestor clipping and
safe areas, and the actual neighboring hit rectangles. An empty neighbor array
is explicit; the resolver MUST NOT infer missing placement, clipping or neighbors.

The source request, `visualBounds`, `availableBounds`, `componentMinimum` and
individual occupied bounds MUST be JSON objects. Positional records are invalid;
`occupiedRegions` itself remains the explicit ordered array of bounds objects.
All records must be validated before publishing a result.

Resolve the existing environment minimum (24 units, or 48 for coarse pointer or
direct touch). Each axis minimum is the larger of that floor and the corresponding
component minimum. Each hit dimension is the larger of that minimum and the
visual dimension. Center the extra extent on the body: `hit.x = visual.x -
(hit.width - visual.width)/2`, and likewise for y. Do not mirror physical bounds
under RTL, shrink targets with density/text/device scale, move the body, or shift
the hit region to hide insufficient space.

Coordinates and dimensions MUST be finite; all dimensions MUST be positive.
The `0.1.0` command boundary decodes numeric fields to IEEE 754 binary64 using
round-to-nearest, ties-to-even. Right/bottom endpoints mean the mathematical
sums `x + width` and `y + height` of those decoded values, retaining addition
residuals when necessary for predicates.
Every endpoint MUST have a finite representation greater than its origin. The
computed rectangle MUST fully contain the visual bounds and lie within available
bounds. Fail explicitly when arithmetic cannot represent these extents or when
the full target would be clipped. No numerical tolerance may admit an undersized
or clipped target. Layout must then reserve enough space or change placement.

Predicates MUST preserve the sign of endpoint differences when rounded endpoint
sums coincide. In particular, do not subtract a rounded endpoint from its origin
to certify the minimum, or compare only rounded sums to certify containment,
clipping, overlap or membership. The Rust reference uses binary64 TwoSum
expansions for exact endpoint comparisons, following
[Shewchuk's compensated arithmetic](https://www.cs.cmu.edu/~quake/robust.html).
This numerical technique does not add arithmetic implementation details to IR.
The [membership vectors](../conformance/interaction/hit-membership-vectors.json)
are independently checked with exact rational arithmetic and exercised against
the Rust hit-membership API, including both sides of rounded endpoints.

Reject positive-area intersection with any occupied region. Boundaries may
touch: hit membership includes left/top and excludes right/bottom, so shared
edges have one owner. A finite point belongs exactly when `x >= left && x < right
&& y >= top && y < bottom`; nonfinite points fail. This rule does not choose among
overlapping regions, accept an overlap because one control is disabled, or
certify an occupied neighbor's own minimum. Consumers MUST recompute when bounds,
environment, component requirements or neighboring reservations change.

The [result](../schemas/hit-region-ir.schema.json) contains version `0.1.0`, the
complete `bounds` and resolved `minimumSize`. Publish no result after any failure.
Unknown/duplicate members, missing inputs, unsupported versions and invalid
nested environment or bounds fail. Normative IR includes no toolkit, renderer,
pointer device identifier, event handler or product-specific control type.

`resina-hit-region <path|->` reads one UTF-8 request up to 1 MiB. Success exits 0
with complete JSON; input or resolution failure exits 1 with a diagnostic and no
result; usage errors exit 2. The public cases and external checker exercise
minimum resolution, placement, clipping, overlap and strict transport. This does
not establish complete component interaction or accessibility conformance.

## Resolved surface composition

The Rust reference provides `resolve_surface_hit_region` for a validated opaque
surface body, including the opaque fallback of Frost. It derives `visualBounds`
from the body's complete resolved silhouette, including extrusion, and applies
the same rectangular target policy and diagnostics. Pass the body from static
or sampled command paint. A command's label size or front contour omits its
swept footprint; a navigation ring is not part of its body. Unsupported visible
geometry fails explicitly.

The environment, available rectangle and occupied reservations MUST refer to
the same layout snapshot as the body. All coordinates remain in the body's
physical logical-pixel frame, including negative silhouette coordinates. The
owner maps placement transforms and native pointer coordinates into that frame
once; this operation neither guesses a placement nor converts device pixels.
Layout reserves stable interaction geometry over a command's supported feedback
domain; do not move an active target solely because sampled extrusion changes.
Use the [command snapshot contract](49-command-snapshot.md) to revalidate that
reservation and check each actual body footprint.
Disabled availability does not release its occupied reservation. The resulting
rectangle remains fully active even outside rounded paint or label bounds;
actual invocation remains subject to the current activation lifecycle.

Integration evidence connects complete label containment, interaction projection,
static and sampled command paint, target membership and accessible availability.
It checks body footprint coverage, independent focus geometry, clipping and
neighbor conflicts. Label extents in these cases are supplied layout arithmetic
fixtures, not font measurements. These headless cases do not establish native event routing,
pointer capture, assistive technology delivery or a complete Button.

`HitRegionIr::contains_bounds` checks closed geometric containment of another
positive finite rectangle using exact endpoint residuals. Right/bottom boundaries
are included for complete body coverage; point membership remains half-open for
shared-edge ownership. Invalid or unrepresentable input bounds fail diagnostically.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
