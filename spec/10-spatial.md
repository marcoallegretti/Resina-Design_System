# Spatial roles (candidate, schema 0.2.0)

Spatial roles name layout intent without fixing a physical scale, renderer, toolkit, device class, or product. A theme supplies a [spatial assignment](../schemas/spatial-assignments.schema.json) for one resolution context. Its `schemaVersion` is `0.2.0`, and `roles` MUST contain exactly these paths:

| Role | Meaning |
| --- | --- |
| `space.control.inline` | Separation between adjacent control content on the inline axis. |
| `space.control.block` | Separation between adjacent control content on the block axis. |
| `space.control.inset.inline` | Inset between a control body edge and its content on the inline axis. |
| `space.control.inset.block` | Inset between a control body edge and its content on the block axis. |
| `space.container.inner` | Inset between a container edge and its content. |
| `space.container.outer` | Separation between a container and surrounding content. |
| `space.group` | Separation between related content groups. |
| `space.section` | Separation between distinct sections. |
| `space.page` | Inset from the usable page or surface boundary. |

Each role MUST map to a nonempty path of a resolved DTCG `dimension` token. The two control inset roles instead map each [density mode](17-density.md) to such a path: their value is an object with exactly the members `compact`, `standard`, `comfortable`, and `immersive`. Every mode is authored explicitly; no mode is derived from another, and a single path for an inset role or a density object for any other role is an error. A control inset is not a content gap: `space.control.inline` and `space.control.block` keep their meaning, and changing a content gap never changes a control inset or the reverse. Missing, unknown, or duplicate role or density members, positional density mappings, unsupported versions, missing tokens, wrong types, invalid dimensions, and negative distances are errors. Duplicate members MUST be rejected before converting the source JSON to an object tree, including names that decode to the same string through escapes. Multiple roles MAY share one token. The role does not imply a physical direction in right-to-left layouts: inline and block follow the resolved layout flow. Safe-area insets are separate environment geometry and are not included in a spatial token.

Headless spatial binding preserves each validated DTCG dimension object, including its unit, without converting it to pixels. It validates every density mapping of the inset roles, whichever mode the environment selects. It reports every failed role in the order above, and within an inset role every failed mode in the order `compact`, `standard`, `comfortable`, `immersive`. A [headless result](19-headless-resolution.md) publishes each inset role's value for the environment's explicit `densityPreference`; the other modes do not reach that result. The [assignment vectors](../conformance/spatial/assignment-vectors.json) and [resolution vectors](../conformance/spatial/resolution-vectors.json) define this contract. This binding is semantic data, not a complete layout or render-ready Resina IR. The [0.1.0 schema](../schemas/versions/spatial-assignments-0.1.0.schema.json), which has no inset roles, remains available for migration tooling; current resolution rejects it. Upgrading requires an explicitly chosen inset token for every density mode, not a copy of the content-gap roles.

Numerical values in the assignment and resolution vectors are illustrative inputs for validation. The foundation vectors below define the candidate scale values.

The [foundation token source](../tokens/foundation.json) defines the current candidate scale `space.0` through `space.8` as 0, 4, 8, 12, 16, 24, 32, 48, and 64 `px`. These are DTCG idealized viewport pixels, corresponding to logical UI units that a native backend can translate to its coordinate system; they are not physical display pixels. The [foundation vectors](../conformance/spatial/foundation-vectors.json) fix the resolved values for conformance. The scale was calibrated with representative structural surfaces and controls at dense desktop, touch, and far-viewing layouts, including 200% text and Tier 0 effects. Future role mappings and component layout rules must consider explicit density, geometry, text scale, and input capabilities; they MUST NOT infer a device class from a token value. [Minimum interactive hit regions](15-targets.md) are a separate component and accessibility contract; `space.touch.minimum` MUST NOT be used as a substitute for hit-target rules.
