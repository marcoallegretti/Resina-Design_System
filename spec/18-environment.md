# Environment snapshot (candidate, schema 0.2.0)

This document defines the input contract for a single resolution operation. `EnvironmentSnapshot` describes conditions available to Resina; it does not identify a device, operating system, toolkit, or renderer. The machine-readable form is [environment.schema.json](../schemas/environment.schema.json). A producer MUST provide every field. A consumer MUST reject unknown fields, missing fields, and invalid values; it MUST NOT silently substitute defaults. Identical snapshots convey identical environment information to the resolver.

Schema 0.2.0 adds the required `translucentSurfaces` capability. A 0.1.0 producer must determine and provide that capability explicitly before labeling its snapshot 0.2.0; consumers MUST NOT infer it from backdrop flags or upgrade the schema version automatically. The [0.1.0 schema](../schemas/versions/environment-0.1.0.schema.json) remains available for migration tooling, while current resolution rejects that version.

The JSON object has `schemaVersion` equal to `0.2.0` and these members:

| Member | Meaning |
| --- | --- |
| `geometry` | Available rectangular extent and safe-area insets in logical units; aspect ratio and orientation are derived from its dimensions. |
| `scale` | Positive ratio of physical pixels to one logical unit. This is information for backends, not a reason to infer a device category. |
| `textScale` | Positive user-preferred multiplier for text size. |
| `inputCapabilities` | Set of currently available abstract input capabilities. Multiple capabilities may coexist. An empty set means none was reported. |
| `viewingProfile` | Explicit `near`, `desk`, `couch`, or `unknown`; it MUST NOT be inferred solely from input capabilities. |
| `densityPreference` | Explicit `compact`, `standard`, `comfortable`, or `immersive`. |
| `accessibilityPreferences` | Explicit `reducedMotion`, `reducedTransparency`, and `highContrast` booleans. `false` means the preference is not requested, not that a backend may disregard accessibility. |
| `locale` | Nonempty locale identifier supplied by the host. This field is opaque to the environment model; localization behavior and identifier validation belong to the localization contract. |
| `layoutDirection` | Explicit `ltr` or `rtl`; the resolver MUST NOT infer it from `locale`. |
| `rendererCapabilities` | Explicit support flags described below. Flags describe available primitives, never a backend name. |
| `qualityPolicy` | Requested `economy`, `balanced`, or `full` quality. Capability support and quality request are separate. |

`geometry.width` and `geometry.height` MUST be finite and greater than zero. Their quotient `width / height` is the aspect ratio and MUST be representable as a finite, positive number. Orientation is `landscape` when width exceeds height, `portrait` when height exceeds width, and `square` when they are equal. These derived properties MUST NOT be repeated in the serialized snapshot. Each safe-area inset (`start`, `end`, `top`, `bottom`) MUST be finite and nonnegative. The sum of `start` and `end` MUST be smaller than `width`; the sum of `top` and `bottom` MUST be smaller than `height`. `start` and `end` are logical directions and therefore remain meaningful in both layout directions. `scale` and `textScale` MUST be finite and greater than zero. JSON itself cannot encode non-finite numbers; implementations constructing snapshots in memory MUST apply the same checks.

The allowed `inputCapabilities` values are `finePointer`, `coarsePointer`, `hover`, `directTouch`, `stylus`, `keyboard`, `directionalNavigation`, `gamepad`, and `voiceAction`. Duplicate entries are invalid. No capability implies another: a gamepad does not imply couch viewing, and a fine pointer does not imply hover.

`rendererCapabilities` has mandatory boolean fields `gradients`, `translucentSurfaces`, `innerShadow`, `advancedShadow`, `sdfShapes`, `backdropEffect`, `backdropBlur`, `shapedBackdrop`, `dynamicLighting`, `deformation`, `masks`, `customShader`, `wideGamut`, and `hdr`. `translucentSurfaces` means the renderer can composite a pigmented surface over underlying content without sampling or blurring that content; it is separate from backdrop effects. Every combination is valid at this layer. A later resolver may require a combination to realize an effect and MUST select a documented lower-capability representation when it is unavailable. The tier names in the blueprint are descriptive and MUST NOT replace capability tests.

Geometry, scale, and quality do not define a rendering algorithm. A renderer may map logical units to its coordinate system and choose a cheaper realization under a lower quality policy, while preserving the resolved semantic intent.

The usable content bounds are derived from validated geometry and safe-area insets. In a viewport whose origin is the top-left corner, `left` equals `safeArea.start` for `ltr` and `safeArea.end` for `rtl`; `top` equals `safeArea.top`. `width` is `geometry.width - (safeArea.start + safeArea.end)` and `height` is `geometry.height - (safeArea.top + safeArea.bottom)`. All four values remain in the snapshot's logical units; `scale` and `textScale` do not alter them. The [content-bounds vectors](../conformance/environment/content-bounds-vectors.json) cover asymmetric RTL insets, zero insets, and fractional values. This rectangle describes usable geometry, not component layout.

The [conformance cases](../conformance/environment/) are normative examples of valid and rejected snapshots and derived geometry. The Rust reference crate implements this contract without a GUI dependency.
