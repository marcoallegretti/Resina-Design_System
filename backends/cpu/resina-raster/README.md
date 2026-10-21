# CPU paint realization (0.0.1)

`resina-raster` synchronously realizes validated opaque surface and focus indicator IR as bounded row-major, straight RGBA8 sRGB pixels. It uses the canonical point paint evaluators and deterministic regular-grid integration. This is a rendering backend operation; the viewport, sampling grid and output buffer do not enter normative Resina IR.

The library accepts an explicit `Viewport` and samples per axis. `render_surface`, `render_focus` and `render_surface_paint` return a complete `RasterImage` or a diagnostic error, with pixel/sample/allocation/precision limits checked before sampling. There is no file loading, image decoding, worker queue or platform integration in these operations. Consumers must choose the actual device scale, reserve complete paint bounds, prepare the image before exposing a state, and reevaluate it when IR or scale changes. Performance and the renderer's upload/compositing behavior still need measurement at that boundary.

`prepare_viewport` rounds local paint bounds outward on the device grid after applying parent placement. Its `PreparedViewport` exposes the local sampling viewport, already placed logical bounds, and the original far corner for adapter precision checks. The far corner preserves division of the rounded pixel edge rather than recomputing origin plus extent. Preparation does not allocate pixels or check toolkit precision; rendering still enforces pixel/sample/allocation limits, and adapters must validate their native coordinates before rendering. This operation is available without PNG support.

`render_surface_paint` consumes the complete [body/navigation IR](../../../spec/39-surface-paint-ir.md). It evaluates both disjoint paint regions at each shared grid sample and integrates their combined coverage and linear-light color into one image. A coarse pixel may contain samples from both body and ring; averaging or compositing two previously quantized images would not preserve this result. Unfocused paint uses the existing body operation and is byte-identical. Focused paint retains conservative body-interior and combined-exterior shortcuts; uncertain regions evaluate the canonical body and ring laws.

For grids of at least three samples per axis, the backend conservatively proves uniform sample regions in the body interior, focus hole and exterior. These pixels avoid repeated geometry queries while preserving the original sample positions, repeated linear-light accumulation, quantization and resource limits. Edges, highlights, unsupported contours and uncertain arithmetic use the canonical point evaluator. This optimization introduces no IR fields or public APIs.

Use `width`, `height` and `rgba` to inspect an image. `into_rgba` transfers ownership of its existing pixel allocation to a consumer; it does not copy or premultiply the pixels. A toolkit adapter owns any subsequent buffer conversion, GPU upload or cache. The existing [sampling, color, limit and PNG evidence contract](../../../conformance/raster/README.md) describes the complete current operation.

PNG support is an optional feature, enabled by default to preserve the conformance commands and `write_png` API. A production pixel consumer can disable it:

```toml
resina-raster = { path = "../cpu/resina-raster", default-features = false }
```

The three conformance binaries require the `png` feature. The default build retains the existing commands and adds `resina-paint-raster` for a complete body/navigation image. No image-format dependency is needed by a build with default features disabled, and the reference color/model/resolver layers remain free of image-format and toolkit dependencies.

```sh
cargo test -p resina-raster --no-default-features --locked
cargo test -p resina-raster --locked
```

Both configurations are tested in Linux and Windows CI. Coverage includes independent square/circle/focus geometry oracles, linear-light filtering, straight alpha, resource and precision failures, and ownership transfer without a copy; the PNG configuration additionally checks encoding metadata, transport and failures. This backend does not implement components, input, layout, animation or optical treatments, and does not replace GUIdo's role as the high-fidelity reference renderer.

## Continuous-outline focus

`render_outline_focus(&OutlineFocusIr, Viewport)` realizes the analytic ring from
[specification 84](../../../spec/84-outline-focus-geometry.md). It retains the
resolved guarded sRGB color and explicit physical sweep. Circular and quartic
corner axes are never enlarged to substitute for Euclidean disk offsets.

Adaptive support queries construct convex contact hulls and conservative support
half-plane intersections. Exact represented-input rational clipping bounds each
pixel's ring area. The contact hull's coordinate uncertainty is accounted for
through convex erosion; the containing intersection accounts for support and
normal uncertainty. Subdivision guides preparation, while the final area bounds
decide whether coverage is accurate enough to quantize. The admitted coverage
interval is at most 1/255 wide; midpoint rounding differs from exact coverage by
at most one alpha byte. Pixels use straight RGBA8; transparent pixels are zero.

