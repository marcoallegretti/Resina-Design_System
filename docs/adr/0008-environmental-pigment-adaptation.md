# 0008 Environmental pigment adaptation

Status: accepted. Decided in [#138](https://github.com/marcoallegretti/Resina-Design_System/issues/138). The contract is in [environmental pigment adaptation](../../spec/95-environmental-pigment.md).

## Context

Resina's Light and Dark themes are authored with complete, environment-independent role bindings. The blueprint wants some influence of the surroundings on pigment, but rendering-time mixing in each backend would change authored meaning and readability without a shared contract. [Theme derivation](../../spec/91-theme-derivation.md) already turns an explicit input into a derived theme source and passes it through the [legibility guard](../../spec/23-authored-themes.md#legibility-guard). [Bounded background contrast](../../spec/72-bounded-background-contrast.md) bounds contrast over a range of backdrop colours.

## Decision

- **Theme resolution owns adaptation.** A consumer supplies an optional, finite observation: a representative opaque colour and, for Frost, a proved range of backdrop colours. It is supplied again when the environment changes, not streamed, so one observation gives one reproducible result.
- **Selection inside authored ranges.** The observation's Oklch chroma and hue are clamped into per-role ranges the theme author supplies, before gamut mapping. Lightness is never selected and roles are never rebound. A grey template role has no hue, so it takes the zero hue basis seed derivation uses for an achromatic hue.
- **Eligible roles.** Frost chrome tint and the three Cast surfaces. Accents belong to seed derivation, status roles are never adapted, and the light channels belong to [material light](../../spec/85-bounded-material-light.md).
- **No observation, high contrast or reduced transparency give the authored theme exactly.** An invalid observation is a diagnostic, including when adaptation is suppressed. A consumer that wants the authored theme omits the observation, and validating a sample before passing it is the consumer's duty. Silently ignoring a malformed sample under high contrast would hide a defect in the consumer that surfaces the next time the preference is off.
- **Same guard, plus a Frost bound.** The output is a derived theme source that compiles under [theme compilation](../../spec/22-theme-compilation.md) and passes the same 40-pair guard. Adapted Frost chrome is also certified over the observed range with the bounded contrast calculation, and the authored chrome stays when it is not certified.
- **Range instead of luminance interval.** The certification needs a box of sRGB channels because encoded-sRGB source-over mixes channels. A luminance interval alone cannot bound the composite, and it carries no pigment to select, so the observation's range is that channel box. It is the same bounded-backdrop input [translucent Frost](../../spec/94-translucent-frost.md) takes, and both certify with [bounded contrast](../../spec/72-bounded-background-contrast.md).

## Alternatives

- **Fixed themes with consumer-chosen palettes.** Reproducible and already possible, but every consumer would reimplement the mixing and its guards, and the result would not be portable.
- **Per-renderer blending.** It lets a backend decide readability and breaks equivalence between backends.
- **Continuous streaming.** It expands invalidation duties and makes output depend on sampling time.
- **Lightness adaptation.** It would move the tone pairs the guard was authored for, so most observations would fail or be repaired.
- **Repairing an adapted tint until it passes.** It hides authored contrast decisions inside the algorithm. Retaining the authored tint and reporting the bounds keeps the decision with the author.

Prior art: system acrylic and mica-style materials sample the surroundings once and fall back to a solid colour when transparency is off, and high-contrast modes show the user's colours.

## Consequences

- An adapted palette is reproducible from the observation, the profile, the templates and the foundation. The Rust reference and an independent Python implementation agree on the conformance corpus within `1e-9`.
- With the authored profile, a sweep of observation hue and chroma keeps every guard pair in both themes. The smallest margin is 2.1% above the minimum, so a profile that widens the ranges must be checked against the guard.
- Frost chrome adapts only for ranges the consumer can bound, and its nine pairs include the status colours the guard evaluates on chrome, so many ranges leave the authored chrome in place.
- Extracting or sampling observations stays outside Core.
