# Elevation shadow (candidate, 0.1.0)

The [elevation shadow decision](https://github.com/marcoallegretti/Resina-Design_System/issues/209)
separates raised and floating opaque surfaces from what lies beneath them. The
shadow follows the one shared [key light](34-key-light.md) and the surface's
final depth. It is drawn at every tier: Tier 0 paints an opaque contact band, and
`advancedShadow` adds an outward penumbra. This headless operation resolves the
shadow geometry, pigment and guards. It does not paint, acquire a backdrop or
change body appearance.

## Inputs

The [request](../schemas/elevation-shadow-request.schema.json) requires version
`0.1.0`, a complete environment snapshot, a [key-light definition](../schemas/key-light.schema.json),
complete [shadow profiles](../schemas/shadow-profiles.schema.json), the semantic
`elevation` role, its resolved `elevationDepth` in logical `px`, the state
`depthScale`, the front `size` and logical `radii`, the actual opaque
`adjacentColor` beneath the shadow, the exterior `edge` candidates
(`outline`, `outlineStrong`, `minimumContrast`), an explicit `focus` (null, or a
focus `color` and `minimumContrast`) and ordered `distanceSamples`.

The key direction is an explicit input. It is the same light that resolves the
body's side plane; the operation does not read a reference definition. The
caller supplies the role's elevation depth from its [depth assignment](08-elevation.md)
and `depthScale` from the body's resolved state response ([Command paint](43-command-paint.md)).
The final depth is `D = elevationDepth × depthScale`. `elevationDepth` MUST be
finite and nonnegative; `depthScale` MUST be finite and in [0, 2].

The profile document supplies, for each of the six elevation roles, a
`shadowShade` in [0, 1) and a finite nonnegative `contactMargin` in logical
`px`. All roles are validated,
including unselected ones. Shade one is invalid: it replaces every adjacent
color with black. The [Light](../definitions/shadow-light.json) and
[Dark](../definitions/shadow-dark.json) profiles are reference authoring.

The request and every named record, including the profile collection, each role
profile, the edge, the focus and every color, MUST be JSON objects; positional
arrays are invalid. Missing, unknown, duplicate, nonfinite and out-of-range
members, unsupported versions, a zero key direction, invalid radii and a
translucent adjacent, edge or focus color MUST fail without a partial result.

## Geometry

Let `F` be the bounded front shape, with logical corners mapped to physical
corners by the environment's layout direction as in the
[extruded contour](35-extruded-contour.md). Let `l` be the normalized key
direction from spec/34 and `c` the selected role's contact margin.

```text
d = D·(−l)                    side offset (spec/34)
S = F ⊕ [0, d]                silhouette (spec/35)
v = (2D + c)·(−l)             shadow sweep
H = F ⊕ [0, v]                shadow hull
B = H \ S                     contact band
```

`H` is an ordinary extruded contour: its record is the spec/35 canonical
contour of `F` with offset `v`. Its support function satisfies
`h_H(n) = h_F(n) + max(0, n·v)` for every physical direction `n`, which is
independent conformance evidence. Beyond the silhouette, the band reaches
`D + c` along `−l`. On a straight face with outward normal `n` its thickness is
`max(0, n·(D + c)(−l))`; the result publishes these four values. With the
calibrated direction (−0.34, −0.94) the band lies below and to the right of the
body.

Neither `l` nor `v` mirrors in RTL. Layout direction changes only the logical
to physical corner mapping of `F`. A symmetric shape therefore has the same
shadow in both directions. An asymmetric shape in RTL casts the shadow of its
RTL front along the same physical sweep, which is not the mirror image of its
LTR shadow.

A shadow exists exactly when `D > 0` and `F` has positive width and height. A
body flattened to `D = 0`, for example a disabled body, casts none, even with a
positive contact margin. An empty front casts none. Overflow of the final depth,
sweep or paint bounds MUST fail.

## Pigment

For each encoded sRGB channel `a` of the adjacent color, the band color is
`a × (1 − shadowShade)`, with alpha one. This is the [opaque pigment](29-opaque-pigment.md)
shade law applied to the actual neighbor, and equals source-over of black at
alpha `shadowShade` in encoded sRGB. A backend may composite it that way only
over that known opaque color. A backdrop with several opaque regions is
evaluated once per region; the geometry is the same for each.

The result publishes ΔE_OK, the Euclidean [Oklab](04-color-conversion.md)
difference between band and adjacent colors. A difference of at least 0.04, the
[state perceivability](43-command-paint.md) threshold, is an authoring and
calibration requirement on a theme's own surfaces, not a resolution failure.
Where the adjacent color cannot darken, as on pure black, the band equals it,
ΔE_OK is zero and the guarded edge still separates the body.

## Penumbra

The penumbra width is `w = D/2` when the renderer declares `advancedShadow`
and high contrast is off; otherwise `w = 0`. The factor one half is the recorded
calibration and a constant of this law for every role and theme, not an
authored profile member. For a point outside the silhouette, the shadow weight
is one inside `H`. At Euclidean distance `t` outside `H` it is `1 − t/w` for
`0 < t < w` and zero for `t ≥ w`. The painted color is
`a × (1 − shadowShade × weight)`. The penumbra is therefore outward-only and
monotone, and lies between the band and adjacent colors. With `w = 0` it is
absent and the exact band remains: Tier 0 is a strict subset of the enhanced
appearance. The penumbra also surrounds the lit sides of `F` by at most `w`.

A blur centered on the hull edge, such as a Gaussian applied to `H`, is not
conformant, because it puts less than full weight inside the band and changes
the guarded Tier 0 color. Backends realize the hull with geometry or an exact
shader, and may subdivide the penumbra into opaque fills to their documented
rasterization quality. `gradients` is not required.

`distanceSamples` are signed distances from the boundary of `H`, negative inside
it, for points outside the silhouette. Each produces one ordered `{weight, color}`
by this law; without a shadow every weight is zero and every color is the
adjacent color. Samples are a pigment oracle, not geometry: they do not
establish where `S` ends.

## Guards

The colors the shadow can place next to the body lie in the channel box from the
band color to the adjacent color. Every penumbra weight in [0, 1] stays in that
box under the reference arithmetic, because each step is monotone. The result
publishes this `colorRange`; without a shadow the range is the uniform adjacent
color.

The exterior edge is selected over that range with the [bounded background
contrast](72-bounded-background-contrast.md) lower bound: retain `outline` when
it meets `minimumContrast`, otherwise select `outline.strong`, otherwise fail.
A focused body evaluates its focus color the same way, falling back to
`outline.strong`: with a 4.8:1 focus minimum beside the Light `surface.base`
band, `focus` reaches 4.692 and `outline.strong` 4.972, so the focus guard
selects `outline.strong`. Both tiers use the same range, so one guard result
bounds the band and the penumbra. The shadow never weakens an edge or focus guard. Where
the plain outline cannot hold beside the band, the existing fallback applies:
in the Light theme on `surface.low` the edge selects `outline.strong`.

The band and penumbra lie outside the silhouette, so the body's content region
is untouched. Content that belongs to the backdrop or to a sibling MUST NOT lie
inside `H` or within `w` of it unless its contrast is re-evaluated against the
actual shadow color.

## Interaction and paint bounds

Shadow paint is not interactive. It is excluded from hit regions and target
reservation, so the reserved target of [Command snapshot](49-command-snapshot.md)
is unchanged. It is included in paint bounds and neighbor keep-out: the result
publishes `paintBounds`, the hull bounds expanded by `w` on every side. Painting
fills `H` and the penumbra beneath the body, then paints the body over `S`.

## Scope

Reduced transparency does not change this opaque operation; its result equals
the standard result. Quality policy, reduced motion and the other environment
members do not change it either. Ambient (omnidirectional) shadow, inner shadow
for wells and `innerShadow`, shadow transmission through translucent bodies,
and specular, wrap and subsurface terms are outside this version. Integrating
the band into the [opaque surface IR](36-opaque-surface-ir.md) and the
[lit control body](88-lit-control-body.md), and re-authoring the reference key
light, are subsequent changes.

## Reference profiles

The reference profiles use `shadowShade` 0.062 in Light and 0.40 in Dark for
every role and contact margin zero. Roles differ by band
extent, which grows with depth. In Light the window between perceivability on
`selection` and the outline guard on the opaque chrome leaves no room for a
per-role ladder. In Dark the outline is lighter than every surface, so a darker
band only raises edge and focus contrast.

With the calibrated direction, a 3:1 edge and focus minimum, on the authored
opaque surfaces:

| Theme | Adjacent | Band | ΔE_OK | Edge | Ratio |
| --- | --- | --- | --- | --- | --- |
| Light | `surface.base` | `#e2e1dc` | 0.0450 | `outline` | 3.147 |
| Light | `surface.low` | `#d5d9d5` | 0.0434 | `outline.strong` | 4.545 |
| Light | `surface.high` | `#ecebe9` | 0.0467 | `outline` | 3.466 |
| Light | `surface.chrome` (opaque) | `#d9ddda` | 0.0442 | `outline` | 3.005 |
| Light | `selection` | `#b3cecb` | 0.0409 | `outline.strong` | 3.914 |
| Dark | `surface.base` | `#0b1317` | 0.0537 | `outline` | 6.963 |
| Dark | `surface.low` | `#080e12` | 0.0417 | `outline` | 7.543 |
| Dark | `surface.high` | `#172327` | 0.0892 | `outline` | 4.977 |
| Dark | `surface.chrome` (opaque) | `#152227` | 0.0875 | `outline` | 5.077 |
| Dark | `selection` | `#1b353a` | 0.1252 | `outline` | 3.247 |

Light `selection` already selects `outline.strong` without a shadow.

## Public boundary and evidence

`resina-elevation-shadow <path|->` reads one UTF-8 request up to 1 MiB. Success
exits 0 with one [result](../schemas/elevation-shadow-result.schema.json) and no
diagnostic; invalid input or a failed guard exits 1 with a diagnostic and no
result; usage errors exit 2. Duplicate JSON members are rejected at every depth.
The result records the role, normalized key direction, final depth, side
offset, selected profile, the shadow (null when absent), the edge and focus
guards and the samples.

Geometry members are lengths in logical `px`: `finalDepth`, `sideOffset`,
`contactMargin` and, in the shadow, `sweep`, `bandThickness`, `penumbraWidth`,
`paintBounds` and the hull coordinates, radii and bounds. They use absolute
tolerance `1e-12` or relative tolerance `1e-12`, whichever is larger. All other
numbers use absolute tolerance `1e-12` only: `keyDirection`, `shadowShade`, the
hull's unit radial parameters, every color component, `colorRange`, contrast
ratios, ΔE_OK and sample weights. The hull's segment count, order and kind follow
the spec/35 canonical record exactly; structure, roles and flags are exact.

[Vectors](../conformance/elevation/shadow-vectors.json), validated by the
[case schema](../schemas/elevation-shadow-case.schema.json), cover every role at
depthScale 0, 0.5, 1, 1.55, 1.6, 1.75 and 2 in both themes, the authored
surfaces, white and black neighbors, LTR and RTL with symmetric and asymmetric
shapes, zero depth with and without a contact margin, an empty front,
`advancedShadow` on and off, high contrast, reduced transparency, other key
directions, the focus fallback, invalid input, and edge and focus guard
failures that occur only because of the band, with a no-shadow control in which
the same minimum holds. Raster evidence at several scales, opaque surface
integration and native captures are subsequent evidence; this headless
operation alone does not establish them.
