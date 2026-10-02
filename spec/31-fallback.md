# Material fallback (candidate)

Fallback changes the realization of a selected material family; it MUST NOT silently change the requested semantic role or the theme's family assignment. Every representation MUST retain the family's recognizable hierarchy and states at lower capability levels. The first defined chain is Frost:

```text
shapedBackdrop
→ regularBackdrop
→ translucentPigmented
→ opaqueDimensional
```

The Frost resolver takes explicit renderer capabilities, accessibility preferences, and quality policy. It selects exactly one representation in this order:

1. If `reducedTransparency` or `highContrast` is true, or `translucentSurfaces` is false, select `opaqueDimensional`.
2. Otherwise, if quality is `economy`, select `translucentPigmented`.
3. Otherwise, if `backdropEffect` and `backdropBlur` are both true, select `shapedBackdrop` only when `shapedBackdrop` is also true and quality is `full`; select `regularBackdrop` in the other cases.
4. Otherwise, select `translucentPigmented`.

`shapedBackdrop` retains a shaped, diffusive backdrop response. `regularBackdrop` retains diffusion over an unshaped backdrop. `translucentPigmented` uses a pigmented translucent body without backdrop sampling or blur. `opaqueDimensional` uses an opaque pigmented body with a visible edge and basic depth separation. All four are Frost; none is a request for a specific shader, scene graph, or window-system effect. The backdrop representations require `translucentSurfaces` as well as their named backdrop capabilities.

The representation choice does not make a translucent semantic color opaque. Before an `opaqueDimensional` surface is ready to paint, its pigment contract must supply an [opaque color fallback](13-opaque-color-fallback.md) and a visible edge. A bound surface retains the source color and portable sRGB fallback for inspection; a backend MUST NOT treat the representation name as permission to discard source alpha or invent a backdrop.

Reduced motion does not change this static representation choice; motion resolution has a separate contract. A zero-capability renderer selects `opaqueDimensional`. The [Frost conformance vectors](../conformance/materials/frost-fallback-vectors.json) cover quality, capability combinations, accessibility overrides, and Tier 0. Other material families and treatments are not assigned implicit fallback behavior by this document.
