# Opaque base-state surface IR (candidate, 0.1.0)

Blueprint §§21–22, 27–28, 36, and 42 require portable material identity before advanced rendering. This contract composes the existing shape, depth, pigment, light, and readability operations into one static opaque surface appearance. It is a Resina IR contract defined by this document, its schemas, and conformance cases; Rust is its reference implementation. It does not define a component tree, layout, interaction behavior, motion, optical treatment, translucent surface, or a complete Resina bundle.

## Inputs and resolution

The [request](../schemas/opaque-surface-request.schema.json) explicitly supplies version `0.1.0`, a [theme resolution request](22-theme-compilation.md), surface intent, size, [appearance profile](../schemas/opaque-surface-appearance.schema.json), foreground role, actual adjacent opaque sRGB color, and minimum content and edge contrast. `postTreatmentBackdrop` is optional for non-Frost surfaces and required for Frost. When present it MUST be a valid known opaque sRGB color; null is invalid. The caller supplies actual local surroundings, rather than an assumed page background.

The appearance profile contains complete shape and elevation depth assignments, opaque pigment profiles, one physical key light, and an explicit band profile for each of Cast, Frost, Elastomer, and Gel. Band widths are logical `px`: `edgeWidth` MUST be finite and positive; `highlightWidth` MUST be finite and nonnegative. Zero highlight width explicitly suppresses the highlight band. Profile validation MUST reject unknown fields, invalid values, and unsupported versions. The [authored Tier 0 profile](../definitions/tier0-surface-appearance.json) composes the current shape, depth, pigment, and key-light definitions. Its numerical choices are reference authoring, not universal values imposed on themes.

Compile the theme once and retain its resolved token mapping. Resolve its semantic bindings against the **supplied** environment, then apply [base-state readability](28-surface-readability.md), preserving its contrast and optical-stack guards. Only `rest` and an effective current treatment of `none` are supported. An unresolved translucent body MUST fail; do not change the environment or capabilities to force opacity. Frost may reach this contract through its actual capability, preference, or legibility fallback and MUST report `opaqueDimensional`. Other families MUST NOT report a Frost representation or a content fallback.

Resolve the selected shape against the same theme token mapping. Validate all six depth bindings and select the bound elevation's extent. Resolve physical lighting from that extent without normal samples. Light direction and extrusion MUST NOT mirror in RTL; logical corner mapping follows the supplied layout direction. Derive opaque body, side, and highlight pigments from the selected readable body and family profile. Content foreground and exterior edge colors retain their resolved roles, measured ratios, and fallback flags.

The [foundation depth vectors](../conformance/elevation/foundation-depth-vectors.json) publish raw logical extents `0, 2, 4, 6, 8`. The [Tier 0 depth assignment](../definitions/tier0-depth.json) uses zero for embedded/base and successive positive extents for raised/floating/overlay/modal. These are explicit reference choices; elevation roles do not imply fixed numbers.

## Geometry and material regions

Let `F` be the bounded front shape, `e` the edge width, `h` the highlight width, and `d` the physical side offset. Use [inset resolution](33-inset-contour.md) **independently from F** at `e` and `e+h`; do not repeatedly inset an already inset approximation. Use the [extruded contour operation](35-extruded-contour.md) to represent each boundary as ordinary physical lines and elliptical arcs:

| IR member | Region bounded by the contour |
| --- | --- |
| `front` | `F` |
| `silhouette` | Sweep of `F` along `[0,d]` |
| `edgeInterior` | Sweep of inset `F(e)` along `[0,d]`, translated by `(e,e)` |
| `highlightOuter` | Inset `F(e)`, translated by `(e,e)` |
| `content` | Inset `F(e+h)`, translated by `(e+h,e+h)` |

Each placed contour records its local contour and physical translation separately. Dimensions and coordinates are logical `px`. The content region MUST have positive width and height. A combined band extent that overflows, an inset outside the front bounds, or widths lost to numerical precision MUST fail diagnostically. Do not silently omit a requested band or emit empty content geometry.

The material paint semantics are:

1. Fill `silhouette` with side pigment.
2. Fill `front` with body pigment.
3. Paint the highlight in `highlightOuter` minus `content`, using the directional law below.
4. Fill `silhouette` minus `edgeInterior` with the guarded edge color.

