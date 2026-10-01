# Renderer capabilities (candidate)

Renderer capabilities are claims about available drawing primitives, independent of renderer, toolkit, platform, and product. The [environment snapshot](18-environment.md) carries the capability set for one resolution operation. Every flag is explicit. A false flag MUST NOT be inferred from a renderer name or from another flag, and a true flag alone does not authorize an effect that needs other primitives.

Tier 0 requires the ability to draw an opaque pigmented surface with a shape, visible edge, basic depth separation, hierarchy, text, and interaction state. Optional features are advertised by the `rendererCapabilities` fields. `translucentSurfaces` is separate from `backdropEffect`, `backdropBlur`, and `shapedBackdrop`: alpha compositing a pigmented surface does not imply that the renderer can sample, blur, or shape the content behind it.

Capability flags describe support, while `qualityPolicy` expresses a requested cost level. `economy`, `balanced`, and `full` MUST NOT be interpreted as device categories or renderer identities. Accessibility preferences may restrict a representation even when capabilities and quality would permit it. Rendering tiers in the blueprint describe increasing visual scope; they are not a substitute for explicit capability checks.

The first normative capability decision is [Frost fallback](31-fallback.md). Other material and treatment decisions require their own rules and conformance vectors before a renderer may claim support for them.
