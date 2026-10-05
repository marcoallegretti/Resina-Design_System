# Slint runtime paint

This application backend turns validated `SurfacePaintIr` into a runtime Slint
image with its final logical placement and extent. It uses the same bounded CPU
sampler as the other backends, combining the material body and navigation ring
before publication. No generated source, PNG encoding or component compilation
is needed when paint changes.

The crate is an independent workspace, like the GUIdo adapter. Slint 1.18.1 is
pinned to revision `372cf0ee5577c3dfec309a45e7b778ba4e81b734`; its runtime
dependencies stay outside the portable reference workspace. The consuming
application selects its windowing backend and renderer.

## Publication

Call `prepare_surface_paint(&ir, placement, window.scale_factor(), samples)`
outside frame handling. `PreparedPaint` is `Send + Sync` and can be prepared on
a worker. On the UI thread, call `into_snapshot()` and publish its image, origin
and logical extent through **one application struct property**. The compiled
[publication test](tests/publication.rs) defines that struct and the corresponding
Slint image bindings, and performs the actual single setter operation.

Slint's `Image` is not `Send`. Transfer `PreparedPaint`, then construct the
snapshot in an `invoke_from_event_loop` closure with the application's weak
component handle. An application using asynchronous preparation must discard
obsolete results when its state, placement or actual window scale changes.
Reprepare for those changes; stretching an old image does not preserve device
grid sampling. The returned origin already includes placement, so do not add it
again. Bind the decorative image to the snapshot's fixed extent, use
`image-fit: fill`, `image-rendering: pixelated` and `accessible-role: none`, and
reserve its complete body and focus coverage in the component layout.

The sampler emits straight RGBA. The adapter copies it once into Slint's shared
buffer and premultiplies RGB with round-to-nearest, retaining alpha exactly.
`Image::from_rgba8_premultiplied` matches this representation. Slint's straight
RGBA software path truncates premultiplication; the explicit conversion avoids
accumulating that extra rounding error. This is backend storage conversion;
reference pixels and IR remain unchanged.

Invalid scales, unrepresentable bounds, native binary32 coordinate error above
1/1024 logical px, cumulative corner error and sampler resource limits fail
before publishing a snapshot. Raster errors retain their diagnostic source.
The raster's pixel and sample budgets also bound the copied toolkit buffer;
Slint's allocation API itself does not expose recoverable allocation failure.

## Conformance

```sh
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
RESINA_SCENES="$PWD/target/material-scenes.json" cargo test --manifest-path backends/slint/resina-slint-runtime/Cargo.toml --features testing --locked
cargo clippy --manifest-path backends/slint/resina-slint-runtime/Cargo.toml --all-targets --features testing --locked -- -D warnings
```

The `testing` feature selects Slint's software renderer for a compiled component
on its public `MinimalSoftwareWindow` platform. The required test publishes all
16 authored light/dark material scenes at actual scales 0.5, 1, 1.25, 2 and 1.3,
changing material, rest/focus state and fractional placement in the same window.
Every captured pixel must match an independently placed CPU reference over the
authored surrounding color within one RGB byte, with exact opaque alpha and
dimensions. Unit tests verify worker transfer, negative focus reservation, exact
premultiplied bytes, invalid input, resource bounds and cumulative native
coordinate precision rejection.

For visual inspection, set `RESINA_CAPTURE_DIR` to an existing fresh directory.
The test writes raw RGBA captures with dimensions in each filename using
exclusive creation. Existing captures are never overwritten. CI runs the
compiled software test on Linux and Windows.

This verifies runtime paint publication through Slint's compiled software
renderer. OS window management, GPU rendering, input, text, motion and assistive
technology need separate application integration evidence. Component semantics
and interaction belong to the application, not this decorative image adapter.
