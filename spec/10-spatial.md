# Spatial roles (candidate, schema 0.1.0)

Spatial roles name layout intent without fixing a physical scale, renderer, toolkit, device class, or product. A theme supplies a [spatial assignment](../schemas/spatial-assignments.schema.json) for one resolution context. Its `schemaVersion` is `0.1.0`, and `roles` MUST contain exactly these paths:

| Role | Meaning |
| --- | --- |
| `space.control.inline` | Separation between adjacent control content on the inline axis. |
| `space.control.block` | Separation between adjacent control content on the block axis. |
| `space.container.inner` | Inset between a container edge and its content. |
| `space.container.outer` | Separation between a container and surrounding content. |
| `space.group` | Separation between related content groups. |
| `space.section` | Separation between distinct sections. |
| `space.page` | Inset from the usable page or surface boundary. |

Each role MUST map to a nonempty path of a resolved DTCG `dimension` token. Missing or unknown roles, unsupported versions, missing tokens, wrong types, invalid dimensions, and negative distances are errors. Multiple roles MAY share one token. The role does not imply a physical direction in right-to-left layouts: inline and block follow the resolved layout flow. Safe-area insets are separate environment geometry and are not included in a spatial token.

Headless spatial binding preserves each validated DTCG dimension object, including its unit, without converting it to pixels. It reports every failed role in the order above. The [assignment vectors](../conformance/spatial/assignment-vectors.json) and [resolution vectors](../conformance/spatial/resolution-vectors.json) define this contract. This binding is semantic data, not a complete layout or render-ready Resina IR.

Numerical values in conformance vectors are illustrative inputs for validation, not normative spacing values.

The conceptual foundation scale is `space.0` through `space.8`. Its numerical values and mappings to these roles remain open until dense desktop, touch, couch, and increased-text prototypes have been calibrated. Minimum interactive hit regions are a separate component and accessibility contract; `space.touch.minimum` MUST NOT be used as a substitute for hit-target rules.
