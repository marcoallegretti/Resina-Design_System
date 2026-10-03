# Focus indicator IR (candidate, 0.1.0)

The [focus color contract](27-focus-indicator.md) establishes a separate navigation cue with a guarded opaque pigment, 2 logical `px` stroke width, and 2 logical `px` gap. This contract resolves its complete static geometry around the bound form's directional side-plane silhouette. It preserves concurrent states; it does not resolve their body appearance, mechanics, motion, or accessibility behavior.

The [request](../schemas/focus-ir-request.schema.json) explicitly supplies version `0.1.0`, a [theme resolution request](22-theme-compilation.md), focused surface intent, positive surface size, complete shape and depth assignments, physical key light, and the actual opaque sRGB surrounding color. Missing or unknown fields, unsupported versions, duplicate JSON members, invalid colors, unfocused intent, empty surfaces, invalid shape/depth bindings, and insufficient focus contrast MUST fail without a partial IR. Only an effective current optical treatment of `none` is supported: an unresolved treatment's geometry MUST NOT be silently ignored. An ancestor treatment followed by `none` remains permitted; this operation resolves local geometry, not the ancestor's rendering.

Compile the theme once and retain its token mapping. Resolve bindings against the supplied environment, then apply focus color resolution without removing availability, validation, selection, interaction, navigation, activity, or base signals. Resolve the bound shape and all six elevation depths from the same theme tokens. Use its selected depth and the physical key light to resolve side offset. Map logical corners using the actual layout direction; light and offset MUST NOT mirror in RTL. A renderer capability or preference change may change the bound Frost representation, but MUST NOT suppress the navigation cue.

## Parallel exterior ring

Let `F` be the normalized front shape and `d` its physical side offset. Its exterior footprint is the [swept silhouette](35-extruded-contour.md) `S = F ⊕ [0,d]`. Let `D(a)` be the closed disk of radius `a`. Define:

| Region | Definition |
| --- | --- |
| `silhouette` | `S` |
| `inner` | `S ⊕ D(gap)` |
| `outer` | `S ⊕ D(gap + strokeWidth)` |
| Visible focus ring | `outer` minus `inner` |

The ring MUST surround the full footprint, including the exposed side plane. Applying an ordinary stroke to the front rectangle alone would not satisfy this definition. Disk offsetting is an ordinary [Minkowski dilation](https://doc.cgal.org/latest/Minkowski_sum_2/index.html), independent of a toolkit stroke convention.

The current [Tier 0 shape profiles](09-geometry.md) produce circular or square corners. Their exact disk offset is a rounded rectangle with width and height increased by `2a`, each **already normalized** corner radius increased by `a`, and origin translated to `(-a,-a)`. Square corners acquire radius `a`, producing round exterior joins. Sweep that enlarged shape along the original physical offset. This follows associativity of point-wise Minkowski addition: `(F ⊕ [0,d]) ⊕ D(a) = (F ⊕ D(a)) ⊕ [0,d]`. Merely adding `a` to both axes of an arbitrary elliptical corner is not an exact disk offset; this contract does not accept arbitrary elliptical authoring.

Each placed boundary retains the [canonical contour record](35-extruded-contour.md) in its own coordinates and a separate physical translation. Both outsets of the ring are computed independently from `F`, using extents 2 and 4. The contour unit is logical `px`; no device scale, toolkit path, rasterization instruction, or GPU object is present. A negative translation is expected and is not an invalid surface position.

For every unit physical direction `n`, conformance requires `hInner(n) = hS(n) + gap` and `hOuter(n) = hS(n) + gap + strokeWidth`. These support-function identities establish containment and constant Euclidean gap/stroke independently of the construction algorithm. Numerical overflow or bounds so large that the gap or stroke is lost to floating-point precision MUST fail diagnostically. Zero-width or zero-height front shapes MUST fail rather than acquiring a fabricated filled footprint through dilation.

Fill the visible ring with the resolved indicator color. Do not paint its hole, connect it to the surface edge, replace another state channel, add a shadow, or fade it through a translucent pigment. The ring is immediately present while focus is active and requires no motion, blur, gradients, mask capability, or custom shader. This static contract introduces no animation. A backend MUST reserve the complete outer paint bounds and avoid clipping or occlusion. Content layout and the indicator's hit testing remain outside this geometry contract.

## Evidence and boundary

The [IR](../schemas/focus-indicator-ir.schema.json) retains the complete focus indicator result, including bound form, state set, actual Frost representation where applicable, chosen color role, unrounded contrast, and fallback flag. It adds the silhouette and both ring boundaries. It is a complete static navigation appearance slice, not a complete focused component. Future state-specific deformation or treatment geometry must define its relationship to the focus footprint before a backend can claim those combinations conform. Consumers MUST transform the surface and its focus geometry consistently when positioning them.

The width and contrast policy originates in [WCAG 2.2 Focus Appearance](https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html); it does not alone establish WCAG conformance. Rendering scale, actual pixel area, clipping, occlusion, and the actual surrounding colors still require backend evaluation. Unknown or varying surroundings require evaluation wherever the ring is painted.

The [baseline request](../conformance/ir/focus-ir-request.json), [hand-derived result](../conformance/ir/focus-ir-expected.json), and [cases](../conformance/ir/focus-ir-cases.json) cover round joins around a sharp raised surface, zero depth, both layout directions, all four families, concurrent states across all seven composition layers, guarded color fallback, invalid inputs, active treatments, and numerical loss. The Rust reference additionally tests parallel-offset support identities across 360 directions for authored Light/Dark themes, every shape and elevation, four physical lighting directions, both layout directions, and all four families in Tier 0 environments at 300% text scale.

Geometry coordinates and bounds use absolute `1e-12` logical `px` or relative `1e-12`, whichever is larger. Arc radial parameters, pigment components, and contrast ratios use absolute `1e-12`. Structures, versions, flags, and state ordering are exact for the resolver protocol. Rasterized approximation quality is a separate backend conformance requirement.

`resina-focus-ir <path|->` reads one UTF-8 request up to 1 MiB. Success exits 0 with one complete JSON IR and no diagnostic; invalid input exits 1 with a diagnostic and no result; usage errors exit 2. The [external checker](../tools/check_focus_ir_backend.py) validates the schema, public geometry and color evidence, strict parsing, and repeated results independently of Rust. The typed reference operation accepts a previously compiled theme for repeated resolution.
