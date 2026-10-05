# Quickshell paint realization (0.1.0)

This Shell backend realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as a complete static Qt Quick `Shape`, and complete [body/navigation paint IR](../../../spec/39-surface-paint-ir.md) as prepared Qt Quick image paint. It is independent of GUIdo and the Web SVG mapper. It does not implement a component, interaction, layout, animation, or a complete Shell conformance claim.

## Complete opaque paint

`resina_qml::render_surface_paint(&SurfacePaintIr, placement, device_scale, samples_per_axis)` prepares the full body and optional focus ring on one device grid. It uses the existing bounded CPU raster backend, including circular contours, physical extrusion, region ordering, directional normal-weighted highlights and linear-light coverage integration. This is shared compiled rasterization, not an independent QML geometry evaluator or a GUIdo dependency. No renderer concepts enter normative IR.

The result is standalone QML with one synchronous `Image` containing a percent-encoded sRGB RGBA8 PNG. It requires neither an external image file nor a JavaScript sampler. The CPU backend's pixel/sample limits and the Qt coordinate precision guard apply before output. PNG encoding also completes before publication; invalid scale, placement, sampling, bounds or resource use produces an error and no component.

`paintOriginX` and `paintOriginY` already include the supplied parent placement, rounded outward to the device grid. Place the item at those coordinates without adding placement a second time. Its implicit dimensions reserve the complete pixel footprint, including negative focus origins and less than one pixel of transparent padding. Changing the root item's dimensions does not scale the image. Keep ancestors from clipping it, preserve its scale and translation, and publish it only when `paintReady` is true. The paint is decorative and owns no input or accessible semantics.

Preparation requires the actual destination device scale, available through Qt Quick's attached `Screen.devicePixelRatio` once the item is associated with its display. `preparedDeviceScale` records that value. Prepare a complete replacement when placement, device scale or resolved paint changes; changing the display scale cannot regenerate an exported static image. Schedule CPU preparation outside the interactive frame loop and reuse the result. This path does not claim animated, live monitor-transfer or component-state integration.

```sh
cargo run --release -p resina-qml --bin resina-paint-qml -- request.json 1.25 4 44.3 26.1
```

The command accepts one strict UTF-8 surface paint request from a file or `-`, up to 1 MiB, followed by device scale, samples per axis and both placement coordinates. Success exits 0 with complete QML and no diagnostics. Input/preparation failure exits 1 with a diagnostic and no QML; usage errors exit 2. It preserves authored theme and capability resolution rather than forcing opacity for unsupported treatments. Package versions remain 0.1.0; no external package or package version was added to the lockfile.

The [native paint probe](../../../conformance/quickshell/paint-probe.qml) checks actual scale, readiness and decorative semantics before capture. The [runtime checker](../../../tools/check_quickshell_paint_runtime.py) derives the tight prepared viewport from reference IR and compares every captured alpha, opaque RGB and premultiplied RGB channel with the compiled raster result within one byte. Matching the prepared viewport matters at exact boundaries: algebraically equivalent larger viewports can round sample positions differently. This checks native placement/upload/compositing, not independently implemented geometry or accessibility.

```sh
cargo build --release -p resina-qml -p resina-raster -p resina-resolver --bins --locked
python tools/check_quickshell_paint_runtime.py \
  --backend target/release/resina-paint-qml --raster target/release/resina-paint-raster \
  --resolver target/release/resina-surface-paint --scenes target/material-scenes.json \
  --output-dir target/quickshell-paint-proof --scales 1 1.25 2
```

Generate the catalog using the existing [material scene tooling](../../../tools/material_scenarios.py), install the pinned render-test requirements below, and use a fresh output directory. On local Quickshell 0.3.1 / Qt 6.11.2, all 16 Light/Dark, Cast/Frost/Elastomer/Gel and resting/focused scenes passed software/offscreen captures at scales 1.25 and 2, and graphics/offscreen captures at scales 0.5, 1.25 and 2 using OpenGL on Mesa software graphics (`--scene-graph rhi`). Software `grabToImage` at scale 0.5 clipped the capture and fails the checker; live presentation at that scale remains unverified. The checker retains that failure when explicitly requested. Software/Wayland captures on WSLg also passed all 16 scenes at scales 1, 1.25 and 2 (`--platform wayland`). Hardware GPU, compositor diversity and runtime CI remain unverified.

## Native focus contours

`resina_qml::render_focus(&FocusIndicatorIr)` emits standalone QML using native `PathMove`, `PathLine`, and `PathArc` objects in one `ShapePath`. Both boundaries use the resolved physical side footprint. A closed compound fill with `OddEvenFill` preserves the hole; there is no toolkit stroke, image embedding, SVG intermediary, mask, blur, or source-text injection. The resolved sRGB pigment is passed to `Qt.rgba` without premature byte quantization. Only constants and validated numerical values are emitted.

