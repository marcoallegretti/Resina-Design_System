# 0007 Dynamic colour model

Status: accepted. Decided in [#192](https://github.com/marcoallegretti/Resina-Design_System/issues/192). The contract is in [theme derivation](../../spec/91-theme-derivation.md).

## Context

Resina's Light and Dark themes are authored and fixed. The blueprint asks for themes derived from a user, application, system or wallpaper seed (§41). For the seed-to-palette step it suggests an "HCT-like strategy", with Oklab and Oklch used for interpolation (§42). The algorithm must be documented and conformance tested, not left to an implementation.

Resina already has the parts a derivation needs. It converts colors through Oklab and Oklch using a pinned CSS Color 4 sequence ([color conversion](../../spec/04-color-conversion.md)). It measures readability as the WCAG 2.2 contrast ratio over opaque fallbacks ([contrast](../../spec/12-contrast.md)). Its authored themes record that contrast for named role pairs ([authored themes](../../spec/23-authored-themes.md)). [Color fallback](../../spec/07-color-fallback.md) rejects out-of-gamut colors instead of mapping them, so a derivation must not produce any.

## Decision

- **Explicit seed.** The input is one sRGB seed with three finite components in `[0, 1]`. There is no default seed. Extracting, quantizing and scoring candidates from images, and reading platform accents, belong to consumers and backends. Core starts from the chosen seed.
- **Oklch instead of HCT.** The seed contributes its Oklch hue. Each derived role keeps the lightness and chroma of its template role, as Material's tonal spot keeps a fixed chroma per role. The secondary and tertiary accents keep the template's hue offsets from the primary accent. Seeds with chroma below 0.02 produce achromatic roles.
- **Explicit guards instead of tone heuristics.** HCT ties contrast to tone differences, and WCAG luminance only approximates that relation. Resina checks every named role pair against WCAG thresholds instead: the [legibility guard](../../spec/23-authored-themes.md#legibility-guard), which replaces an informal preflight. A theme that fails is reported and never adjusted until it passes.
- **Templates.** The authored themes are templates. Derivation replaces only the accents, selection and focus. Status, content, surface, outline and chrome roles stay authored, so a seed cannot weaken status meaning or neutral hierarchy. One seed produces both a Light and a Dark source, and either both are published or neither is.
- **Defined gamut mapping.** Targets outside sRGB are mapped by the CSS Color 4 binary search with local MINDE (JND 0.02, ε 0.0001, final clip). The rule is pinned to the Candidate Recommendation Draft of 20 October 2026 and restated in the specification. Color fallback stays unchanged because derived colors are already inside sRGB.

## Alternatives

- **HCT and Material Color Utilities.** These are mature, and Android and Material use them in production. Adopting them would add a second color appearance model alongside Oklab and a dependency on CAM16 viewing conditions. Contrast would still need a separate check, and every independent implementation would have to reproduce CAM16 to conform.
- **Seed lightness and chroma.** A dark or pale seed would move accents away from the lightness and chroma their pairings were authored for. Readability would then depend on the seed, and guard failures would be common.
- **Clipping.** Clipping is simpler, but it can move a color by more than one JND near the sRGB boundary. Blue to violet seeds do this for the Dark focus color at its template lightness and chroma.
- **Adjusting colors until they pass.** A repair loop would hide authored contrast decisions inside the algorithm and make the output depend on the search order. An explicit failure keeps the decision with the theme author.
- **Seed extraction in Core.** Image decoding, quantization and scoring depend on the source and the product. They would bring platform and image concerns into Core without changing the derivation contract.

## Consequences

- A derived palette is reproducible from the seed, the templates and the foundation alone. The Rust reference and an independent Python implementation agree on the conformance corpus within `1e-9`.
- In a sweep of seed hues at 0.1 degree steps with the authored templates, every guarded pair passed in both themes. Each pair that involves a derived role kept at least 1.28 times its threshold. A template that needs more contrast must be adjusted by its author, not by the derivation.
- Environmental pigment adaptation ([#138](https://github.com/marcoallegretti/Resina-Design_System/issues/138)) reuses the same guard. Platform seed sources, such as the portal accent color ([#200](https://github.com/marcoallegretti/Resina-Design_System/issues/200)), plug in without changing Core.
- Moving to HCT tones or to extracted seeds later needs a new decision that supersedes this record.
