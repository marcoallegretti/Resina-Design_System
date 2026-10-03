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

Resolve the existing environment minimum (24 units, or 48 for coarse pointer or
direct touch). Each axis minimum is the larger of that floor and the corresponding
component minimum. Each hit dimension is the larger of that minimum and the
visual dimension. Center the extra extent on the body: `hit.x = visual.x -
(hit.width - visual.width)/2`, and likewise for y. Do not mirror physical bounds
under RTL, shrink targets with density/text/device scale, move the body, or shift
the hit region to hide insufficient space.

Coordinates and dimensions MUST be finite; all dimensions MUST be positive.
Every rectangle's right/bottom endpoint MUST be finite, greater than its origin,
and retain at least its declared dimension when the origin is subtracted. The
computed rectangle MUST fully contain the visual bounds and lie within available
bounds. Fail explicitly when arithmetic cannot represent these extents or when
the full target would be clipped. No numerical tolerance may admit an undersized
or clipped target. Layout must then reserve enough space or change placement.

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