The exterior edge includes the visible side boundary. The highlight remains inside that edge, and the content region remains uniformly body-colored. A zero side offset produces no visible side region; a zero highlight width produces an empty highlight band. These are valid authored choices. The explicit edge remains present.

At a physical point in the highlight band, find the nearest boundary point or points on `highlightOuter`. For each nearest boundary segment, evaluate its outward unit normal; at tied segments or points, use the greatest `clamp(dot(normal, lighting.direction), 0, 1)`. Straight edges use their outward normal. Circular arcs use their outward radial normal; square vertices use their incident segment normals. If a point is an arc center, consider all equally nearest normals on that arc. This maximum defines the weight even at ties. The current shape fallback emits circular or square corners; an implementation MUST NOT substitute an ellipse's radial parameter for its geometric normal if extending this contract to elliptical authoring.

For each portable encoded sRGB body channel `c`, the painted highlight channel is `c + (1-c) × pigment.profile.highlightLift × weight`, with alpha one. This is the [opaque pigment law](29-opaque-pigment.md), weighted by the [physical key light](34-key-light.md). The resolved full highlight color is the weight-one endpoint. A backend MUST NOT add a material-specific light direction, shade factor, or color choice. The formula is an appearance definition, not a requirement to use a gradient, transparency, or a shader: a backend can subdivide the band into opaque fills to its documented rasterization quality.

Compound filled paths and holes are ordinary contour realization. They do not imply a Resina `masks` capability, a toolkit clipping API, or a GPU command. For example, [SVG fill rules](https://www.w3.org/TR/SVG2/painting.html#FillRuleProperty), [Qt filled paths](https://doc.qt.io/qt-6/qpainterpath.html), and [Slint Path](https://docs.slint.dev/latest/docs/slint/reference/elements/path/) provide upstream evidence that these regions can be expressed as conventional filled geometry. Their APIs are not part of Resina IR, and documentation evidence does not certify any backend's implementation.

## Readability, scope, and conformance

The content contrast ratio applies to the resolved foreground over the uniform body inside `content`. Consumers MUST keep content within that region or reassess its actual painted background. The edge ratio applies to the supplied adjacent color. Neither ratio certifies text layout, focus indicators, accessibility semantics, hit targets, or a dynamic backdrop. Text scale and localization remain environment inputs; this operation does not infer content size or suppress those requirements.

The [IR schema](../schemas/opaque-surface-ir.schema.json) preserves bound intent, appearance regions, pigments, lighting, and readability evidence. There are no widget classes, toolkit paths, platform identifiers, device coordinates, or product behavior. Geometry containment, closure, unit vectors, cross-field consistency, and the paint law are semantic requirements beyond structural validation.

The [public baseline](../conformance/ir/opaque-surface-request.json), [hand-derived result](../conformance/ir/opaque-surface-expected.json), and [cases](../conformance/ir/opaque-surface-cases.json) cover all four families, opaque Frost, zero depth/highlight, physical direction under RTL, strict validation, unsupported state/treatment, missing bindings, and numeric failures. Geometry comparison uses absolute `1e-12` logical `px` or relative `1e-12`, whichever is larger; unit directions, weights, pigments, and ratios use absolute `1e-12`. Structures, versions, roles, and flags are exact. These tolerances concern resolution, not rasterized pixels. Renderer approximation quality requires separate backend conformance evidence.

`resina-opaque-surface <path|->` reads one UTF-8 request up to 1 MiB and rejects duplicate members at every depth. Success exits 0 with one complete JSON result and no diagnostic; invalid input exits 1 with a diagnostic and no result; usage errors exit 2. The [external checker](../tools/check_opaque_surface_backend.py) checks schemas, public cases, numeric output, and repeated results. The Rust typed operation also accepts a previously compiled theme, preserving the same resolution semantics across multiple surfaces.

This static opaque IR is available at Tier 0. State-specific appearance, translucent and optical treatment IR, advanced curvature, complete component semantics, and GUIdo, Quickshell, Slint, and Web conformance remain separate contracts and implementation work.
