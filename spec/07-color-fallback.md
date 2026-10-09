# sRGB color fallback (candidate, schema 0.1.0)

Before a semantic color is used by a lower capability renderer, it needs an sRGB representation. This contract converts one validated [DTCG 2025.10 color value](https://www.designtokens.org/TR/2025.10/color/) into a [fallback value](../schemas/srgb-fallback.schema.json) with `colorSpace: "srgb"`, three numeric components in `[0, 1]`, and an explicit alpha in `[0, 1]`. It does not choose a renderer representation, composite transparency, derive material pigment, or guarantee contrast.

Resolved fallback records supplied to headless operations MUST be JSON objects
with explicit `colorSpace`, `components` and `alpha` members. Positional records
are invalid; the three numeric components remain a JSON array.

The source MUST first pass DTCG color-value validation. When `colorSpace` is `srgb` and all three components are numeric, the resolver MUST preserve those component values. Otherwise, if a six-digit `hex` fallback is present, the resolver MUST use it, decoding each byte to an sRGB component by dividing by 255. The source alpha MUST be preserved; absent alpha resolves to 1. The six-digit `hex` value does not encode alpha.

If `hex` is absent and `colorSpace` is `oklab` or `oklch` with three numeric components, the resolver MUST use the [color conversions](04-color-conversion.md) to encoded extended sRGB. Oklch first converts to Oklab. Each resulting sRGB channel MUST be in `[0, 1]`, allowing only a `10^-12` numerical tolerance outside the interval; a channel outside the interval but within that tolerance MUST be set to the nearest boundary. A result farther outside the interval is out of gamut and MUST fail rather than be gamut mapped or clipped. A nonfinite conversion result MUST fail. If any Oklab or Oklch component is `none`, resolution requires authored `hex`. An authored `hex` remains authoritative for non-sRGB colors, including Oklab and Oklch.

If `hex` is absent and `colorSpace` is `srgb-linear`, `hsl`, or `hwb` with three numeric components, the resolver MUST use their [direct sRGB conversion](04-color-conversion.md). The same `10^-12` boundary tolerance and nonfinite-result failure apply. These spaces are defined directly in sRGB, so valid inputs do not require perceptual gamut mapping. A `none` component still requires authored `hex`.

If `hex` is absent and `colorSpace` is `xyz-d65` or `xyz-d50` with three numeric components, the resolver MUST use the [pinned XYZ conversion](04-color-conversion.md), including D50-to-D65 adaptation where needed. It applies the same `10^-12` boundary tolerance. A result outside the sRGB gamut MUST fail; no automatic gamut mapping or clipping is implied. A `none` component requires authored `hex`.

If `hex` is absent and `colorSpace` is `display-p3`, `a98-rgb`, `prophoto-rgb`, or `rec2020` with three numeric components, the resolver MUST use the [pinned RGB conversion](04-color-conversion.md). It applies the same `10^-12` boundary tolerance and fails for out-of-gamut results. A `none` component requires authored `hex`. Authored `hex` remains authoritative for these spaces, including when the numeric source is out of gamut in sRGB.

If `hex` is absent and `colorSpace` is `lab` or `lch` with three numeric components, the resolver MUST use the [pinned Lab/LCH conversion](04-color-conversion.md), including D50-to-D65 adaptation. LCH first converts to Lab. The same `10^-12` boundary tolerance applies; an out-of-gamut or nonfinite result MUST fail. A `none` component requires authored `hex`. Authored `hex` remains authoritative for these spaces.

The resolver MUST fail when no complete portable fallback is available.

For an sRGB source with `hex`, each numeric source component MUST differ from its corresponding decoded byte by no more than `0.5 / 255 + 10^-12`; otherwise resolution fails. This allows nearest-byte quantization while rejecting an inconsistent authored fallback. Components marked `none` are supplied by `hex` and do not participate in that comparison. This rule does not compare non-sRGB source components to sRGB fallback bytes because perceptual gamut mapping is not uniquely specified by DTCG.

The [fallback vectors](../conformance/color/srgb-fallback-vectors.json) cover numeric sRGB, direct sRGB spaces, derived Oklab, Oklch, XYZ, Lab, LCH, and predefined RGB colors, missing components, alpha, authored hex, and failures. A theme or bundle claiming lower capability color support MUST provide a valid fallback for every semantic color role. The headless resolution operation validates every role in canonical role order and rejects the entire result if any fallback is unavailable. An [opaque color fallback](13-opaque-color-fallback.md) is a separate requirement when alpha compositing is unavailable or disallowed. The Rust color reference implements `resolve_srgb_fallback`; the resolver applies it to semantic roles through `resolve_semantic_color_fallbacks`. Neither requires a renderer.
