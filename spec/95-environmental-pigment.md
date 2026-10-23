# Environmental pigment adaptation (candidate, schema 0.1.0)

A consumer can supply one finite observation of the environment behind a surface. Theme resolution then selects the pigment of a few surface roles inside authored ranges and publishes a derived [theme source](22-theme-compilation.md). Roles are never rebound to other roles, and the [legibility guard](23-authored-themes.md#legibility-guard) still applies to the result. The reasons are recorded in [ADR 0008](../docs/adr/0008-environmental-pigment-adaptation.md).

Core does not acquire observations. Sampling a wallpaper or a backdrop, choosing a representative colour and proving the extent of a colour range belong to consumers and backends. The consumer supplies a new observation when the environment changes; it is not a per-frame stream, so one observation always gives one result.

## Request

A [request](../schemas/environmental-pigment-request.schema.json) has `schemaVersion` `0.1.0` and the members below. Only `observation` is optional.

- `profile`: the authored [profile](#profile);
- `observation`: the [observation](#observation);
- `accessibilityPreferences`: exactly boolean `reducedMotion`, `reducedTransparency` and `highContrast`, as in the [environment snapshot](18-environment.md);
- `templates`: exactly `light` and `dark`, each the UTF-8 text of a complete theme source. The [authored themes](23-authored-themes.md), or a source [derived from a seed](91-theme-derivation.md), are valid templates;
- `externalSources`: the URI-to-source-text map that compiles the templates, as in theme resolution.

Duplicate JSON members MUST be rejected at every depth before conversion to an object tree, in the request and in each template's text. Unknown, missing or positional members, numeric overflow and unsupported versions MUST fail with a diagnostic.

## Profile

A [profile](../schemas/environmental-pigment-profile.schema.json) holds, for `light` and `dark`, a `hue` range and a `chroma` range for each eligible role. The eligible roles are `surface.base`, `surface.low`, `surface.high` and `surface.chrome`. These are the Cast surfaces whose warmth the environment may tint, and the Frost chrome tint. Every other role is never adapted: accents, selection and focus belong to [seed derivation](91-theme-derivation.md), and status, content and outline roles keep their authored colours. The profile names no role outside the four, and it supplies every one.

- A `hue` range is a pair of offsets in degrees from the template hue, with `min` in `[-180, 0]` and `max` in `[0, 180]`. It therefore always contains the authored hue.
- A `chroma` range is a pair of Oklch chromas in `[0, 0.4]` with `min ≤ max`. A theme MUST fail when a role's authored chroma lies outside its range, so the range always contains the authored colour.

The [authored profile](../definitions/environmental-pigment-profile.json) gives each authored theme a hue range of ±60° for the Cast surfaces and ±45° for Frost chrome. It limits Light chroma to 0.016 (0.012 for `surface.high`) and Dark chroma to 0.04. With these ranges every sampled observation hue and chroma keeps all 40 guard pairs in both themes (the smallest margin is Light `status.warning` over `surface.low`, 2.1% above its minimum), but a different profile is only as safe as the guard that evaluates it.

## Observation

An observation has a `color` and an optional `range`.

- `color` is exactly `colorSpace` `srgb` and three finite `components` in `[0, 1]`: one representative opaque colour. It has no `alpha`.
- `range` is exactly `lower` and `upper`, two such colours that satisfy the [opaque range](72-bounded-background-contrast.md) rules: every channel of `lower` is at most the same channel of `upper`. The consumer MUST prove that every backdrop colour that can occur behind the Frost chrome lies inside the box. Resina does not infer coverage.

`color` MUST lie inside `range` in every channel when both are present. An observation that breaks any rule is invalid and MUST fail with a diagnostic. It is never repaired, clamped or ignored, whatever the accessibility preferences are. An observation without a `color` carries no pigment to select, so a luminance interval alone is not an observation. A consumer that wants the authored theme omits the observation, and that is the only override: an invalid observation is not a way to ask for it, and a consumer MUST NOT pass a sample it has not validated.

The `range` is the same bounded-backdrop input that [translucent Frost](94-translucent-frost.md) takes, and certification below uses the same [bounded contrast](72-bounded-background-contrast.md) calculation.

The colour's Oklch coordinates come from the encoded sRGB to Oklab conversion in [color conversion](04-color-conversion.md), with chroma `C = √(a² + b²)` and the hue normalization of [theme derivation](91-theme-derivation.md#seed). A colour with `C < 0.02` is achromatic and its hue is reported as 0.

## When adaptation applies

Adaptation applies only when the request carries an observation and neither `highContrast` nor `reducedTransparency` is true. Otherwise the result is the authored theme exactly, and `adaptation.reason` is the first of these that holds:

1. `highContrast`: the user's colours are shown, so nothing is tinted;
2. `reducedTransparency`: a surface that cannot be translucent keeps its authored pigment;
3. `noObservation`.

`reducedMotion` has no effect. There is no hidden default observation. A request without an observation is valid, and its result publishes the template as `source`.

## Selection

For each theme and eligible role, let `[Lₜ, Cₜ, hₜ]` be the Oklch coordinates of the template's sRGB fallback for that role, and `[_, Cₒ, hₒ]` those of the observed colour. When `Cₜ < 0.0001` the template role is grey: its hue is floating-point noise, so `hₜ` is taken as 0 for the whole selection. The target is `[Lₜ, C, h]`:

- `C` is `Cₒ` clamped to the role's chroma range;
- for a chromatic observation, `h = norm(hₜ + δ)`, where `δ` is `hₒ − hₜ` taken modulo 360 into `(-180, 180]` and then clamped to the role's hue range. For an achromatic observation, `h = hₜ`.

`norm` is the hue normalization of seed derivation. Lightness always comes from the template, so an observation selects hue and chroma and never lightness. A range bound is the most any observation can move the target before mapping. The target is mapped into sRGB by the [gamut mapping](91-theme-derivation.md#gamut-mapping) of seed derivation, which can move the published colour off the target. What is guaranteed is that the published colour is exactly that mapping of the target and that its Oklch lightness differs from the target's by at most the mapping's `JND` of 0.02; hue and chroma of the published colour are not bounded by the ranges.

The zero hue basis for a grey template role follows seed derivation, which also reports an achromatic hue as 0. The threshold is far below the `0.02` used for seeds because the authored Cast surfaces are deliberately tinted at low chroma and their hue is meaningful: noise in a grey is about `1e-15`, and the smallest chroma that one 8-bit channel step produces is about `1e-3`. Like the achromatic threshold of seed derivation, it makes the selected hue discontinuous at the threshold. At an exact antipode, where `hₒ − hₜ` is 180° up to rounding, the sign of `δ` can differ between implementations; the clamp makes this inherent.

The three Cast roles adapt as soon as adaptation applies. They MUST be opaque and MUST NOT appear in the template's [opaque color assignments](13-opaque-color-fallback.md), because their Tier 0 colour is their own fallback. The Frost chrome role MUST be translucent and MUST have an opaque assignment, which stays authored. A template that breaks these rules, or that already defines a token at or below `adaptedColor`, fails for that theme.

## Frost certification

Frost chrome adapts only when the observation has a `range` and the adapted tint is certified over it.

Let `Cs` be the adapted chrome colour with the template's alpha `αs`. The encoded-sRGB source-over of [contrast](12-contrast.md) places it over the `lower` bound and over the `upper` bound. Source-over is monotone in the backdrop channel, so the two results are the bounds of an [opaque range](72-bounded-background-contrast.md) that contains the composite of every backdrop colour in the box. For each [guard pair](23-authored-themes.md#legibility-guard) whose background is `surface.chrome`, the [lower bound on contrast](72-bounded-background-contrast.md#contrast-lower-bound) of the foreground's opaque fallback over that range is compared with the pair's minimum, unrounded and with no tolerance. The nine pairs are `content.primary`, `content.secondary`, `content.muted`, `status.error`, `status.warning`, `status.success` and `status.info` at 4.5, then `outline` and `focus` at 3.

If every pair passes, the adapted chrome is published. Otherwise the authored chrome stays, and the bounds are still published in `frostCertification`, so a consumer can see why. Nothing is repaired. Certification covers the adapted tint only: it does not claim that the authored chrome passes over the range, and it is not a measurement of any rendered pixel. Passing establishes the bound for the supplied box and nothing about backdrops outside it.

## Derived source

Let `group` contain one DTCG `color` token for each adapted role. Cast roles have `colorSpace` `srgb` and the mapped `components`. The Frost token additionally has the template `alpha`. None has `hex`. The adapted source is the template with exactly these changes, in the shape of [seed derivation](91-theme-derivation.md#derived-sources):

- an embedded-token template gains `adaptedColor: group` in `tokens`;
- a resolver-backed template gains `{"type": "set", "name": "adaptedColor", "sources": [{"adaptedColor": group}]}` at the end of `tokenResolver.resolutionOrder`;
- each adapted role in `colorAssignments.roles` is bound to `adaptedColor.<role>`.

When adaptation does not apply, the source is the template. The adapted source MUST compile. Its opaque colour fallbacks MUST then pass the legibility guard, and so MUST the authored source when nothing is adapted. A theme that fails either step publishes nothing.

## Result

A [result](../schemas/environmental-pigment-result.schema.json) has `schemaVersion` `0.1.0` and an `adaptation` record: `applied` `true` with the `observation` Oklch coordinates and `achromatic` flag, or `applied` `false` with the `reason`. For both `light` and `dark` it publishes:

- `source`: the theme source;
- `roles`: each adapted role's `targetOklch` before mapping and its mapped `color`. It is empty when `applied` is `false`, and its Cast roles are all present when `applied` is `true`;
- `frostCertification`: the nine bounds in guard order when a `range` was certified, each with `foreground`, `minimumContrast`, `contrastBound` and `passed`;
- `legibilityGuard`: all 40 pair results of the published theme in guard order.

Adaptation either publishes both themes or neither. A failing theme does not stop the other theme from being evaluated, so one diagnostic reports every failure. A diagnostic about a theme begins on a line that starts with `light theme:` or `dark theme:`. A request-level error is reported without a theme name. No partial result is written.

`resina-pigment-adapt <path|->` MUST read one UTF-8 request from a file or standard input and write exactly one result. Success exits 0 with no standard error. An invalid request or a failing theme exits 1 with a diagnostic and no standard output. Usage errors exit 2. The Rust command bounds request bytes before parsing. The Rust reference exposes `adapt_pigment_request_source`.

## Conformance

The [adaptation vectors](../conformance/themes/environmental-pigment-vectors.json) follow the [case schema](../schemas/environmental-pigment-case.schema.json). Each case builds a request from the authored Light and Dark templates and the authored profile:

- `observation` becomes the request observation, and the request has none when the member is absent;
- `accessibilityPreferences` replaces the default preferences, which are all `false`;
- `templateVariant: "embeddedTokens"` replaces each template's resolver with its composed token document;
- `templateChanges` replace values by RFC 6901 pointers in one template;
- `requestChanges` replace values by RFC 6901 pointers in the request, and `requestOmit` removes members;
- `requestDocument` is sent unchanged instead of a built request.

Valid cases record the expected adaptation, roles, certification and guard results for both themes. `failingThemes` names the themes whose diagnostic lines must appear. Numbers MUST match within `1e-9` absolute. The corpus covers:

- no observation, and an observation under each accessibility preference, with the precedence of the reasons;
- grey template roles in both themes;
- bright, dark, warm, grey, black, white, high-chroma and low-contrast backdrops, achromatic observations on both sides of the chroma threshold, and hues around the circle;
- observations with and without a range, and ranges that certify Frost in one theme and not the other;
- embedded-token templates;
- observation components outside `[0, 1]` or huge, wrongly typed or missing members, and an `alpha` member;
- inverted ranges, a colour outside its range, and profile ranges that are malformed or exclude the authored colour;
- template opacity, opaque-assignment and reserved-group failures;
- an adaptation that breaks the guard in one theme, authored themes that fail the guard, and failures in both themes.

[`tools/environmental_pigment.py`](../tools/environmental_pigment.py) is an independent implementation of this contract for the same template subset as [seed derivation](91-theme-derivation.md#conformance). The [external checker](../tools/check_environmental_pigment_backend.py) recomputes every case with it and compares the result with the vectors. It then runs the backend twice per case and checks the result schema, the eligible roles, the authored chroma and hue limits of each target, that each published colour is the gamut mapping of its target with lightness inside the `JND` allowance, the template alpha, the certification, the derived source against its template, every guard pair, determinism, diagnostics and the absence of partial output. The unit tests also sweep observation hue and chroma against the authored profile. Passing establishes conformance only for the covered cases.

This contract does not acquire an observation, extract a wallpaper colour, animate between palettes, change a role's binding, adapt an accent or a status colour, or modulate the [light channels](85-bounded-material-light.md). The guard names the pairs it checks, and rendered readability still requires the [contrast contract](12-contrast.md).
