# Opaque command paint (candidate, schema 0.1.0)

This contract resolves the body and independent navigation paint of an ordinary command surface. It builds on [state composition](16-state-composition.md), [opaque surface appearance](36-opaque-surface-ir.md), and [complete surface paint](39-surface-paint-ir.md). The operation is headless and renderer agnostic. It defines static endpoints; it does not define a transition trajectory or a complete Button component.

## Scope and state channels

A request MUST use a `control.passive`, `control.interactive`, or `control.primary` material role, bound through the actual theme and environment. Cast, Frost and Elastomer are valid command families. Gel MUST NOT be used. Supported signals are `rest`, `hover`, `pressed`, `disabled`, and `focused`, in any valid nonempty combination. Every other signal MUST fail explicitly until its component appearance contract exists.

The body phase is `disabled` when disabled is present, otherwise `pressed` when pressed is present, otherwise `hover` when hover is present, otherwise `rest`. This is precedence for this command body's pigment and depth channels, not a change to the state set or a general state precedence rule. All input signals MUST remain in the body IR and any navigation binding. `focused` independently requires a visible focus indicator, including when pressed or disabled is also present. State selection MUST NOT infer enablement, focus eligibility, activation, selection, or a semantic action. The [activation contract](42-command-activation.md) owns gesture lifecycle.

## Authored responses

The appearance document, its material-family collection, each family's phase collection, and every response MUST be JSON objects with named members. Positional arrays are invalid. The [appearance schema](../schemas/command-appearance.schema.json) requires explicit hover, pressed and disabled responses for each of Cast, Frost and Elastomer. Every profile MUST be validated, including families and phases not selected by this request. There are no backend defaults. Rest is the identity response: `bodyMix = 0` and `depthScale = 1`.

`bodyMix` MUST be finite and in [-1, 1]. For an encoded sRGB body channel `c`, a negative mix `m` resolves to `c * (1 + m)`; a nonnegative mix resolves to `c + (1 - c) * m`. Alpha is preserved. This uses the same encoded sRGB shade/lift convention as the existing opaque pigment contract. The response applies to both the opaque fallback body and Frost's actual portable body before legibility selection. Foreground and semantic color role remain unchanged. Final content contrast MUST be calculated against the actual resolved body, never the unmodified rest body. Insufficient contrast MUST fail; the operation MUST NOT silently reduce the authored response.

`depthScale` MUST be finite and in [0, 2]. Final depth is the theme-bound semantic elevation depth multiplied by this scale. The front contour and content footprint remain stable; only the dimensional extrusion changes. A scale below 1 compresses the extrusion and a scale above 1 raises it. This is a depth change, not in-plane elastic deformation. Body silhouette, edge interior and focus silhouette MUST all use this same final depth. The semantic elevation remains available as intent in the IR.

### Perceivable state responses

Each authored hover, pressed and disabled response of the bound family MUST be perceivable from rest through at least one measurable channel:

- **body color:** the Euclidean Oklab difference ΔE_OK between the resolved opaque body with the response applied and the same bound opaque body at rest is at least 0.04; or
- **depth:** the final extrusion depth differs from the rest depth (the semantic elevation depth) by at least 1 logical `px`. Compare with an absolute geometric tolerance of `1e-12` logical `px`, as in [focus geometry](37-focus-indicator-ir.md).

Measure on the opaque body the response is applied to, using the [Oklab conversion](04-color-conversion.md) of its encoded sRGB channels. For Frost this is its opaque fallback body, because this operation only publishes an `opaqueDimensional` representation. Rest is the identity response and is not measured. An embedded body with zero semantic depth can only satisfy the color channel. Resolving any state, including rest, validates all three responses of the bound family on the body being resolved, so an imperceptible profile fails at its first resolution rather than at the first hover or disablement. When a response satisfies neither channel, resolution MUST fail with a diagnostic naming its phase and publish no partial IR. The operation MUST NOT strengthen, replace or omit an authored response to pass. Only authored responses are measured; sampled motion responses between endpoints are not.

The 0.04 threshold is twice the commonly cited informal ΔE_OK just-noticeable difference of about 0.02. That figure is not a standard and depends on the display; the factor of two leaves margin for display variation. The 1 `px` depth threshold is one logical pixel of physical extrusion change. Both channels keep foreground and semantic color role unchanged, so existing content and edge contrast guarantees remain. On near-white light-theme bodies a lift cannot reach the color threshold; such responses darken or rely on depth.

The [light](../definitions/command-appearance-light.json) and [dark](../definitions/command-appearance-dark.json) definitions are reference authoring, not universal numeric material constants. Hover raises the body, pressed compresses it and disabled flattens it. Cast keeps the most restrained rise and compression and Elastomer the strongest, following the blueprint's qualitative family distinction. Every family's body mix stays within the range that keeps an embedded Toggle track perceivable and its thumb edge guard satisfied in each theme. These values do not calibrate spring mechanics or promise physical simulation. Products and themes may supply different validated responses that satisfy perceivability, contrast and material identity requirements.

## Resolution and fallback

Resolve the actual environment and theme once. Bind the original surface and preserve its full state set. Validate that every authored response of the bound family is perceivable from rest, apply the selected authored body response, then run the existing content and adjacent-edge guards. Frost MUST retain actual capability, preference and legibility selection. An opaque output is valid only when the resulting Frost representation is genuinely `opaqueDimensional`; a remaining translucent representation MUST fail this opaque operation. Do not force reduced transparency or change capability flags to obtain a desired output.

Resolve shape, final depth, key light, body contours and family pigment using the existing portable laws. If focused, resolve the independent focus contrast and ring against the same response geometry. Focus remains geometrically separated from the body. The operation MUST publish the complete result atomically: any body, geometry or navigation failure produces no partial IR. Static endpoints remain usable with reduced motion; no animation is introduced by this operation. Text scale and locale remain in the actual environment and do not implicitly shrink the authored body or hit region.

## Wire contract and conformance

The [request](../schemas/command-paint-request.schema.json) wraps a complete surface request and explicit command appearance. The request MUST be a JSON object; positional request records are invalid. The [result](../schemas/command-paint-ir.schema.json) contains `phase`, `response`, and complete `paint`. Its [body schema](../schemas/command-body-ir.schema.json) reuses the static geometry and pigment constraints with the command state domain. The existing base-state surface operation and its schema remain restricted to rest/focused.

The Rust reference exposes typed and strict source operations; `resina-command-paint <path|->` reads one UTF-8 request, with a 1 MiB limit, writes one result on success, exits 1 with a diagnostic and no output on failure, and exits 2 for invalid usage. Duplicate and unknown JSON members MUST fail. [Public cases](../conformance/ir/command-paint-cases.json) and the independent backend checker cover precedence, state preservation, authored response arithmetic, perceivable state responses, contrast, matching focus geometry and strict failure paths. Existing CPU and GUIdo paint consumers accept the resolved complete paint directly; they do not implement command state rules.

A complete command component still owns anatomy, label layout, semantics, accessibility exposure, native event routing, localization, target placement and motion. This paint contract does not claim those responsibilities are implemented.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
