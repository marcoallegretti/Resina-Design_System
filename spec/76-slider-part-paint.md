# Slider part paint (candidate, 0.1.0)

This contract resolves one allocated track or thumb's complete opaque body and
navigation paint from [Slider appearance](71-slider-appearance.md). The caller
supplies the actual part size, authored surface, appearance, readOnly flag,
foreground, optional actual post-treatment backdrop, explicit content/edge
thresholds, and checked adjacent and optional surrounding ranges. It does not
infer another part's geometry, background or allocation.

## Resolution

Require a persistent control material role and the complete coherent Slider
state projection. Select the actual part/family/phase response; Gel is invalid
even at rest. Resolve the theme against the actual environment once. Apply the
selected body mix before content and common adjacent-edge readability. Preserve
Frost capability, preference and legibility fallbacks; unresolved translucency
fails the opaque paint contract. Never force an environment flag or drop states.

Apply depthScale to the selected elevation before body and focus geometry.
Reuse existing shape, physical lighting, pigment and opaque region laws. Response
depth compression is not in-plane elastic deformation, and no value, target,
layout, color role or physical direction changes through response selection.

The thumb owns navigation paint, consistent with the value handle described in
the [W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/).
Focused remains in both parts' full state sets. A focused thumb requires a
nonempty actual surrounding range set and one common focus color passing every
range. The track publishes no separate focus ring. Any supplied surrounding set
must be nonempty, including when unused. Focus follows the same response-adjusted
depth, shape and physical extrusion as the thumb body. Focus does not replace
the selected body phase or require Rest.

Adjacent ranges are mandatory and nonempty for each part. The actual producer
must cover all relevant track, canvas and other neighboring colors, including
continuous highlights; [derived opaque paint ranges](75-opaque-paint-color-ranges.md)
can supply a conservative whole-surface cover but do not prove adjacency.
Content, edge, opacity, geometry or focus failure publishes no part result.
Do not resolve against one sampled color and repair the published edge afterward.

## Public records

The [request](../schemas/slider-part-paint-request.schema.json) is an object with
`schemaVersion: "0.1.0"`, `part` (`track` or `thumb`), boolean `readOnly`, a complete
[theme request](../schemas/theme-resolution-request.schema.json), `surface`,
`size`, `appearance`, `interactionAppearance`, `foregroundRole`, `adjacentRanges`,
`minimumContentContrast` and `minimumEdgeContrast`. `postTreatmentBackdrop` and
`surroundingRanges` are optional actual inputs. Omission is distinct from `null`;
supplied values must validate even when the selected part does not use them.

Each range is a named `lower`/`upper` object containing complete opaque sRGB
colors. Every lower component must be at most its upper component. The thresholds
are finite numbers from 1 through 21. Named records do not accept positional
arrays, and part and foreground role names do not accept object alternatives.
Unknown members, decoded duplicate members, unsupported versions, invalid colors,
incoherent states and incomplete appearance profiles are errors. Validate the
complete authored profiles, including responses outside the selected part,
family or phase. Reuse the existing theme, environment, surface, appearance and
[common contrast](73-common-background-contrast.md) contracts; no backend identity is an input.

The [result](../schemas/slider-part-paint-ir.schema.json) retains
`schemaVersion: "0.1.0"`, `part`, `readOnly`, actual `layoutDirection`, selected
`phase`, selected `response` and `paint`. Paint retains its own version, the
complete [Slider body IR](../schemas/slider-part-body-ir.schema.json), and focus
IR exactly when the thumb is focused. The full body state set includes dragging;
phase precedence and read-only coherence follow [Slider appearance](71-slider-appearance.md).
Rest has the identity response (`bodyMix: 0`, `depthScale: 1`).

The body publishes the final opaque pigment, readability decisions, physical
lighting and complete region geometry. Navigation retains the original surface
binding and uses the final response-adjusted silhouette. In a Frost legibility
fallback, the body's final representation supersedes the preliminary Frost
representation in that navigation binding for body painting. Do not reconstruct
body paint from the navigation binding.

## Reference and evidence

The typed Rust result retains part, readOnly, actual layoutDirection, selected
phase and response plus the complete body/focus paint IR. Rust is the reference,
not this contract's normative definition. `resolve_slider_part_paint_source`
admits the public records and delegates to the existing typed resolver.
The shared private paint core accepts genuine scalar or ranged backgrounds;
geometry helpers accept only their actual geometry inputs. Existing Command,
Toggle and base-surface behavior remains compatible.

Tests replay public Slider phase records across both parts and all three
persistent families, checking complete states, response body arithmetic, scaled
extrusion and thumb-only navigation matching the body silhouette. Negative
tests cover incompatible mixed backgrounds, missing/empty ranges and invalid
materials. Existing Command/Toggle/base/focus tests guard shared regressions.

`resina-slider-part-paint <path|->` reads one UTF-8 request from a file or stdin,
bounded to 1 MiB. Success exits 0 with one strict JSON result and no diagnostic;
failure exits 1 with no result and a diagnostic; usage errors exit 2.
The [portable checker](../tools/check_slider_part_paint_backend.py) runs the
[authored cases](../conformance/ir/slider-part-paint-cases.json) and public phase
matrix through that protocol, repeating successes deterministically. It checks
full IR against independent sRGB contrast, response, pigment and contour
arithmetic for the explicit rectangular, upward-lighting fixture. These cases
cover both directions and parts, all persistent families, Tier 0, common range
fallbacks, zero compressed depth, all three advanced Frost representations and
their legibility/preference fallbacks. They do not certify arbitrary shape
calibration or a complete component. Source tests additionally prove complete
typed/source parity, decoded-name handling, strict rejection and transport limits.
Nonzero advanced Frost responses over a nonblack backdrop verify that mixing
precedes composition, with thresholds separating the two arithmetic orders.

Part-local paint does not assemble the control. A complete Slider must still
place both parts consistently with current value/presentation, prove real
background and painted-target coverage, preserve localized/scaled content and
semantics, implement motion and native interaction, and pass rendered visual
review. Tier0 uses these same laws; translucent and transient Gel effects require
their own valid lower-capability representations.
