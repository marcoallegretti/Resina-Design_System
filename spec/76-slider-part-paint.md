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

## Reference and evidence

The typed Rust result retains part, readOnly, selected phase and response plus
the complete body/focus paint IR. Rust is the reference, not this contract's
normative definition. No new source parser or renderer dependency is introduced.
The shared private paint core accepts genuine scalar or ranged backgrounds;
geometry helpers accept only their actual geometry inputs. Existing Command,
Toggle and base-surface behavior remains compatible.

Tests replay public Slider phase records across both parts and all three
persistent families, checking complete states, response body arithmetic, scaled
extrusion and thumb-only navigation matching the body silhouette. Negative
tests cover incompatible mixed backgrounds, missing/empty ranges and invalid
materials. Existing Command/Toggle/base/focus tests guard shared regressions.

Part-local paint does not assemble the control. A complete Slider must still
place both parts consistently with current value/presentation, prove real
background and painted-target coverage, preserve localized/scaled content and
semantics, implement motion and native interaction, and pass rendered visual
review. Tier0 uses these same laws; translucent and transient Gel effects require
their own valid lower-capability representations.
