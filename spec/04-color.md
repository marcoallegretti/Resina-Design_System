# Semantic color roles (candidate, schema 0.1.0)

Resina color roles name visual meaning independently of a palette, material, renderer, and product. A theme supplies a [color assignment](../schemas/color-assignments.schema.json) for one resolution context. Its `schemaVersion` is `0.1.0`, and its `roles` object MUST contain exactly these roles:

| Family | Roles |
| --- | --- |
| Accent | `accent.primary`, `accent.secondary`, `accent.tertiary` |
| Surface | `surface.base`, `surface.low`, `surface.high`, `surface.chrome` |
| Content | `content.primary`, `content.secondary`, `content.muted`, `content.inverse` |
| Outline | `outline`, `outline.strong` |
| Status | `status.error`, `status.warning`, `status.success`, `status.info` |
| Interaction | `focus`, `selection` |

Each role maps to one nonempty path of a resolved DTCG token whose declared type is `color`. The role name and token path are separate: consumers MUST NOT infer a role from token grouping, token names, or color values. Missing or unknown roles, unsupported versions, unresolved paths, and non-color targets are errors. Several roles MAY point to one token where a theme intentionally shares a color.

The [assignment vectors](../conformance/color/role-assignment-vectors.json) define mapping validation. Their token paths are illustrative, not a palette or default theme. This contract establishes semantic identity only. The [sRGB–Oklab conversion](04-color-conversion.md) provides a separate color calculation. Palette derivation, contrast guarantees, material pigmentation, color-gamut adaptation, and rendering fallback require separate contracts and conformance evidence before a resolved color is ready for a backend.

Headless semantic color resolution consumes a valid assignment and the validated path-to-token result of DTCG document resolution. For each role, the resolver MUST find the named token, require its declared type to be `color`, and preserve its validated DTCG color value without conversion. It MUST collect and report all missing paths, wrong types, and invalid color values in the role order above. The [resolution vectors](../conformance/color/resolution-vectors.json) define these outcomes. Separate [sRGB](07-color-fallback.md) and [opaque color fallback](13-opaque-color-fallback.md) contracts provide portable representations. This semantic binding remains distinct from render-ready Resina IR and does not guarantee readable contrast.
