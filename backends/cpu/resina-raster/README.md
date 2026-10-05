# CPU paint realization (0.1.0)

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

## Preparation benchmark

Generate the [shared scene bundle](../../../conformance/scenes/README.md), then measure a named authored scene:

```sh
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-focus
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-rest
cargo bench -p resina-raster --no-default-features --bench paint -- /absolute/path/material-scenes.json light-cast-paint-focused
```

Pass an absolute bundle path: Cargo runs the benchmark from the package directory. The benchmark resolves the selected request before timing. It uses the bundle's physical origin, logical capture extent and samples per axis at device scales 1, 1.25, 2 and 3, rounding pixel dimensions upward. Each scale has one warmup and nine measured renders; output reports minimum, median and maximum milliseconds. Timing includes pixel allocation, paint evaluation and color integration, and excludes request resolution, buffer destruction, image encoding, toolkit upload and compositing. `cargo bench` uses the optimized bench profile; the target is excluded from ordinary tests. Run on an otherwise idle machine and record the compiler, hardware and bundle with results. Timing is measurement evidence, not a portable CI threshold or proof of interactive readiness.

Current point sampling can be expensive even for a small surface. Prepare reusable images before exposing a state; measure preparation and upload before using this realization in an input-event path. The benchmark accepts separate surface/focus scenes and complete paint scenes from the same public bundle and does not require PNG support.

`resina-command-raster` uses the same viewport and sampling arguments as `resina-paint-raster`, with a [command paint request](../../../schemas/command-paint-request.schema.json). It renders the complete headless result through the existing surface paint sampler; command state rules remain in portable resolution.
