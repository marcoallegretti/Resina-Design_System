# Lit control body (candidate, 0.1.0)

The [design decision](https://github.com/marcoallegretti/Resina-Design_System/issues/129)
makes Command and Toggle part bodies lit relief solids. This contract embeds the
[material-lit relief face](86-material-relief-paint.md) as the front plane and
projects it with the existing [side plane and swept edge](36-opaque-surface-ir.md).
It does not acquire observations, select component anatomy or sizes, animate,
reserve targets, change focus or hit regions, or introduce translucent
composition. The [opaque surface IR](36-opaque-surface-ir.md), [focus
indicator](37-focus-indicator-ir.md), [surface paint](39-surface-paint-ir.md),
[bounded material light](85-bounded-material-light.md) and
[material-lit relief paint](86-material-relief-paint.md) keep their versions and
outputs. Component paint contracts adopt this body in their own versions.

## Geometry

For one part in physical part-local `px`:

- **F, the outline**, is the [surface outline](81-surface-outline.md) with
  `circular` curvature, built from the part's size and Tier 0 shape-fallback
  extents with the actual layout direction and `minimumInset` equal to `e`.
  Every physical corner MUST have equal extents after normalization; elliptical
  corners fail diagnostically.
- **e** is the family's exterior `edgeWidth`.
- **E** is the [closed conservative edge interior](83-opaque-relief-paint.md)
  of F at inset e, decided by its exact predicate.
- **D** is the final depth: semantic depth times the component's depth scale.
  It MUST be finite and nonnegative. **d** is the
  [key-light side offset](34-key-light.md) `-l*D`.
- **S**, the silhouette, is `F ⊕ [0,d]`, published as the
  [extruded contour](35-extruded-contour.md) of F.
- **E_d** is the swept edge interior: the spec/36 `edgeInterior` placed contour,
  the sweep of the inset outline `F(e)` translated by `(e,e)`.

The face carries the [bounded relief](82-surface-relief.md) over F. Its profile
is selected by part from authored data alone: neither state, material family
nor depth scale selects or scales it, so hover, pressed and disabled change D,
never the relief. A nonzero corner of radius r with shoulder width w requires the spec/82
`tau = 2*(w/r + w/r) < 1`; otherwise resolution fails with a diagnostic naming
the physical corner, the radius and the shoulder width. Component contracts
that select the profile by part prefix that diagnostic with the part name.
No shoulder clamp is applied.

## Region law

Every set is closed. Each point takes the first rule that applies:

1. **d = (0,0).** The body is the spec/86 face with its membership and
   classification verbatim: outside F is null, F outside E is the edge color,
   E takes face color.
2. **p ∈ F and p ∈ E.** Face color by the signed law below on the full relief
   normal at p.
3. **p ∈ F, p ∉ E and p ∈ E_d.** Face color by the same law. These front-ring
   points are covered by the sweep and stay front, as in spec/36.
4. **p ∈ F and p ∉ E_d.** The guarded exterior edge color.
5. **p ∉ F, p ∈ S and p ∈ E_d.** The exact side pigment.
6. **p ∉ F and p ∈ S.** The edge color.
7. **Otherwise** null.

F is tested first, so a point of F's boundary on the shadow side takes rule 3.
F and E use the exact spec/81 and spec/83 predicates; S and E_d use the
closed-set spec/36 contour membership. For d ≠ 0 the two kinds of predicate can
disagree only within the spec/36 arithmetic tie band near the boundary of E_d.
Raster evidence brackets those samples instead of treating either color as
exact. The highlight band, `highlightOuter`, `highlightWidth`
and the rounded `content` contour of spec/36 are not part of this body.

## Light

One key light, the appearance profile's `keyLight`, resolves both d and the
face light. There is no second key field. The light channels are one
unversioned named object with exactly `ambient` and `interaction`, using the
bounded material-light domains; absence of either channel is explicit null.
Component requests carry it as a required member and validate its original
source coefficients, including suppressed channels, under the bounded
material-light source rule.

The resolved body phase follows the existing precedence: `disabled`, then
`pressed`, then `hover`, otherwise `rest`. The interaction channel is admitted
exactly when that phase is `hover` or `pressed`. `focused` alone and `checked`
do not admit it; disabled always suppresses it. This is light admission, not
an inference of enablement, activation or focus, and it does not feed the
spec/86 `enabled` or `interactionActive` inputs. A keyboard press reaches the
pressed phase; with no pointer direction the product supplies null interaction
and no directional light appears. The supported states are `rest`, `hover`,
`pressed`, `disabled`, `focused` and `checked`.

The admitted channels then follow the bounded material-light environment
policy unchanged:

| Condition | Ambient | Interaction |
| --- | --- | --- |
| High contrast | 0 | none |
| Reduced motion, economy quality or no `dynamicLighting` | kept | none |
| Phase `rest` or `disabled` | kept | none |
| Reduced transparency | kept | kept |
| Balanced or full quality | kept | authored strength, unscaled |

Face color uses the signed bounded law exactly: `k = clamp(n.d)`,
`u = (1-t)*k + t*j`, `q = clamp((1-a)*u, -1, 1)`; positive q mixes toward
the highlight and negative q toward the side, and recessed faces invert through
their normals. Light never mirrors with layout direction. The side plane and the
exterior edge keep their exact colors under every light. As in bounded material
light, lighting MUST NOT be the sole state cue; component contracts keep
measuring state perceivability on the unlit body color and final depth.

The resolved body depends on no gradient, mask, shader or dynamic primitive.
Its face can be sampled into ordinary opaque pixels; only the documented
interaction suppression responds to capabilities.

## Guards and colors

The face keeps every spec/86 guard: the reserved content rectangle C inside E
and the uniform relief plateau is the exact body color for every admitted light,
boundary included; the foreground must meet the minimum content contrast
against the exact body; the edge candidate is selected against the supplied
adjacent color. Light never changes the body, edge or foreground. Component
contracts own label containment against C and disabled label dimming.

The three spec/86 ranges, body-to-highlight, body-to-side and the exact edge,
cover every color this body paints: face pixels lie in the first two, the side
pigment is the endpoint of the second, and the edge is exact.

## Part-keyed relief

The [control relief schema](../schemas/control-relief.schema.json) names one
complete relief profile for each of `commandBody`, `toggleTrack` and
`toggleThumb`; a material family does not select them. The
[reference profiles](../definitions/control-relief.json) are: Command body
convex, shoulder width 2 and amplitude 1; Toggle track recessed, shoulder
width 1.5 and amplitude 0.75; Toggle thumb convex, shoulder width 1 and
amplitude 0.5; each with nominal body thickness 2, which only validates the
recessed amplitude and has no paint effect. These are reference authoring
choices subject to visual acceptance, not universal values. Loading the
document applies the spec/82 profile rules to every part, including the
amplitude bound of half the shoulder width and a positive recessed thickness
that JSON Schema cannot express, and the diagnostic names the failing part.

## Published IR

The [body IR](../schemas/lit-control-body-ir.schema.json),
`lit-control-body-ir:0.1.0`, contains `schemaVersion`, `representation`
(`"opaque"`, which names opacity rather than geometry), `materialRole`,
`colorRole`, `materialFamily`, `form`, `states`, `frostRepresentation` for
Frost only, `face` (the complete spec/86 `paint` object), `lighting` (the key
light result at depth D without sampled normals), `depth` (D), `geometry` with
`silhouette` and `edgeInterior`, `foregroundRole` and `contentFallbackApplied`.
Roles are the control roles and families are Cast, Frost and Elastomer. The
[paint IR](../schemas/lit-control-paint-ir.schema.json),
`lit-control-paint-ir:0.1.0`, wraps one body with the spec/39 focus member,
present exactly when the body states include `focused`. Consumers distinguish
this body from the spec/36 body by its enclosing envelope.

Semantic invariants beyond structural validation: `face.light.keyDirection`
equals `lighting.direction`; interaction light appears only in the hover or
pressed phase; `lighting.sideOffset` is `-direction*depth`; the silhouette and
swept edge interior are the canonical spec/35 records, segments and bounds, of
F and `F(e)` swept by that offset, the latter placed at `(e,e)`; the outline is
circular with equal corner extents, every nonzero corner has `tau < 1` for the
face profile and the outline content reservation uses `minimumInset` e; C lies
in E and in the relief plateau; `face.contentContrastRatio` is the contrast of
`face.foreground` against the body; the third range is the exact edge color.
Derived numbers use absolute or relative tolerance 1e-12, whichever is larger;
the exact body, side and edge colors, null classification, structure and
versions remain exact.

## Proof

The [authored source](../conformance/ir/lit-control-body-source.json) and
[point vectors](../conformance/ir/lit-control-body-vectors.json), validated by
the [case schema](../schemas/lit-control-body-case.schema.json), cover every
region rule and its boundaries, including the shadow-side boundary of F, the
closed boundary of E and the content rectangle corners; every phase with light
admitted and suppressed; every environment condition of the policy; absent,
ambient-only and zero-strength channels; a steep key direction; Cast, Frost and
Elastomer; Command, Toggle track and Toggle thumb profiles; flat and square
bodies; asymmetric corners in both layout directions at the same physical
points; and diagnostic rejection of `tau >= 1`, elliptical corners, negative
depth, insufficient content contrast, a collapsed plateau, unsupported states,
Gel, non-control roles and a Frost representation or content fallback on the
wrong family.

The [independent arithmetic](../tools/lit_body_arithmetic.py) evaluates the
vectors without the reference: circular per-corner relief gates and normals,
exact rational F and E membership, sweep membership by intersecting the
parameter intervals of `{p - t*d : t in [0,1]}` with each convex constraint, the
signed law, guards, content reservation, the published IR invariants with
independently derived canonical swept contours, and supersampled pixels with
tie-bracketed bounds. Its
[tests](../tools/test_lit_body_arithmetic.py) also check normals against height
differences, E against an independently inset contour, sweeps against a dense
parameter search, its contours against the public circular extrusion vectors and depth-zero equivalence with the spec/86 rectangular
arithmetic. The Rust reference `LitControlBodyIr::sample_paint` reproduces the
vectors, equals the spec/86 result at depth zero under the stated mapping
(`enabled` true, `interactionActive` exactly for hover or pressed), equals the
spec/36 inset contour for E at depth zero and matches the
[published IR fixture](../conformance/ir/lit-control-body-ir.json).

This version publishes no command boundary of its own: component requests
carry the body in their own versions. Raster, native realization and accepted
Light/Dark scenes remain separate evidence before the appearance is accepted.
