# Opaque relief face paint (candidate, 0.1.0)

Compose [bounded relief](82-surface-relief.md), [opaque pigments](29-opaque-pigment.md)
and the existing physical [key light](34-key-light.md) without changing semantic
elevation, thickness, shadow, targets or navigation. This operation paints a
local face, not a complete component or a projected solid. Material family does
not select its outline or relief profile. An opaque face requires neither blur,
transparency, shaders nor a particular renderer.

## Face modulation

Let `n` be the full analytic unit normal and `d` the normalized physical planar
key-light direction. Use `q = clamp(n.x*d.x + n.y*d.y, -1, 1)`. Do not normalize
`n.xy`: doing so would erase tilt strength and introduce a discontinuity at a
flat plateau. For positive q, mix body to the existing highlight pigment with
weight q; for negative q, mix body to the existing side pigment with weight -q.
Mix each encoded sRGB channel as `body*(1-weight) + endpoint*weight`, with alpha
exactly one. Zero q MUST return the exact body color. Recessed and raised faces
invert directional modulation. Physical lighting does not mirror with logical
layout direction. This is bounded pigment treatment, not a photometric BRDF.

Cover both continuous mixtures with separate outward-rounded channel ranges,
alongside the uniform exterior edge. Four binary64 epsilons followed by outward
adjacent values cover rounding of the bounded interpolation and its endpoints;
clamp only the range bounds to gamut. These ranges describe possible source
colors, not renderer filtering, antialiasing or actual spatial adjacency.

## Exterior edge and uniform content

Require a finite positive representable edge inset i, with positive inset-box
area. The edge interior is the intersection of `[i,W-i] × [i,H-i]` and all four
derived corner constraints. Retain each outer physical corner center. For an
outer corner with axes a,b both greater than i, replace its axes by a-i,b-i and
require the clipped normalized power sum to be at most one, using the outer
curve's power (2 or 4). If either axis is at most i, the inset box cannot enter
that outer corner, so its constraint is absent. Apply every constraint globally;
derived axes can overlap. Do not renormalize them into another surface outline.
This is a conservative inset region, not an exact Euclidean offset.

For a constrained corner, outer normalized coordinates equal the inner
coordinates multiplied by `(1-i/a)` and `(1-i/b)`. Thus its inner power sum at
most one implies its outer power sum at most one. The box and each clipped-power
sublevel set are convex. Their intersection is contained by the outer face.
Paint the face outside this interior with the existing edge contrast result.

Reserve one positive representable content rectangle inside both this interior
and the relief's uniform plateau. For each outer corner axis larger than i use
diagonal pad `i + (axis-i)*(1-2^(-1/power))`, with an outward numerical guard;
for an axis at most i use i. This can over-reserve an absent corner constraint.
Combine incident
corner pads and the relief's content bounds. Check all four published vertices
against the edge interior and require exact body paint there. Convexity and the
relief plateau contract establish containment of the complete rectangle.
Reject collapsed or unrepresentable reservations. Foreground must be opaque and
meet the supplied finite content threshold in [1,21] against the exact body.
The edge candidate is selected by the existing contrast operation against the
actual supplied adjacent background. Content belongs inside the published
rectangle; this operation does not certify text placed over a shoulder or edge.

## Public boundary

The [request](../schemas/relief-paint-request.schema.json) contains version
0.1.0, a complete outline request, relief profile,
complete opaque pigment request, key light, complete edge contrast request,
edge width, opaque foreground, minimum content contrast and ordered physical
points. Each nested operation retains its strict contract. Reject unknown,
missing, duplicate, positional, nonfinite and unsupported inputs. Validate all
inputs, including unused candidates, before publishing any result. Any relief
precision failure fails the complete request without partial samples.

The [result](../schemas/relief-paint-result.schema.json) contains version 0.1.0,
one resolved paint object and ordered samples.
Finite points outside the face yield null; points inside yield opaque color.
The paint object retains relief, pigment, normalized physical direction, edge
result and width, foreground, content contrast and the reserved content bounds.
An empty point array is valid; it does not promise every shoulder can satisfy
the relief accuracy requirement. `resina-relief-paint <path|->` follows the
existing 1 MiB UTF-8 command boundary and status 0/1/2 convention.

Proof includes independent signed-normal interpolation, circular/quartic and
asymmetric edge containment, complete plateau grids, numeric rejection,
[authored cases](../conformance/ir/relief-paint-vectors.json), strict
schema/protocol cases and unchanged prior paint output. Run
`python tools/check_relief_paint_backend.py -- <backend-command> -` to evaluate
the public boundary. Raster, GPU,
fractional-scale Light/Dark scenes and native content/focus integration require
separate executed evidence before complete surface appearance is accepted.
