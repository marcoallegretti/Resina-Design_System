# Slider track segments (candidate, 0.1.0)

A Slider shows its value through the thumb's position and through which part of
the track lies on each side of it. This contract divides the track allocation
from [part allocation](61-slider-layout.md) into an active segment and an
inactive segment, separated from the thumb by an explicit clearance. It owns no
roles, pigment, shape or response, and it does not change the layout's track,
which remains the pointer and value-mapping track.

## Why a clearance

The thumb crosses the track. Where it does, its edge may border any color of the
track's paint, including the track's own edge. The
[snapshot](77-slider-snapshot.md) therefore checks the thumb edge against the
whole track cover. A clearance between thumb and track is the geometry a future
snapshot profile needs to prove narrower adjacency, in which the thumb borders
the canvas instead of the track; this contract supplies that geometry and does
not change the snapshot's checks.
[Material's slider](https://github.com/material-components/material-components-android/blob/master/docs/components/Slider.md)
added a gap between thumb and track, with its own corner size towards the
thumb, as part of its non-text contrast update. The gap also lets the two
segments carry different color roles without a seam under the thumb.

## Resolution

Supply checked layout IR and a finite, nonnegative `clearance` in logical px.
There is no default clearance. On the layout's main axis, let the track span
`[t0, t1]` and the current thumb span `[h0, h1]`. Each segment is that side of
the track minus the clearance around the thumb:

- the low segment spans `[t0, min(h0 − clearance, t1)]`;
- the high segment spans `[max(h1 + clearance, t0), t1]`.

Each segment keeps the track's cross-axis position and thickness. A segment is
present when its span, after the rounding below, is nonempty; otherwise it is
absent. Presence does not depend on whether a segment is long enough for a
particular shape, which the owner decides. The active segment is the one on the
minimum-value side, which is the low side when the layout's minimum thumb lies
before its maximum thumb on the main axis. Horizontal and vertical axes, layout direction and minimum
position are therefore taken from the layout, with no further inference. Filling
from the minimum matches Material's standard slider; a fill origin elsewhere,
such as the middle of a centered slider, or a range with two thumbs, is outside
this contract.

At either endpoint, the segment on that side is absent. A clearance larger than
the track leaves both absent. A nonfinite or negative clearance fails with no
result.

The reference computes in binary64 and rounds every endpoint towards the
segment's interior, so published bounds never exceed the exact spans above.
`t1` is the track's origin plus extent rounded down, and `h1` is the thumb's
origin plus extent rounded up. `h0 − clearance` rounds down and
`h1 + clearance` rounds up; when the latter exceeds the largest finite value,
the high segment is absent. Each extent is its end minus its start, rounded
down. Exactly representable values are unchanged.

The [result](../schemas/slider-track-segments-ir.schema.json) contains
`schemaVersion` `0.1.0`, the clearance, and `active` and `inactive` bounds or
`null`. Segment bounds lie within the track, outside the clearance around the
thumb, and never overlap.

## Evidence and boundary

The [public cases](../conformance/geometry/slider-track-segments-cases.json)
apply clearances 0, 6, 1000, 0.1 and 1/3 to every valid case of the public
layout vectors: both axes, both directions, both minimum positions, endpoint
and interior values, and fractional and zero insets. Further cases place a
clearance where a segment's span is empty or shorter than the track thickness,
and apply the largest finite clearance. Their expected bounds are derived with
exact rational arithmetic from the layout vectors' expected geometry and the
rounding above. Negative clearances fail. Rust tests replay every case, check
containment, clearance and disjointness exactly, and reject nonfinite
clearance. The [case schema](../schemas/slider-track-segments-cases.schema.json)
describes the records; it is not a production request protocol.

This operation chooses no clearance value and makes no adjacency claim. The
owner chooses one that covers the thumb's complete painted footprint, including
extrusion and the navigation ring, and remains responsible for the colors that
actually border the thumb. Segment roles belong
to component anatomy; segment paint uses [Slider part paint](76-slider-part-paint.md)
with the track response.
