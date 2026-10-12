# Focus geometry for continuous outlines (candidate, 0.1.0)

The [focus policy](27-focus-indicator.md) keeps its opaque guarded color,
2 logical `px` gap, 2 logical `px` stroke and concurrent states. This operation
extends the geometric proof to [circular/quartic outlines](81-surface-outline.md).
Existing [circular/square focus IR](37-focus-indicator-ir.md), shape assignments,
relief, paint, elevation and shadow retain their contracts and outputs.

## Footprint and offsets

Let `F` be the supplied resolved convex outline and `d` an explicitly resolved
physical side offset. Its declared footprint is `S = F ⊕ [0,d]`. The caller must
supply the actual painted footprint: zero offset for an unprojected face, or the
actual resolved projection for a solid. Nominal thickness does not create a
projection. Logical direction maps authored corners once; `d` does not mirror.
An unresolved active optical treatment fails rather than acquiring fabricated
geometry.

For the closed Euclidean disk `D(a)`, the inner boundary is `S ⊕ D(gap)` and the
outer boundary is `S ⊕ D(gap + strokeWidth)`. The visible ring is outer minus
inner, with the outer boundary included and the inner boundary excluded. This
retains the [Minkowski disk-offset law](https://doc.cgal.org/latest/Minkowski_sum_2/index.html)
independently of any toolkit stroke or backend path.

Do not increase elliptical or quartic corner axes to approximate these sets.
For equal-axis quartic corners, that construction changes support by
`a * 2^(1/4)` in a diagonal unit direction, rather than `a`. A requested 2 px
offset becomes about 2.378 px. A representation must retain the original outline
and explicit sweep/disk extents; it must not renormalize enlarged corner axes.

## Independent support proof

For a unit physical normal `n`, define `hA(n) = max(p·n : p in A)`. Require:

```text
hS(n) = hF(n) + max(0, d·n)
hInner(n) = hS(n) + gap
hOuter(n) = hS(n) + gap + strokeWidth
```

These identities determine each closed convex offset and establish the gap and
stroke without assuming a rendering algorithm. At a zero sweep dot product,
either endpoint maximizes support; use the untranslated endpoint for a
deterministic reported witness.

Choose the physical corner in the normal's quadrant, using right/bottom when
the corresponding normal component is zero. Let its extents be `a,b`, its center
be `c`, and its power be `p = 2` or `4`. With `q = p/(p-1)`, its support is:

```text
L = ((a*abs(n.x))^q + (b*abs(n.y))^q)^(1/q)
hF(n) = c·n + L
```

For `L > 0`, an exposed point is `c` plus the signed components
`a*((a*abs(n.x))/L)^(q-1)` and
`b*((b*abs(n.y))/L)^(q-1)`. For a square corner both extents are zero and the
exposed point is the physical rectangle corner. Axis normals also expose a
straight segment; this convention chooses its corner-join endpoint. Add `d`
exactly when its dot product is positive, then add each disk extent times `n`.
This is the maximizer of the bounded power constraint, rather than a radial
normal approximation. Scalar support values and exposed points must agree.

Normalize finite nonzero query directions without overflowing their magnitude.
Normalization multiplies the direction by a positive scalar: determine sweep
endpoint signs and zero ties from the represented input direction before
rounding the reported unit normal.
Reject nonfinite, zero, unrepresentable and uncertifiable arithmetic instead of
inventing a direction, silently dropping a projection or losing gap/stroke.
The enclosing bounds are the actual sweep bounds expanded by each disk extent.

## Public boundary

`resina-outline-focus <path|->` accepts one UTF-8 request of at most 1 MiB,
including when read from stdin. The versioned request, analytic IR, result and
case schemas are `schemas/outline-focus-*.schema.json`. A request supplies
`schemaVersion: 0.1.0`, the existing `focus` color request, the existing `outline`
request, physical `sideOffset` and an ordered `directions` array. An empty array
requests the complete IR without support witnesses. Objects must remain objects;
positional arrays, duplicate members and unknown fields are rejected.

A valid request returns the complete `focus` IR and ordered `supports` in one
JSON result with clean exit 0. Invalid geometry, requests or arithmetic return
exit 1, a diagnostic on stderr and no partial stdout. Invalid command usage
returns exit 2. Runtime checks supplement schema validity: a finite direction
can still lose a component during normalization, and finite geometry can lose
the positive disk extent. No result may conceal these failures.

`conformance/states/outline-focus-cases.json` composes the established outline
vectors and focus scenarios. `tools/check_outline_focus_backend.py` verifies
the public command against those outlines and an independent 100-digit support
calculation. Backend approximation requires separate evidence.

## Ownership and verification

The focus color operation resolves the supplied focused scenario and actual
opaque surrounding color unchanged. Geometry retains that complete result,
the original resolved outline, physical side offset, disk extents and enclosing
bounds. Ordered support queries retain normalized normals, exposed points and
scalar support for silhouette, inner and outer. Geometry is logical `px`;
no device scale, tessellation, toolkit path or rendering object is normative.

Proof must cover all normal quadrants, axis joins, circular/quartic and asymmetric
corners, square corners, normalization, both layout directions, zero and signed
projection, concurrent focus states, guarded color fallback and numerical loss.
An independent checker must compare exposed support against the power equation
and the sweep/disk identities, including the diagonal counterexample above.
Coordinates and support use absolute or relative `1e-12`, whichever is larger;
gap and stroke must remain representably positive regardless of that tolerance.
Agreement between a rounded point dot product and a rounded scalar is insufficient
under cancellation. Certification must bound the real normalization, power
boundary, selected sweep and disk arithmetic from the represented inputs. A
query outside that accuracy guarantee fails without relaxing the tolerance.

The reference encloses its represented binary64 calculation with outward-rounded
intervals and exact cube-root brackets. This is a reference numeric strategy;
another implementation may use a more accurate calculation. An `accuracyProbe`
case permits either an atomic precision rejection or a complete result that
passes the same independent geometry and color checks. Rejection must not become
a mandatory semantic outcome merely because the reference cannot certify a
mathematically valid calculation.

This operation describes complete analytic focus geometry. It does not certify
backend approximation, rasterized area, clipping, occlusion, accepted component
appearance, input or accessibility delivery. A backend must realize the declared
sets with independent device-space evidence and reserve the complete outer bounds.
