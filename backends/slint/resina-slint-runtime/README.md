# Slint runtime paint and measurement

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

On Ubuntu, install `libfontconfig-dev` and `fonts-dejavu-core` before building
this crate. The locked native font dependency discovers Fontconfig
through `pkg-config`; runtime libraries or a font file alone are insufficient.

```sh
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
RESINA_SCENES="$PWD/target/material-scenes.json" RESINA_LABEL_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf cargo test --manifest-path backends/slint/resina-slint-runtime/Cargo.toml --features testing --locked
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
renderer. OS window management, GPU rendering, input, text drawing, motion and assistive
technology need separate application integration evidence. Component semantics
and interaction belong to the application, not this decorative image adapter.

## Native command label measurement

Enable the `native-text` feature for this API. It includes Slint's software
renderer, without selecting an OS windowing backend. Other application
renderers can coexist, but their measurement behavior needs separate evidence.
The pinned SDK's public `unstable-fontique-011` feature alone does not compile
without a renderer that supplies its shared font context. The tested software
configuration supplies that context; paint-only builds need neither this
feature nor its font query API.

Import `ResinaLabelMeasure` and `ResinaLabelRequest` from
[resina-label-measure.slint](resina-label-measure.slint) into the application's
compiled component. The measurement component is invisible and has no
accessibility role. It reports complete native layout advances; it does not
render a command or provide interaction.

Register the application's fonts before measurement through Slint's shared
Fontique collection. Construct `LabelMeasurer::new(window)` on the UI thread;
retain it across natural and fitted measurements so file-backed faces reuse
the native source cache. Call `prepare(family, input)` from the measurement
closure supplied to `resolve_command_label`. Copy the resulting fields into
one application `ResinaLabelRequest` property, setting `constrained` from
`maximum_width.is_some()` and `wrapping-width` to that width when present.
Read `measured-width`, `measured-height` and `minimum-required-width` from the
component, then pass all three to `NativeLabelMeasure::complete`. The compiled
[label test](tests/label.rs) demonstrates registration, publication and the
complete resolver closure.

The family must name a registered native family. Missing families and failed
face loading are diagnostic. Matching follows the pinned SDK's normal-style
weight selection; fractional weights are rejected because Slint's Text weight
property is an integer. Glyph fallback follows the shared native collection's
configured fallback families. The application must establish their availability
and retain that font context for both measurement and drawing. The adapter does
not establish universal script coverage or certify a particular substitution.

Slint's line-height factor multiplies the selected face's natural metrics,
whereas Resina's resolved line height multiplies font size. The adapter reads
the actual matched face, including synthesized variation coordinates, and
converts between those definitions. It uses the SDK's versioned public
`unstable-fontique-011` integration and the already pinned Skrifa 0.44.0.
These backend dependencies do not enter portable IR or resolution policy.

Resolved typography already includes text scaling. Font size, letter spacing,
line height and wrapping width must reach measurement and drawing without a
second scale adjustment. Binary32 conversion above 1/1024 logical px fails;
wrapping widths round inward. The native word minimum is checked before
accepting a fitted extent. A word exceeding that width fails explicitly rather
than reporting a false fit. Explicit paragraphs remain complete, with no
ellipsis, line limit or font shrinking.

The readouts must belong to the exact published request. Changing fonts,
typography, text, width or actual window scale invalidates prior measurement.
The caller owns this provenance and the same-context drawing requirement.

The measurement test covers seven complete labels, including expanded German,
Arabic and explicit line breaks, at three text scales, five actual window
scales and three spacing values (315 combinations), plus weight matching,
blank paragraphs, CRLF and invalid input. `RESINA_LABEL_FONT` is required; CI explicitly registers DejaVu
Sans on Linux and Arial on Windows. This proves layout measurement at those
tested contexts, not visual shaping fidelity or full script coverage.

**Drawing limitation:** the pinned SDK's
[shared text renderer](https://github.com/slint-ui/slint/blob/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/core/textlayout/sharedparley.rs#L195-L230)
clips `overflow: clip` to the Text rectangle, and its
[legacy software path](https://github.com/slint-ui/slint/blob/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/renderers/software/lib.rs#L2834-L2845)
also clips to Text bounds. The compiled
[ink probe](tests/label_ink.rs) confirms this behavior for a single centered
“j” at 96 logical px and device scale 1. It compares an advance-sized Text box
with a diagnostic box extended by 20 logical px on each side. The added width
and centered alignment preserve the glyph position. Every pixel within the
advance box must match, and every difference must be cropped left overhang.
Linux DejaVu Sans produced a 27 px advance with 16 cropped pixels; Windows
Arial produced a 22 px advance with 33 cropped pixels. These counts describe
the tested font files, not a universal threshold.

This test records the pinned SDK's known clipping capability; a future SDK
change that permits overhang must update this evidence and its assertions.
The wider box is a single-glyph diagnostic, not a production wrapping solution.
No native overhang comparison has established a conformant complete label
renderer. Using ellipsis or increasing the wrapping width would change the
command contract. Native label drawing remains an explicit application
backend limitation.
