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

The representation choice does not make a translucent semantic color opaque. One validated [Frost pigment input](../schemas/frost-pigment.schema.json) supplies `tintStrength` strictly between 0 and 1 for a resolution context. It has no default. This dimensionless value is the colorant body's opacity multiplier, not an opacity for child content or a physical transmission measurement. For `shapedBackdrop`, `regularBackdrop`, and `translucentPigmented`, surface binding MUST multiply the selected semantic color's portable fallback alpha by `tintStrength`, preserve its sRGB components, and publish the result as `frostPortableBody`. A zero resulting alpha MUST fail binding without a surface. This operation does not sample or composite a backdrop. For `opaqueDimensional`, binding MUST instead use the selected role's [authored opaque color fallback](13-opaque-color-fallback.md) unchanged, regardless of the source alpha or tint strength. These rules give an opaque semantic color a translucent Frost body when translucency is available and a distinct authored body when it is not.

The portable body is one pigment prerequisite, not final Frost paint. The [opaque pigment operation](29-opaque-pigment.md) can derive side and highlight colors after `opaqueDimensional` has selected an opaque body. Edge geometry, highlight placement, depth separation, treatment, and actual adjacent-color legibility still require resolution before a backend can paint a complete Frost surface. A backend MUST NOT treat the representation name or body color as permission to invent a backdrop or discard the specified alpha.

For a known post-treatment backdrop and actual opaque foreground, the [Frost legibility operation](24-frost-legibility.md) MAY further select `opaqueDimensional` when the selected body does not meet an explicitly supplied contrast threshold. It fails when even the authored opaque body cannot meet that threshold.

Reduced motion does not change this static representation choice; motion resolution has a separate contract. A zero-capability renderer selects `opaqueDimensional`. The [Frost conformance vectors](../conformance/materials/frost-fallback-vectors.json) cover quality, capability combinations, accessibility overrides, and Tier 0. Other material families and treatments are not assigned implicit fallback behavior by this document.
