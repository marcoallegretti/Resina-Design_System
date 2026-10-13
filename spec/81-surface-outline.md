# Continuous surface outlines (candidate, 0.1.0)

This geometry operation supplies a bounded convex outline and a conservative
rectangular content reservation. Shape intent, material, elevation, relief,
shadow, color, interaction and navigation retain their own ownership. Existing
[Tier 0 shapes](09-geometry.md), [insets](33-inset-contour.md),
[extrusion](35-extruded-contour.md) and [opaque IR](36-opaque-surface-ir.md)
retain their versions and outputs.

## Representation decision

Use an explicit `circular` or `quartic` outline, with four independently authored
logical corner extents. The latter supplies continuous curvature at nonzero
corner-to-straight joins without a variable exponent or toolkit shape identity.
The former retains circular/elliptical geometry, including circular capsules.
Numeric extents do not infer any of the five semantic shape intents.

[Flutter's rounded superellipse](https://api.flutter.dev/flutter/dart-ui/RSuperellipse-class.html)
demonstrates separation of continuous geometry from conventional corner radii;
its rounded superellipse is not a plain superellipse. Its
[geometry implementation](https://github.com/flutter/flutter/blob/master/engine/src/flutter/impeller/geometry/round_superellipse.cc)
also explicitly separates exact geometry from a rounded-rectangle approximation.
[Skia's rounded rectangle](https://api.skia.org/classSkRRect.html) represents
elliptical corner radii. These are prior art, not Resina semantics or backend
qualification. Resina chooses a fixed algebraic curve for independently
evaluatable membership and conservative content proof. It does not claim
pixel equivalence to either implementation.

An arbitrary exponent, copied toolkit path, or negative elevation would expand
the contract without solving its containment requirements. Redefining the old
arc records would change existing conformance. A separate outline preserves
those records and makes the new representation explicit.

## Geometry

The [request](../schemas/surface-outline-request.schema.json) supplies version,
positive logical `px` bounds, four nonnegative finite logical elliptical corner
extents, layout direction, curvature and `minimumInset`. Normalize extents once
using the common [bounded-corner factor](09-geometry.md). Map logical corners to
physical top-left, top-right, bottom-right and bottom-left using the supplied
direction. A corner with either extent zero becomes a square corner with both
physical extents zero. Light and physical coordinates do not mirror.

Let `(x,y)` be distances from a physical rectangle corner measured toward the
rectangle interior, and `(a,b)` its normalized extents. Outside the bounds the
point is outside the filled outline. Within each nonzero corner rectangle
`0 <= x < a`, `0 <= y < b`, retain precisely:

```text
u = (a-x)/a
v = (b-y)/b
u^n + v^n <= 1
n = 2 for circular, 4 for quartic
```

All other in-bounds points are retained. The filled outline is convex. The
positive-quadrant level set has nonnegative curvature; its implicit Hessian is
positive semidefinite. For `n=4`, curvature tends to zero at each axis endpoint,
matching the straight edge. Square corners remain intentionally sharp. No
variable exponent, concave outline corner, mesh or path primitive is authored.
Recessed surface faces are a different operation from nonconvex outlines.

For equal extents, an arc can be evaluated using
`u = sqrt(cos(t)), v = sqrt(sin(t))`, `0 <= t <= pi/2` for quartic;
use `cos(t), sin(t)` for circular. An elliptical geometric normal is proportional
to `(u^(n-1)/a, v^(n-1)/b)` with physical signs appropriate to the corner.
A radial parameter alone is not that normal. Backends may approximate a curve
with conventional filled geometry to a proved device-space error; this operation
does not claim that any existing backend implements that approximation.

## Content reservation and fallback

Let `g = 1 - 2^(-1/n)`. Reserve left padding at least the greater of
`minimumInset`, `g * topLeft.x` and `g * bottomLeft.x`; derive the other three
physical paddings from their incident extents the same way. Subtract both side
paddings from each dimension. `contentBounds` is this conservative rectangle,
not an exact inward offset, maximal inscribed rectangle or component layout.
At each corner its retained normalized coordinates are at most `2^(-1/n)`,
so their powered sum is at most one. Convexity then contains the whole rectangle.

Round reservation inward toward retained area, never outward into a corner
void. The reference pads `g` by four binary64 epsilons to cover its square-root
and subtraction arithmetic, then rounds each multiplied padding upward to the
next representable value. It verifies all four published rectangle vertices
against its membership evaluator before returning. Numeric padding is included
in the comparison tolerance; consumers may not remove it to gain content space.

The circular outline with the same normalized extents is contained in the
quartic outline: for `u,v` in `[0,1]`, `u^4+v^4 <= u^2+v^2`.
This provides an explicit lower-complexity alternative, not permission to claim
quartic realization from an elliptical drawing. Recompute its content reservation
using `n=2`; quartic content bounds cannot be reused as circular-safe bounds.
An author must select a representation that the consumer can actually realize.
This headless operation neither infers capabilities nor changes environment data.

## Failures and proof

Reject unknown/missing fields, unsupported versions/curvature/direction,
duplicate JSON members, positional arrays, nonfinite/negative extents or inset,
and bounds without positive area. Validate extents even if the bounds collapse.
Content bounds must have finite, representable positive area. Requested positive
extents or padding lost to precision fail diagnostically instead of disappearing.
This includes numeric source decoding, before normalization or nested resolution;
a requested nonzero extent or inset must not silently become zero.
Invalid sampling points fail. Membership uses bounded normalized arithmetic;
the reference does not square unbounded physical coordinates.

The [result](../schemas/surface-outline-result.schema.json) contains only
curvature, logical size, physical extents and content bounds. Compare geometry
with absolute or relative tolerance `1e-12`, whichever is larger. Curvature,
structure, corner order, versions and diagnostic success/failure remain exact.
The [vectors](../conformance/geometry/surface-outline-vectors.json) use
independently evaluated algebraic expectations, including asymmetric RTL,
overlap, square, elliptical and capsule geometry, explicit reservation and
invalid/unrepresentable inputs. Reference tests independently prove fallback
containment, rectangle containment and logical mirroring on sampled grids.

`resina-surface-outline <path|->` reads one UTF-8 request up to 1 MiB.
Success exits 0 with one complete JSON result and no diagnostic; invalid input
exits 1 with no result and a diagnostic; usage errors exit 2. The
[external checker](../tools/check_surface_outline_backend.py) validates schema,
source/result consistency and deterministic repetition independently of Rust.
This geometry proof does not establish relief, paint, focus, targets, text
measurement, accessibility, rendered appearance or native runtime conformance.