The item reserves the full outer paint bounds. Coordinates are local to those bounds; `paintOriginX` and `paintOriginY` retain their physical origin relative to the surface. Place the cue at `surface.x + cue.paintOriginX`, `surface.y + cue.paintOriginY`. Preserve the implicit dimensions, transform it consistently with the surface, and keep its ancestors from clipping or occluding the ring. Changing the item's width or height does not scale its geometry (`Shape.NoResize`). The item creates no pointer handler or keyboard target and is ignored by accessibility; the owning component must expose its own accessible semantics and focus behavior.

The generated QML requires Qt Quick **6.7 or later**. It requests the [curve renderer](https://doc.qt.io/qt-6/qml-qtquick-shapes-shape.html); Qt selects the software shape renderer when the scene graph uses its `software` backend, and falls back to its supported default if the requested renderer is unavailable. No advanced effect is necessary for the navigation cue. The [native arc](https://doc.qt.io/qt-6/qml-qtquick-patharc.html) mapping follows the canonical clockwise, small, axis-aligned arcs. This is a toolkit realization; Qt's curve approximation and rasterization are distinct from exact headless geometry.

Before returning output, the mapper checks every emitted coordinate, radius, paint origin and dimension against a binary32 rounding budget of **1/1024 logical px**. Nonfinite conversions and larger errors fail with `QmlError::CoordinatePrecision`; this backend limit does not narrow normative IR. Arbitrary caller placement, transforms, device scaling and compositing still require evaluation by the consumer.

## Commands and evidence

```sh
cargo run -p resina-qml --bin resina-focus-qml -- conformance/ir/focus-ir-request.json
python tools/check_focus_qml_backend.py -- target/debug/resina-focus-qml -
```

The command reads a strict UTF-8 focus IR request from a file or `-` (stdin), up to 1 MiB. It resolves and renders completely before writing QML. Success exits 0 without diagnostics; invalid input or unsupported precision exits 1 with diagnostics and no output; usage errors exit 2. Packages remain at 0.1.0. The external checker compares the full native profile and both translated boundaries against all public focus cases, rejects malformed input, and checks repeated output. These checks run in Linux and Windows CI without a Qt runtime.

The [baseline QML](../../../conformance/quickshell/focus-baseline.qml) represents the public 20 by 14 silhouette with gap 2 and width 2. The [runtime probe](../../../conformance/quickshell/focus-probe.qml) loads either that baseline or generated QML, checks the bounds and decorative semantics, compares 3,197 off-boundary points against an independent distance-to-rectangle oracle, and captures the native item. For example, from the repository root on Linux:

```sh
python -m pip install -r tools/requirements-render.txt
mkdir -p target/quickshell-proof
target/debug/resina-focus-qml conformance/ir/focus-ir-request.json > target/quickshell-proof/Focus.qml
python tools/check_quickshell_focus_runtime.py --image target/quickshell-proof/software.png -- env \
  QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  RESINA_FOCUS_QML="file://$PWD/target/quickshell-proof/Focus.qml" \
  RESINA_FOCUS_IMAGE="$PWD/target/quickshell-proof/software.png" \
  quickshell --no-color --path conformance/quickshell/focus-probe.qml
```

Use a fresh image path for every run. Repeat with `QT_QUICK_BACKEND=rhi QSG_RHI_BACKEND=opengl` for the graphics path and with `QT_QPA_PLATFORM=wayland QT_QUICK_BACKEND=software` on an actual Wayland session. Install Quickshell using its [upstream instructions](https://quickshell.org/docs/v0.3.0/guide/install-setup/), keeping its private Qt ABI aligned with the installed Qt release. Native captures and containment passed locally on Quickshell 0.3.1 with Qt 6.11.2: software/offscreen, curve/OpenGL on Mesa software graphics, and software/Wayland on WSLg. Hardware GPU, compositor diversity and runtime CI remain unverified.

The runtime checker requires a fresh, fully decoded 160 by 128 PNG at four capture pixels per logical px. An independent distance-to-rectangle oracle checks 17,696 off-boundary pixels: the ring interior must be opaque, and the hole and exterior must be clear. Every painted pixel must have the baseline's white pigment. A half-logical-px exclusion around each boundary allows native antialiasing and curve approximation; the alpha bounds must still preserve the complete expected footprint, and integrated alpha coverage must be within 1 logical px² of the analytic ring area.

These are quality checks for this fixed baseline, not a normative geometry tolerance or proof of every boundary pixel. The generated cue passed these pixel checks on the three local rendering paths listed above. Regression tests reject blank, filled, shifted, clipped, incorrectly colored, faded and malformed captures, including boundary damage that escapes the off-boundary oracle. This primitive alone does not establish WCAG conformance, navigation behavior, complete state rendering or perceptual equivalence with other backends.
