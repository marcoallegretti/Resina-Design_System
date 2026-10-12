# Bounded surface relief (candidate, 0.1.0)

Surface relief supplies a local face height and geometric normal over an explicit
[bounded outline](81-surface-outline.md). It does not select semantic material,
elevation, shadow, target, focus, navigation, layout or light. Existing opaque
surface IR and its outputs retain their own contracts. No negative extrusion
represents a recess.

## Ownership and representation

An explicit profile supplies `kind` (`flat`, `convex` or `recessed`), positive
finite nominal `bodyThickness`, nonnegative finite `amplitude` and
`shoulderWidth`, all in logical `px`. The nominal front plane is height zero;
the nominal solid extends down to `-bodyThickness`. A convex face rises from
zero to `amplitude`; a recessed well falls to `-amplitude`. The body thickness
is independent of semantic elevation and shadow displacement. This operation
does not project a side plane or infer a shadow from either quantity.

The word convex names the raised face profile. It does not claim positive
principal curvature everywhere: a smooth shoulder joining a flat edge and a
flat content plateau necessarily changes its curvature. The filled outline
remains convex; the well is a local height profile, not a concave outline.

Flat requires amplitude and shoulder width exactly zero. Other kinds require
both positive and `amplitude <= shoulderWidth/2`. Recessed amplitude must be
strictly below body thickness. Convex `bodyThickness + amplitude` and recessed
`bodyThickness - amplitude` must remain finite, positive and distinguishable
from body thickness. The positive ratio `amplitude/shoulderWidth` and subtraction
of shoulder width from each surface dimension must remain representable.
Requested dimensional distinctions lost to precision fail.

The face uses a bounded product of analytic transition gates. This avoids
closest-point iteration, arbitrary meshes, signed extrusion and requirements
for SDF or shader execution. A consumer can sample the same function into
ordinary opaque pixels. Sampling and paint remain separately verified contracts.

## Height field

Let `w = shoulderWidth`, `A = amplitude` and `n` the outline's exponent (2 or 4).
Use the quintic transition, extended constantly outside its unit interval:

```text
S(t) = 0                                 when t <= 0
S(t) = t^3 * (10 - 15*t + 6*t^2)         when 0 < t < 1
S(t) = 1                                 when t >= 1
S'(t) = 30*t^2*(1-t)^2                   inside (0,1), otherwise 0
```

Its value, first derivative and second derivative join continuously to the
constant endpoints. Its maximum first derivative is `15/8` at `t=1/2`.
For physical `(x,y)` within width `W`, height `H`, use four straight gates:
`S(x/w)`, `S((W-x)/w)`, `S(y/w)` and `S((H-y)/w)`.

For each nonzero physical corner `(a,b)`, let `(d,e)` be distances from that
rectangle corner toward its interior. Define:

```text
u = max((a-d)/a, 0)
v = max((b-e)/b, 0)
F = 1 - u^n - v^n
tau = n * (w/a + w/b)
corner gate = S(F/tau)
```

Every nonzero corner requires representable finite ratios and `0 < tau < 1`.
Square corners contribute a constant gate of one. Let `B` be the product of
all eight gates. Local height is `A*B` for convex and `-A*B` for recessed.
Flat height is zero. Outside the outline there is no face sample.

Differentiate the product by summing each gate's derivative times the other
gates; do not divide by a gate because boundary gates can be zero. The normal
is the normalized vector `(-dz/dx,-dz/dy,1)`, pointing toward the viewer in
physical x-right, y-down coordinates. Layout direction does not mirror light
or this coordinate frame. For fixed quartic corners the height field is C2;
the circular alternative is C1 at clamped corner-axis joins. Square outlines
retain sharp outline corners despite a smooth height field.

The reference evaluates derivatives scaled by `w` and multiplies afterward by
`A/w`. This preserves finite normals when physical dimensions are extremely
small or large. Each corner's scaled derivative is bounded by `15/8`; at most
two straight and four corner terms affect an axis. Thus each height derivative
has magnitude at most `(6*15/8)*(A/w) <= 45/8`. Normal evaluation needs no
unbounded physical-coordinate square or reciprocal of a tiny dimension.

## Uniform content plateau

Content must lie entirely where every gate equals one. Reserve each physical
side at least its outline content padding and `w`. For each incident nonzero
corner additionally reserve the corresponding axis extent times:

```text
g = 1 - ((1-tau)/2)^(1/n)
```

Both retained coordinates are then at most `((1-tau)/2)^(1/n)`, giving
`F >= tau`. Intersections of these convex superlevel regions contain the whole
reserved rectangle. The reference pads `g` by eight binary64 epsilons and
rounds multiplied extents upward. It verifies the four published vertices
have exactly the plateau height and normal `(0,0,1)`. Reject a collapsed or
unrepresentable reservation instead of placing text in a shoulder.

This reservation is conservative; it is neither an exact offset nor maximal
layout space. Recompute it for a circular alternative. A flat profile uses
the outline's existing content rectangle. Geometry does not measure text or
redefine interaction targets. Focus and navigation retain separate semantics.

## Source, failures and proof

The [versioned request](../schemas/surface-relief-request.schema.json) supplies
an explicit outline request, complete profile and ordered physical points.
Empty points are valid. Source records are objects;
point arrays contain vector objects, not positional records. Reject duplicate,
unknown or missing members, unsupported versions/kinds, nonfinite points,
invalid dimensions and all profile limits before emitting any result. A point
outside the outline produces a null sample, not an input error. All points are
validated even if outside the face.

The [result](../schemas/surface-relief-result.schema.json) records the resolved
outline, profile, content rectangle and ordered point samples with height and
unit normal or null. Numeric results use absolute
or relative tolerance `1e-12`, whichever is larger; source/result structure,
versions, kinds and inside/null classifications remain exact.

Finite inputs alone do not establish sampling accuracy. In particular, a thin
shoulder can amplify subtraction error in the corner residual. A requested
sample whose height or normal cannot meet the numeric tolerance fails with a
precision diagnostic; it must not yield an approximate successful sample or
partial result. Geometry with an empty points array can still resolve, and a
uniform plateau does not imply that every shoulder sample is representable.

The reference encloses the analytic calculation for its represented binary64
inputs with outward-rounded intervals, including coordinate subtraction,
corner powers, transition gates, product derivatives and normal normalization.
The quintic value is monotone on its clamped interval; its derivative has its
single maximum at one half. These facts bound gates without treating the
rounded corner residual as exact. Every output component must be within the
stated tolerance of the whole enclosure. Relative tolerance uses the enclosure's
minimum absolute magnitude. Basic operations and
[correctly rounded square root](https://doc.rust-lang.org/std/primitive.f64.html#method.sqrt)
support the enclosure; this is a reference numeric strategy, not a backend
identity or required rendering algorithm. Exact subnormal and Sterbenz
subtractions avoid unnecessary error expansion at supported tiny scales.

`resina-surface-relief <path|->` reads one UTF-8 request up to 1 MiB. Success
exits 0 with one complete JSON result and no diagnostic; invalid input exits 1
with no result and a diagnostic; usage errors exit 2.
[Authored vectors](../conformance/geometry/surface-relief-vectors.json) and the
[external public checker](../tools/check_surface_relief_backend.py) prove this
boundary. Independent equations, derivative checks, signed-profile symmetry
and plateau grids across numeric scales prove
the reference operation. These proofs do not accept appearance, lighting,
backend realization or native interaction.