Preparation admits at most 4,096 support directions and 134,217,728 bounded work
steps, alongside the existing viewport and pixel limits. Lost precision,
nonconvergence and exhausted limits return a diagnostic without partial output.
Filtered orientation predicates use outward float intervals only when they prove
the sign; cancellation or overflow falls back to exact rational arithmetic.
Prepare and reuse the complete image before exposing a state. This operation
does not schedule frames, compose component content or accept component styling.

`resina-material-relief-raster` accepts a
[material-lit relief paint request](../../../schemas/material-relief-paint-request.schema.json)
and the same explicit viewport and sampling arguments as `resina-relief-raster`.
It preserves guarded edges and uniform content while evaluating bounded light
over the analytic face. Success writes one RGBA8 perceptual-sRGB PNG; invalid
source, placement or resource limits emit no image. The binary requires `png`.
Run `python tools/check_material_relief_raster_backend.py -- <backend-command>`
for independent rectangular normal, color and coverage checks at scales
1, 1.25, 1.5, 2 and 3, deterministic output and atomic boundary rejection.
Raster evidence does not establish complete component or accepted native appearance.

## Preparation benchmark

Generate the [shared scene bundle](../../../conformance/scenes/README.md), then measure a named authored scene:

```sh
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-focus
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-rest
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-paint-focused
```

Pass an absolute bundle path: Cargo runs the benchmark from the package directory. The benchmark resolves the selected request before timing. It uses the bundle's physical origin, logical capture extent and samples per axis at device scales 1, 1.25, 2 and 3, rounding pixel dimensions upward. Each scale has one warmup and nine measured renders; output reports minimum, median and maximum milliseconds. Timing includes pixel allocation, paint evaluation and color integration, and excludes request resolution, buffer destruction, image encoding, toolkit upload and compositing. `cargo bench` uses the optimized bench profile; the target is excluded from ordinary tests. Run on an otherwise idle machine and record the compiler, hardware and bundle with results. Timing is measurement evidence, not a portable CI threshold or proof of interactive readiness.

Current point sampling can be expensive even for a small surface. Prepare reusable images before exposing a state; measure preparation and upload before using this realization in an input-event path. The benchmark accepts separate surface/focus scenes and complete paint scenes from the same public bundle and does not require PNG support.

`resina-command-raster` uses the same viewport and sampling arguments as `resina-paint-raster`, with a [command paint request](../../../schemas/command-paint-request.schema.json). It renders the complete headless result through `render_lit_control_paint`, which samples the [lit control body](../../../spec/88-lit-control-body.md) and its separated focus ring; command state and light rules remain in portable resolution. `python tools/check_lit_body_raster_backend.py -- <raster-command>` compares every pixel at device scales 1, 1.25, 1.5, 2 and 3, with one and three samples per axis, against independent lit-body and focus-ring arithmetic: RGB within one step, exact alpha, and tie-bracketed bounds for samples on a swept boundary.

With at least three samples per axis, `render_lit_control_paint` paints a pixel without sampling only where its sample box is proven uniform: inside the content bounds or the certified flat relief plateau it takes the body color; separated from the silhouette by more than the arithmetic tolerance, beyond its bounds, a supporting line or a corner arc's circle within that arc's sector, it is clear outside the focus ring and its hole and takes the ring color between the ring's contours. Every other pixel evaluates the lit body sampler, so the image is byte-identical to point sampling. `render_lit_control_paint_reusing` also takes a `ReliefReuse`, which keeps the face's relief and edge-interior terms, relief samples and flat-box certifications between renders of the same face, such as motion frames, keyed by their exact bits; a kept term is the one a new evaluation returns.

`resina-toggle-part-motion-raster` accepts the existing sampled Toggle part request
from specification 57 and the same explicit viewport/sampling arguments as the
other paint binaries. It resolves the guarded sample through the reference, then
renders its complete body/navigation paint with the shared sampler. Success emits
one RGBA8 sRGB PNG; invalid source, placement or resource limits emit no image.
It requires the `png` feature. This supplies the native Toggle frame oracle;
it does not schedule frames, place a component label, choose motion defaults or
publish accessibility.
