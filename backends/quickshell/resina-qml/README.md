# Quickshell paint realization (0.1.0)

This Shell backend realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as a complete static Qt Quick `Shape`, and complete [body/navigation paint IR](../../../spec/39-surface-paint-ir.md) as prepared Qt Quick image paint. It also supplies native label measurement for the portable layout policy. It is independent of GUIdo and the Web SVG mapper. It does not implement a complete control, interaction, animation, or a complete Shell conformance claim.

## Native label measurement

Import [ResinaLabelMeasure.qml](qml/ResinaLabelMeasure.qml) into the application's
QML module, or load it through a runtime `Loader`. Register the application's
fonts before use, for example by waiting for an explicit `FontLoader.Ready`.
The component is invisible, has no input target and is ignored by accessibility.
Call its synchronous `measure(text, family, pixelSize, weight, spacing,
lineHeight, maximumWidth)` method on the Qt UI thread for each natural/fitted
request from the [portable label policy](../../../spec/46-command-label-layout.md).
It returns `{width, height, nativeText}` or throws a diagnostic error;
an error publishes no extent. `maximumWidth` is explicitly `null` for natural
measurement or a finite positive logical wrapping constraint.
Both dimensions are logical pixels. Bind subsequent native drawing to the
returned `nativeText`: CRLF, CR, VT, FF and NEL hard breaks are converted to LF
so Qt preserves them, while U+2028 line separators remain intact. This follows
the mandatory break classes in [Unicode line breaking](https://www.unicode.org/reports/tr14/tr14-54.html).

Size and spacing are already text-scaled logical pixels. Weight is an integer
OpenType weight in 1..1000. `lineHeight` is the resolved font-size multiplier;
the component converts it to [Qt's fixed logical line height](https://doc.qt.io/qt-6.11/qml-qtquick-text.html#lineHeightMode-prop).
Device scale is supplied by the actual Qt window and must not be applied to
these inputs a second time. Complete native shaping uses plain text, centered
lines, word or glyph-boundary wrapping, fixed font size and no ellipsis or line
limit. `implicitHeight` preserves trailing blank lines that `contentHeight`
omits on Qt 6.11.2. Natural width is the native implicit width; fitted width is
the complete content width, checked against the unchanged offered constraint.

The selected primary family, pixel size and reported weight must match the
request. Qt's native font selection/synthesis supplies the weight; per-glyph
fallback availability remains the application's explicit font mapping policy.
Preserve that context, typography and wrapping constraint for drawing, allow
glyph overhang and invalidate measurements when the font environment changes.
Use `Text.QtRendering`, upright mixed-case text with no underline or strikeout,
matching the helper's explicit font traits and layout metrics.
This helper measures layout; native label drawing, ink, accessibility and a
Rust/QML product binding require their own evidence.

Native limits are explicit. [Qt's pixel-size property is integer-valued](https://doc.qt.io/qt-6.11/qml-qtquick-text.html#font.pixelSize-prop),
so fractional resolved font sizes fail rather than rounding or shrinking.
Using points does not provide arbitrary precision: [Qt 6.11.2 rounds the Text
font to half-point steps](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquicktext.cpp#L1630-L1634).
Continuous text-scale conformance remains open for this backend. Native letter
spacing conversion must retain the requested value within 1/1024 logical px.
Values outside [Qt's signed 26.6 fixed-point range](https://github.com/qt/qtbase/blob/v6.11.2/src/gui/painting/qfixed_p.h)
fail before native assignment; quantization is checked through the native
readout. U+009C fails because [Qt interprets it as a multi-length separator and
discards the suffix](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquicktext.cpp#L224-L229),
even with plain text and no ellipsis. U+2029 also fails: the native plain-text
layout ignores its paragraph break, and replacing it with a line separator
would lose independent paragraph direction semantics. A complete paragraph
layout owner is required before accepting it. These limits do not narrow IR.

```sh
python tools/check_quickshell_label_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf
python tools/check_quickshell_label_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf --platform wayland
```

The native probe checks 19 labels, three text scales and three signed spacing
values on each of five actual device scales (0.5, 1, 1.25, 2 and 1.3). It includes
Latin, expanded German, Arabic, 100/150/200% string-length fixtures, literal
markup, LF/CRLF/CR, VT/FF/NEL, line separators and leading/trailing/consecutive
blank lines. It independently checks explicit line heights, wide-word reflow,
spacing effects, weights, 13 invalid/unsupported inputs and recovery after
failure. All 855 measurement and 65 diagnostic cases passed locally on both
software/offscreen and
software/Wayland with Quickshell 0.3.1 / Qt 6.11.2 on WSLg. Runtime CI and other
fonts/compositors remain unverified. The checker itself is covered by portable
CI tests that reject incomplete, duplicate, failed and wrong-scale evidence.

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
