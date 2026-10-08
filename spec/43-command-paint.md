# Opaque command paint (candidate, schema 0.1.0)

This contract resolves the body and independent navigation paint of an ordinary command surface. It builds on [state composition](16-state-composition.md), [opaque surface appearance](36-opaque-surface-ir.md), and [complete surface paint](39-surface-paint-ir.md). The operation is headless and renderer agnostic. It defines static endpoints; it does not define a transition trajectory or a complete Button component.

## Scope and state channels

A request MUST use a `control.passive`, `control.interactive`, or `control.primary` material role, bound through the actual theme and environment. Cast, Frost and Elastomer are valid command families. Gel MUST NOT be used. Supported signals are `rest`, `hover`, `pressed`, `disabled`, and `focused`, in any valid nonempty combination. Every other signal MUST fail explicitly until its component appearance contract exists.

The body phase is `disabled` when disabled is present, otherwise `pressed` when pressed is present, otherwise `hover` when hover is present, otherwise `rest`. This is precedence for this command body's pigment and depth channels, not a change to the state set or a general state precedence rule. All input signals MUST remain in the body IR and any navigation binding. `focused` independently requires a visible focus indicator, including when pressed or disabled is also present. State selection MUST NOT infer enablement, focus eligibility, activation, selection, or a semantic action. The [activation contract](42-command-activation.md) owns gesture lifecycle.

## Authored responses

The [appearance schema](../schemas/command-appearance.schema.json) requires explicit hover, pressed and disabled responses for each of Cast, Frost and Elastomer. Every profile MUST be validated, including families and phases not selected by this request. There are no backend defaults. Rest is the identity response: `bodyMix = 0` and `depthScale = 1`.

`bodyMix` MUST be finite and in [-1, 1]. For an encoded sRGB body channel `c`, a negative mix `m` resolves to `c * (1 + m)`; a nonnegative mix resolves to `c + (1 - c) * m`. Alpha is preserved. This uses the same encoded sRGB shade/lift convention as the existing opaque pigment contract. The response applies to both the opaque fallback body and Frost's actual portable body before legibility selection. Foreground and semantic color role remain unchanged. Final content contrast MUST be calculated against the actual resolved body, never the unmodified rest body. Insufficient contrast MUST fail; the operation MUST NOT silently reduce the authored response.

`depthScale` MUST be finite and in [0, 1]. Final depth is the theme-bound semantic elevation depth multiplied by this scale. The front contour and content footprint remain stable; only the dimensional extrusion is compressed. This is depth compression, not in-plane elastic deformation. Body silhouette, edge interior and focus silhouette MUST all use this same final depth. The semantic elevation remains available as intent in the IR.

The [light](../definitions/command-appearance-light.json) and [dark](../definitions/command-appearance-dark.json) definitions are reference authoring, not universal numeric material constants. Their restrained Cast/Frost depth compression and stronger Elastomer depth compression follow the blueprint's qualitative family distinction. These values do not calibrate spring mechanics or promise physical simulation. Products and themes may supply different validated responses that satisfy contrast and material identity requirements.

## Resolution and fallback

Resolve the actual environment and theme once. Bind the original surface and preserve its full state set. Apply the selected authored body response, then run the existing content and adjacent-edge guards. Frost MUST retain actual capability, preference and legibility selection. An opaque output is valid only when the resulting Frost representation is genuinely `opaqueDimensional`; a remaining translucent representation MUST fail this opaque operation. Do not force reduced transparency or change capability flags to obtain a desired output.

Resolve shape, final depth, key light, body contours and family pigment using the existing portable laws. If focused, resolve the independent focus contrast and ring against the same response geometry. Focus remains geometrically separated from the body. The operation MUST publish the complete result atomically: any body, geometry or navigation failure produces no partial IR. Static endpoints remain usable with reduced motion; no animation is introduced by this operation. Text scale and locale remain in the actual environment and do not implicitly shrink the authored body or hit region.

## Wire contract and conformance

The [request](../schemas/command-paint-request.schema.json) wraps a complete surface request and explicit command appearance. The [result](../schemas/command-paint-ir.schema.json) contains `phase`, `response`, and complete `paint`. Its [body schema](../schemas/command-body-ir.schema.json) reuses the static geometry and pigment constraints with the command state domain. The existing base-state surface operation and its schema remain restricted to rest/focused.

The Rust reference exposes typed and strict source operations; `resina-command-paint <path|->` reads one UTF-8 request, with a 1 MiB limit, writes one result on success, exits 1 with a diagnostic and no output on failure, and exits 2 for invalid usage. Duplicate and unknown JSON members MUST fail. [Public cases](../conformance/ir/command-paint-cases.json) and the independent backend checker cover precedence, state preservation, authored response arithmetic, contrast, matching focus geometry and strict failure paths. Existing CPU and GUIdo paint consumers accept the resolved complete paint directly; they do not implement command state rules.

A complete command component still owns anatomy, label layout, semantics, accessibility exposure, native event routing, localization, target placement and motion. This paint contract does not claim those responsibilities are implemented.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
