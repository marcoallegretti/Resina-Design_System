# Theme derivation from explicit seeds (candidate, schema 0.1.0)

A consumer can derive Light and Dark theme sources from one explicit sRGB seed. The [authored themes](23-authored-themes.md), or other complete theme sources, act as templates. The derivation replaces the accent, selection and focus roles and keeps every other role as authored. It runs in Oklch with a fixed gamut-mapping rule, and every derived theme must pass the [legibility guard](23-authored-themes.md#legibility-guard). The reasons for this model are recorded in [ADR 0007](../docs/adr/0007-dynamic-colour-model.md).

Core does not acquire seeds. Reading a wallpaper or image, extracting and quantizing candidates, scoring them and reading a platform accent all belong to consumers and backends. One platform seed source is the XDG Desktop Portal `org.freedesktop.appearance` `accent-color` key. Its value is an sRGB tuple, and a component outside `[0, 1]` means the key is unset. A consumer passes an accepted value through unchanged; there is no hidden default seed.

## Request

A [derivation request](../schemas/theme-derivation-request.schema.json) has exactly four members:

- `schemaVersion`: `0.1.0`;
- `seed`: exactly `colorSpace` `srgb` and `components`, three finite numbers in `[0, 1]`;
- `templates`: exactly `light` and `dark`, each the UTF-8 text of a complete [theme source](22-theme-compilation.md);
- `externalSources`: the URI-to-source-text map that compiles the templates, as in theme resolution.

Duplicate JSON members MUST be rejected at every depth before conversion to an object tree. This applies to the request and to each template's text. An absent, positional or out-of-range seed, a seed with other members, or a request with other members MUST fail with a diagnostic.

## Seed

The seed's Oklch coordinates come from the encoded sRGB to Oklab conversion in [color conversion](04-color-conversion.md). Chroma is `C = √(a² + b²)`, and hue is `atan2(b, a)` in degrees. The hue is normalized into `[0, 360)`: take the floating-point remainder by 360, add 360 when the remainder is negative, and use 0 when the sum rounds to 360. A seed with `C < 0.02` is achromatic. Its hue has no meaning, so it is reported as 0.

## Template roles

Each template MUST compile as specified in [theme compilation](22-theme-compilation.md) with the request's external sources. The derived roles are `accent.primary`, `accent.secondary`, `accent.tertiary`, `selection` and `focus`. Each MUST have an opaque [sRGB fallback](07-color-fallback.md), and none MAY appear in the template's [opaque color assignments](13-opaque-color-fallback.md). Otherwise an authored Tier 0 tone would no longer match the derived color. The template MUST NOT already define a token at or below the path `derivedColor`. A template that breaks any of these rules fails derivation for that theme.

Each derived role reads its template lightness `Lₜ`, chroma `Cₜ` and hue `hₜ` from its sRGB fallback by the seed rule above. These values are always computed from the template's authored colors and are never fixed constants. Status, content, surface, outline and chrome roles keep their template bindings.

## Targets

For a chromatic seed with hue `hₛ`, the target for each derived role is `[Lₜ, Cₜ, h]`:

- `accent.primary`, `selection` and `focus` take `h = hₛ`;
- `accent.secondary` and `accent.tertiary` keep their template hue offset from the primary accent, `h = norm(hₛ + hₜ − hₜ(accent.primary))`.

The function `norm` is the hue normalization above. For an achromatic seed, every target is `[Lₜ, 0, 0]`. Lightness and chroma always come from the template, so a seed changes hue but not tone or colorfulness. Seed chroma is used only to classify a seed as achromatic, and seed lightness is not used.

## Gamut mapping

Each target is mapped into sRGB using the binary search gamut mapping with local MINDE in [CSS Color Module Level 4, §14.2.1 and the §14.2.2 pseudocode](https://www.w3.org/TR/2026/CRD-css-color-4-20261020/#GMA-Binary-local-MINDE). The contract is pinned to the Candidate Recommendation Draft of 20 October 2026 and restated here so that later drafts cannot change it.

Let `toSrgb` convert Oklch to Oklab by the polar conversion and then to encoded extended sRGB by [color conversion](04-color-conversion.md). A color is in gamut when all three components are in the closed interval `[0, 1]`, with no tolerance. `clip` applies `toSrgb` and clamps each component to `[0, 1]`. `deltaEOK(s, c)` is the Euclidean distance between the Oklab conversion of sRGB `s` and the Oklab form of Oklch `c`. Let `JND = 0.02` and `ε = 0.0001`. For an origin `[L, C, h]`:

1. If `L ≥ 1`, return `[1, 1, 1]`. If `L ≤ 0`, return `[0, 0, 0]`. These are exact sRGB white and black; the CRD instead converts `oklab(1 0 0)` and `oklab(0 0 0)`, which differ from them only by rounding.
2. If `toSrgb(origin)` is in gamut, return it.
3. Set `current` to the origin and `clipped` to `clip(current)`. If `deltaEOK(clipped, current) < JND`, return `clipped`.
4. Set `min = 0`, `max = C`, and `minInGamut = true`.
5. While `max − min > ε`:
   1. set the chroma of `current` to `(min + max) / 2`;
   2. if `minInGamut` and `toSrgb(current)` is in gamut, set `min` to that chroma and continue;
   3. otherwise set `clipped = clip(current)` and `E = deltaEOK(clipped, current)`;
   4. if `E < JND` and `JND − E < ε`, return `clipped`;
   5. if `E < JND`, set `minInGamut = false` and `min` to that chroma;
   6. otherwise set `max` to that chroma.
6. Return `clipped`.

Every mapped component is in `[0, 1]`. Clipping a target without this search is not conforming. When the clipped color is at least one JND from the target, the search instead reduces chroma at the target's lightness and hue. Blue to violet seeds take the Dark focus target this far outside sRGB at the template's lightness and chroma.

Derived colors are numeric sRGB inside the gamut. [Color fallback](07-color-fallback.md) is therefore unchanged: it still rejects out-of-gamut Oklab and Oklch values instead of mapping them, and no out-of-gamut value reaches it from derivation.

## Derived sources

Let `group` contain one DTCG `color` token for each derived role, whose role name is split at `.` into token path segments. For example, `accent.primary` becomes `group.accent.primary`. Each `$value` is `{"colorSpace": "srgb", "components": [r, g, b]}` with the mapped components and no `hex` or `alpha`. The derived source is the template with exactly these changes:

- an embedded-token template gains the member `derivedColor: group` in `tokens`;
- a resolver-backed template gains `{"type": "set", "name": "derivedColor", "sources": [{"derivedColor": group}]}` at the end of `tokenResolver.resolutionOrder`;
- each derived role in `colorAssignments.roles` is bound to `derivedColor.<role>`, for example `derivedColor.accent.primary`.

The derived source MUST compile. Its opaque color fallbacks MUST then pass the legibility guard. A theme that fails any step publishes nothing.

## Result

A [derivation result](../schemas/theme-derivation-result.schema.json) has `schemaVersion` `0.1.0` and a `seed` record with its Oklch coordinates and `achromatic` flag. For both `light` and `dark` it publishes:

- `source`: the derived theme source;
- `roles`: each derived role's `targetOklch` before mapping, and its mapped `color`;
- `legibilityGuard`: all 40 pair results of the derived theme in guard order.

Derivation either publishes both themes or neither. A failing theme does not stop the other theme from being evaluated, so one diagnostic reports every failure. Each failing theme's part of the diagnostic begins on a line that starts with `light theme:` or `dark theme:`, and no partial result is written.

`resina-theme-derive <path|->` MUST read one UTF-8 request from a file or standard input and write exactly one result. Success exits 0 with no standard error. An invalid request or a failing theme exits 1 with a diagnostic and no standard output. Usage errors exit 2. The Rust command bounds request bytes before parsing. The Rust reference exposes `derive_theme_request_source` and `gamut_map_oklch_to_srgb`.

## Conformance

The [derivation vectors](../conformance/themes/derivation-vectors.json) follow the [case schema](../schemas/theme-derivation-case.schema.json). Each case builds a request from the authored Light and Dark templates and the authored foundation:

- `seed` becomes the request seed, and the request has no seed when the member is absent;
- `templateVariant: "embeddedTokens"` replaces each template's resolver with its composed token document;
- `templateChanges` replace values by RFC 6901 pointers in one template;
- `requestChanges` replace values by RFC 6901 pointers in the request, and `requestOmit` removes members;
- `requestDocument` is sent unchanged instead of a built request.

Valid cases record the expected seed, roles and guard results for both themes. `failingThemes` names the themes whose diagnostic lines must appear. Numbers MUST match within `1e-9` absolute. The corpus covers:

- black, white and grey seeds, and achromatic seeds on both sides of the chroma threshold;
- sRGB primaries and secondaries, and `[0, 1]` boundary components;
- yellow near white, and blue and violet near black;
- blue to violet hues that are out of gamut at the template lightness and chroma;
- hue offsets that wrap past 360 degrees, and a seed that reproduces the Light template hue;
- templates whose accent, selection and focus lightness, chroma and hue offsets differ from the authored ones;
- embedded-token templates;
- invalid seeds and request shapes;
- template compilation, opacity, opaque-assignment and reserved-group failures;
- guard failures in one theme and in both.

[`tools/theme_derivation.py`](../tools/theme_derivation.py) is an independent implementation of this contract for the checker's template subset: numeric sRGB tokens, aliases and inline or referenced resolver sets. The [external checker](../tools/check_theme_derivation_backend.py) recomputes every case with it and compares the result with the vectors. It then runs the backend twice per case and checks the result schema, component ranges, every guard pair, each derived source against its template, determinism, diagnostics and the absence of partial output. Passing establishes conformance only for the covered cases.

This contract does not adapt pigment to the environment, animate between palettes, choose a theme by identity or extract a seed. A derived theme is only as legible as the pairs the guard names. Rendered readability still requires the [contrast contract](12-contrast.md).
