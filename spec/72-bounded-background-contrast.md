# Bounded opaque background contrast (candidate, 0.1.0)

This contract extends [opaque contrast](12-contrast.md) to a checked range of
possible background colors. Two concrete uses are a Slider thumb's edge and its
navigation ring beside directional track highlights and canvas. A single sampled
background or the two gradient endpoints cannot establish their complete contrast.
This is color mathematics, independent of any component, renderer or toolkit.

## Range and ownership

A range supplies explicit opaque sRGB lower and upper colors. Every encoded channel
of lower MUST be less than or equal to its upper channel. Alpha MUST be one for
both. Bounds are checked portable colors: invalid/nonfinite/out-of-gamut channels
cannot enter through unchecked data. Equal bounds describe a uniform background.
The [range schema](../schemas/opaque-srgb-range.schema.json) describes checked
output; channel ordering is additionally a semantic invariant.

The range is the closed Cartesian box of all channel combinations between those
bounds, not a claim that every enclosed color actually occurs. The producer MUST
prove that all relevant actual post-treatment background colors lie inside it.
The calculation cannot infer spatial coverage, clipping, blur, blend space or
background provenance. Failure to prove coverage is not permission to sample one
pixel or silently widen/repair a supplied bound.

Separate regions retain separate ranges. Do not combine disconnected dark and
light flat regions into one continuous range unless that conservative loss is
explicitly intended. For several ranges, one chosen foreground MUST pass the
minimum contrast across every range; independent foreground choices per region
do not certify a common edge or ring. An empty region set cannot certify anything.

## Contrast lower bound

Use the existing WCAG relative luminance and opaque ratio mathematics. The sRGB
transfer is monotone and the luminance weights are positive, so lower and upper
colors bound the luminance of every color in the box. With foreground luminance F
and background luminance bounds L and U, choose B = clamp(F, L, U). Return
`(max(F,B) + 0.05) / (min(F,B) + 0.05)`.

This is a conservative lower bound for contrast everywhere in the range. If F is
inside [L,U], return one even if neither endpoint has the foreground's luminance.
If F is below L or above U, the closest luminance endpoint defines the bound.
The operation does not choose a threshold, fallback color, or material response.
Threshold comparisons MUST remain unrounded and strict. A broad box may fail
although a narrower proved rendering domain would pass; it MUST NOT be treated
as a measured least-contrast pixel or an exact minimum of an arbitrary gradient.

For the existing encoded-sRGB opaque highlight law, every highlight channel lies
between the resolved body and full-lift highlight channels. Their ordered channel
box therefore bounds the directional highlight. Additional edge, side, canvas
and other adjacent regions still need their own ranges. Two arbitrarily ordered
gradient endpoints do not necessarily form valid lower/upper channel bounds.

## Evidence and limits

The [public cases](../conformance/color/contrast-range-cases.json) have a
[strict record schema](../schemas/contrast-range-cases.schema.json). They cover
interior luminance crossings, colored bounds, transfer branches, uniform ranges,
and explicit opacity/ordering errors. Rust uses a checked constructor and pure
calculation, with no unchecked deserialization protocol. Tests also bound a dense
set of independently computed actual colors and preserve existing uniform cases.

[WCAG non-text guidance](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)
requires evaluation against relevant adjacent colors and least-contrast gradient
areas. Its treatment of underlying authored colors rather than antialiased pixels
does not remove the need for rendered review of thin strokes. The
[relative luminance definition](https://www.w3.org/WAI/WCAG21/Understanding/relative-luminance.html)
supplies the transfer and weights; this range certificate is a derived Resina law,
not a W3C algorithm for arbitrary images. Native/composited/rendered coverage and
component paint remain separate required evidence.
