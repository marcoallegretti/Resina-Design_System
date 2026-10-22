# Translucent Frost surface (candidate, 0.1.0)

The [translucent Frost decision](https://github.com/marcoallegretti/Resina-Design_System/issues/130)
makes translucent Frost a separately versioned appearance contract. It realizes
the three non-opaque Frost representations of [material fallback](31-fallback.md),
`shapedBackdrop`, `regularBackdrop` and `translucentPigmented`, for a chrome
surface: the body, side plane, highlight band and exterior edge of the
[opaque surface IR](36-opaque-surface-ir.md) and the navigation ring of the
[surface paint IR](39-surface-paint-ir.md). Its `opaqueDimensional` branch is
that opaque body and ring, unchanged. spec/31 remains the only selector of the
representation; this contract adds no capability flag, never inspects a backend
and never changes the environment to force a result. Frost controls adopt it in
their own contract versions.

## Inputs

The [request](../schemas/translucent-frost-request.schema.json) has the spec/36
members `schemaVersion`, `theme`, `surface`, `size`, `appearance`,
`foregroundRole`, `minimumContentContrast` and `minimumEdgeContrast`, with their
spec/36 meaning and strictness, and these others. Every member except
`surroundingRanges` is required; unknown or duplicate members, positional
records and unsupported versions fail.

- `backdrop` is an [opaque sRGB range](72-bounded-background-contrast.md): the
  closed channel box that encloses every color that can lie beneath the body
  after any optical treatment. A producer that cannot declare an interval
  declares black to white.
- `adjacentRanges` is a nonempty set of ranges beside the exterior edge and
  `surroundingRanges` a nonempty set around the navigation ring, both as in
  [common edge and focus contrast](73-common-background-contrast.md). Separate
  regions keep separate ranges. `surroundingRanges` is required exactly when the
  state set contains `focused`.
- `diffusion.radius` is the authored blur radius, a finite number of logical
  `px` greater than 0 and at most 65536.

The surface MUST be Frost, its states MUST be `rest`, `focused` or both and its
final treatment MUST be `none`, as in
[Frost surface readability](26-frost-surface-readability.md). The foreground
role MUST resolve to an opaque color. A range that is inverted, translucent,
not sRGB or has a channel outside `[0, 1]`, an empty or absent required range set,
a missing, nonpositive or larger-than-maximum radius and a content threshold
outside `[1, 21]` are errors. A range whose ends are equal is a valid interval
and declares one known backdrop color. None is repaired,
defaulted or replaced by a sampled color.

## Legibility does not depend on the radius

A blur is a positively weighted average of backdrop colors, so every color it
produces lies inside the channel box of the colors it averages. The tinted body
is therefore certified once, against the declared interval, and the result holds
for every radius and kernel. A producer MUST widen the declared
`backdrop` by any saturation or noise excursion its realization adds, so that
every color the body can lie over stays inside the box. The decision that
introduced this contract allowed a luminance-preserving saturation change; the
certificate is stricter because it bounds channels, and a change that preserves
relative luminance can still leave the box and with it the guarantee. The radius is authored intent. It is not checked by this headless
contract and is checked by a backend only where it controls the backdrop it
diffuses, that is, for an in-scene backdrop.

## Resolution

1. **Selection.** Apply spec/31 to the supplied environment. The representation
   is `shapedBackdrop`, `regularBackdrop`, `translucentPigmented` or
   `opaqueDimensional`.
2. **Body.** For the translucent representations the body is the
   [portable Frost body](31-fallback.md) of the surface color role, with alpha
   strictly between 0 and 1. For `opaqueDimensional` it is the authored opaque
   fallback.
3. **Certification.** Composite the body over the `backdrop` lower and upper
   colors with the encoded-sRGB source-over rule of
   [contrast](12-contrast.md). Each channel of the result is nondecreasing in the
   backdrop channel, so the two composites are ordered bounds of a range that
   encloses every composite over any backdrop color in the box. The published
   `contentContrastRatio` is the spec/72 lower bound of the foreground against
   that range. The body is retained when this unrounded ratio is at least
   `minimumContentContrast`.
4. **Legibility fallback.** Otherwise the opaque fallback is tested with the
   same foreground. If it meets the threshold the result is the opaque branch
   with reason `legibility`. If it does not, resolution fails with a diagnostic
   that reports both ratios and publishes nothing. The foreground is never
   recolored and no body is invented. With today's authored tints a black to
   white interval cannot certify 4.5:1 content, so such surfaces resolve to the
   opaque representation with this reason; changing that is a theme decision, not
   a relaxation of the guard.
5. **Selection precedes legibility.** When spec/31 selects `opaqueDimensional`
   the reason is `selection`, whatever the interval; the opaque ratio is checked
   as in spec/24.
6. **Edge and navigation.** The exterior edge is selected from `outline` and
   `outline.strong` against `adjacentRanges`, and the ring from `focus` and
   `outline.strong` against `surroundingRanges` at the existing 3:1 threshold,
   by spec/73. Failure at any step publishes no result, including when another
   channel could succeed.
7. **Geometry.** Front, silhouette, swept edge interior, highlight outline,
   content region, edge and highlight widths and key-light result are the
   spec/36 members for the same size, appearance, shape, depth and layout
   direction. They are identical in both branches, so the Frost edge, highlight
   and depth hierarchy holds at every size and scale and across the fallback.

## Opaque branch

The result carries one spec/39 `paint` object: the spec/36 body of the same
surface with the opaque pigment of the authored opaque fallback and, when
focused, the ring. Its edge and ring use the range-set selections above. Its
`contentFallbackApplied` is true exactly for reason `legibility`, as for a point
backdrop in spec/24. The ring's retained binding carries the actual
representation, `opaqueDimensional`, with the opaque body as its portable body,
as [spec/37](37-focus-indicator-ir.md) requires, so the body and the ring of one result
never disagree, including after a legibility fallback. Nothing is blurred or sampled in this branch and the `backdrop` interval does not
affect it.

## Translucent paint

The diffused backdrop is visible only inside the silhouette; the scene outside is
untouched. In physical part-local `px` the first rule that applies fixes the
paint of a point. Each point is painted once, so translucent colors never
composite over one another.

1. In the silhouette but outside the edge interior: the guarded opaque edge color.
2. In the edge interior and the front: inside the highlight outline and outside
   the content region, the [spec/36 highlight law](36-opaque-surface-ir.md)
   `c + (1 - c) × highlightLift × weight` on the body channels with the body
   alpha; otherwise the body.
3. In the edge interior but outside the front: the side pigment.

The `pigment` object publishes the body, the weight-one highlight and the side
as sRGB colors that share the body alpha, derived from the body channels by the
[opaque pigment law](29-opaque-pigment.md). Only the content region is certified;
side and highlight bands carry no content, and the highlight is not a legibility
claim. A backend MUST NOT place content over any other region without rerunning
contrast for that region.

`translucentPigmented` samples no backdrop and publishes no `diffusion`. The two
backdrop representations publish `diffusion.radius`. Alpha support alone neither
implies blur nor backdrop access: the capability combinations of spec/31 decide.

## Result

The [result](../schemas/translucent-frost-ir.schema.json) has `schemaVersion`,
the final `frostRepresentation` and `appearance`. Appearance `translucent` has a
`body` and, exactly when the body states contain `focused`, a spec/37 `focus`.
Appearance `opaque` has `fallbackReason` (`selection` or `legibility`) and the
spec/39 `paint`. The translucent `body` has the spec/36 identity, geometry,
lighting and edge members, `pigment`, `foreground`, `backdrop` (the supplied
`range` and the certified `compositedBody`), `contentContrastRatio` and, for the
backdrop representations, `diffusion`. Semantic invariants beyond structure: the
composited range is the composite of the supplied range ends; the ratio is the
spec/72 bound of the foreground against it and is at least the threshold; side
and highlight alpha equal body alpha; the opaque result's representation equals
the top-level one; the same state set, roles and swept silhouette appear in the
body and the ring. Derived numbers use absolute tolerance `1e-12`; structure,
roles, flags and enumerations are exact.

## Evidence

The [baseline request](../conformance/ir/translucent-frost-request.json) and
[cases](../conformance/ir/translucent-frost-cases.json), validated by the
[case schema](../schemas/translucent-frost-case.schema.json), cover every
spec/31 branch (shaped, regular without the shaped capability, regular below
full quality, pigmented translucency without blur, without a backdrop effect and
with economy quality, and the opaque selections for reduced transparency, high
contrast, unavailable translucent surfaces and a zero-capability renderer), the
legibility fallback for a dark and a black to white interval, a bright busy
interval, thresholds just below and just above the certified bound, selection
over legibility, several adjacent regions, ring fallback to `outline.strong`,
focused results in every branch, a point interval, a light foreground bound by the upper end, a foreground
inside the composited interval, and rejection of inverted (backdrop, adjacent and
surrounding), translucent, missing, positional, wide-gamut and out-of-gamut
ranges, an absurd radius, empty range sets, a focused surface
without surrounding ranges, nonpositive or malformed radii, unknown shaping
members, active treatments, non-base states, non-Frost surfaces, translucent
foregrounds, unreachable thresholds and indistinct edges or rings.

The [independent arithmetic](../tools/translucent_frost_arithmetic.py) derives
every case from the request without the reference: selection, tint, composites,
bounds, edge and ring selection and pigments, with geometry and light taken from
the committed [spec/36 vectors](../conformance/ir/opaque-surface-expected.json)
and [focus vectors](../conformance/ir/focus-ir-expected.json). Its
[tests](../tools/test_translucent_frost_backend.py) check selection against the
[spec/31 vectors](../conformance/materials/frost-fallback-vectors.json), the
bound against a grid of 343 backdrop colors spanning the interval box, with both
bounding ends and the interior crossing in separate vectors, and that the comparison detects a
changed decision member. The Rust reference additionally checks that geometry and
light equal the opaque surface at three sizes and three scales, and the strict
request shape. The external checker and the Rust tests also run the equivalent
[surface paint](39-surface-paint-ir.md) request, with point ranges, for both
reasons and for rest, focused and combined states, and require the opaque
`paint` to match it, apart from the ring's retained representation, which is the
actual one here.

`resina-translucent-frost <path|->` reads one UTF-8 request of at most 1 MiB.
Success exits 0 with one JSON result and no diagnostic; resolution or input
failure exits 1 with a diagnostic and no result; usage errors exit 2. The
[external checker](../tools/check_translucent_frost_backend.py) verifies the
protocol, schemas, exact results, failures and repetition.

## Limits

This contract resolves intent and certification; it neither acquires nor blurs a
backdrop and does not prove that a backend's pixels stay inside the declared
interval. The producer owns that coverage. How `shapedBackdrop` shapes its
response beyond the silhouette clip is a backend realization; it MUST stay inside
the interval. The Rust reference publishes the regions and colors of the paint law but, unlike
the opaque surface, has no point sampler for it, so the partition is not yet
exercised by a point oracle. Raster and native captures over declared backdrops, Frost controls,
other states and motion are separate evidence. Failure diagnostics follow the
[public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics).
