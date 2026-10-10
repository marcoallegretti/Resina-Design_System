# Quickshell paint and measurement (0.1.0)

This Shell backend realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as a complete static Qt Quick `Shape`, and complete [body/navigation paint IR](../../../spec/39-surface-paint-ir.md) as prepared Qt Quick image paint. It also supplies native label measurement and drawing for the portable layout policy. It is independent of GUIdo and the Web SVG mapper. It does not implement a complete control, interaction, animation, or a complete Shell conformance claim.

The separately locked [Qt runtime adapter](../resina-qt-runtime/README.md) supplies
a Rust-owned command lifecycle QObject and native invocation evidence. Complete
component composition and Quickshell hosting remain separate requirements.

## Native label measurement

`ResinaCommandLabel.qml` connects the native pair to Rust-owned complete content
layout through the [Qt runtime](../resina-qt-runtime/README.md#commandlabelresolver-and-native-content).
It resolves authored Label typography, measures natural and fitted text in Qt,
and prepares the resulting complete label box for drawing. It requires the
compiled `Resina.Runtime` plugin and an explicit registered font mapping.
Its color must come from guarded body paint after the composing owner validates
content containment; it does not create input or accessibility semantics.

Import [ResinaLabelMeasure.qml](qml/ResinaLabelMeasure.qml) into the application's
QML module, or load it through a runtime `Loader`. Register the application's
fonts before use, for example by waiting for an explicit `FontLoader.Ready`.
The component is invisible, has no input target and is ignored by accessibility.
Call its synchronous `measure(text, family, pixelSize, weight, spacing,
lineHeight, maximumWidth)` method on the Qt UI thread for each natural/fitted
request from the [portable label policy](../../../spec/46-command-label-layout.md).
It returns `{width, height, nativeText, paragraphs, fontMapping,
nativeLineHeight, nativeWrappingWidth}` or throws a diagnostic error;
an error publishes no extent. `maximumWidth` is explicitly `null` for natural
measurement or a finite positive logical wrapping constraint.
Both dimensions are logical pixels. CRLF, CR, VT, FF and NEL hard breaks are
converted to LF so Qt preserves them, while U+2028 line separators remain intact. This follows
the mandatory break classes in [Unicode line breaking](https://www.unicode.org/reports/tr14/tr14-54.html).

U+2029 separates independently shaped paragraphs, including empty leading,
trailing and consecutive paragraphs. Each `paragraphs` entry contains its
`nativeText`, complete `width` and `height`, and logical vertical offset `y`.
Draw each entry as a separate native Text item at that offset within the
resolved label box, using the same offered wrapping width for every entry and
centered lines. Do not replace
paragraph separators with line separators or submit the combined `nativeText`
to one Qt Text item: those paths lose independent paragraph direction.
Paragraph widths are measured advances, not permission to narrow the wrapping
box. Total width is their maximum; total height is their checked sum. Zero
advance is valid for a blank paragraph, while the complete nonblank label must
retain a positive advance. No artificial glyph supplies blank-line metrics.

Size and spacing are already text-scaled logical pixels. Weight is an integer
OpenType weight in 1..1000. `lineHeight` is the resolved font-size multiplier;
the component converts it to [Qt's fixed logical line height](https://doc.qt.io/qt-6.11/qml-qtquick-text.html#lineHeightMode-prop).
Device scale is supplied by the actual Qt window and must not be applied to
these inputs a second time. Complete native shaping uses plain text, centered
lines, word or glyph-boundary wrapping, fixed font size and no ellipsis or line
limit. `implicitHeight` preserves trailing blank lines that `contentHeight`
omits on Qt 6.11.2. Natural width is the native implicit width; fitted width is
the complete content width, checked against the unchanged offered constraint.

The selected primary family, mapped native pixel size and reported weight must
match for every nonempty paragraph. [Qt's empty-layout path](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquicktext.cpp#L430-L455)
uses current native font metrics without refreshing `fontInfo`; blank entries
retain those metrics, while nonempty entries validate the actual selected face.
Qt's native font selection/synthesis supplies the weight; per-glyph
fallback availability remains the application's explicit font mapping policy.
Preserve that context, typography and wrapping constraint for drawing, allow
glyph overhang and invalidate measurements when the font environment changes.
Use `Text.QtRendering`, upright mixed-case text with no underline or strikeout,
matching the helper's explicit font traits and layout metrics.
This helper measures layout; accessibility and a Rust/QML product binding
require their own evidence. The drawing counterpart is described below.

Native limits are explicit. [Qt's pixel-size property is integer-valued](https://doc.qt.io/qt-6.11/qml-qtquick-text.html#font.pixelSize-prop).
Integer sizes with representable spacing retain direct native layout. Otherwise
the pair uses an integer native size of `max(1024, ceil(pixelSize * 8))` and a
uniform inverse transform of `pixelSize / nativeSize`. Spacing, line height and
wrapping width reach native layout in those same units; complete advances and
heights return to logical units. Drawing uses exactly that mapping and transform.
This is a native representation of requested typography, not font shrinking to
fit content or another application of environment text scaling. It uses public
Qt Quick `Text` and item transforms, with no private text API or point-size
rounding.

The effective font-size transform and native spacing readout must preserve
requested values within the existing **1/1024 logical px** budget. Integer font
and [signed 26.6 spacing ranges](https://github.com/qt/qtbase/blob/v6.11.2/src/gui/painting/qfixed_p.h)
are checked before assignment. Transformed wrapping widths round downward to
the native fixed-point grid within that same budget; they never widen the
offered width. The portable label box retains its full offered width. Excessive
or collapsed conversions fail diagnostically. U+009C fails because [Qt interprets it as a multi-length separator and
discards the suffix](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquicktext.cpp#L224-L229),
even with plain text and no ellipsis. The paragraph owner avoids Qt's ignored
plain-text U+2029 break by using independent native layouts, consistent with
[Unicode paragraph direction](https://www.unicode.org/reports/tr9/#P1).
Native limits do not narrow IR.

```sh
python tools/check_quickshell_label_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf
python tools/check_quickshell_label_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf --platform wayland
```

The native probe checks 27 labels, four text scales and four signed spacing
values on each of five actual device scales (0.5, 1, 1.25, 2 and 1.3). It includes
100.06% text scaling and the authored 0.15 px wide tracking, alongside
Latin, expanded German, Arabic, 100/150/200% string-length fixtures, literal
markup, LF/CRLF/CR, VT/FF/NEL, line separators and leading/trailing/consecutive
blank lines and mixed-script/blank paragraphs. A separate native Text oracle
checks Arabic, neutral and Latin paragraph extents and natural alignment;
each fitted paragraph also matches its isolated native measurement. The probe
checks complete line heights, contiguous paragraph offsets, wide-word reflow,
spacing effects, weights with blank paragraphs, fractional line heights and
13 invalid/unsupported inputs, including failure after a valid paragraph and
recovery afterward. The extended matrix passed software/offscreen with unmodified
Quickshell 0.3.1 and Qt 6.11.2. Earlier integer-size evidence passed on
software/Wayland with WSLg; the extended matrix has not been rerun there.
Runtime CI and other
fonts/compositors remain unverified. The checker itself is covered by portable
CI tests that reject incomplete, duplicate, failed and wrong-scale evidence.

## Native label drawing

[ResinaLabel.qml](qml/ResinaLabel.qml) draws a previously resolved label using
the same native measurement context. Install it alongside `ResinaLabelMeasure.qml`
and register the selected fonts before calling `prepare(request)` on the Qt UI
thread. The request requires exactly these fields:

```text
text, family, pixelSize, weight, spacing, lineHeight,
wrappingWidth, height, color, x, y
```

Map content and resolved typography from validated command label IR. The font
family comes from the application's explicit font mapping. `wrappingWidth` and
`height` are the resolved label box dimensions; `x` and `y` place its physical
origin in the parent item's logical coordinates. `color` is four finite sRGB
channels in [0, 1], supplied after the composition owner's content contrast and
containment checks. The component does not parse IR or repeat Resina layout policy.

Preparation remeasures every complete paragraph at the unchanged offered width,
verifies the resolved complete height, and checks native coordinate precision
within 1/1024 logical px, including cumulative paragraph placement. It copies
the request before publishing the paragraph Text items. Later mutation of the
caller's object cannot change drawing. `paintReady` reports publication and
`paintError` reports failure; a rejected request throws and clears previous
drawing. `paintOriginX` and `paintOriginY` expose the prepared placement.
Each paragraph retains the prepared wrapping width rather than following a
later resize of the containing item. Native text is complete, centered and
unclipped. Input is disabled and accessibility ignores this decorative drawing;
the complete control owner must provide its own semantics and interaction.
Font environment changes require preparation again. Ancestor transforms,
clipping, font fallback, overhang background and guarded color remain the
composition owner's responsibility.

```sh
python tools/check_quickshell_label_render_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf --output-dir target/qt-label-software
python tools/check_quickshell_label_render_runtime.py --font /usr/share/fonts/truetype/DejaVuSans.ttf --scene-graph rhi --scales 0.5 1 1.25 2 1.3 --output-dir target/qt-label-rhi
```

Use fresh output directories. The probe compares 36 actual drawing/reference
capture pairs per device scale, including glyph overhang, three text scales,
expanded German, Arabic, independent mixed-script paragraphs, literal markup,
string-length expansion and blank paragraphs. Additional Latin, German, Arabic
and mixed-paragraph captures use 12.3, 16.01, 20.5 and 20.012 px sizing with
0.15 px tracking, including overhang from a 60.012 px `j`. It also checks request mutation,
nine rejected requests and recovery. References use independent native Text
items at the same physical position and validate their selected font. The font
asset has an unambiguous identifier rather than shadowing `Text.font`; a
reference that falls back to the ambient family is rejected. This avoids GPU rounding differences from
translating the reference across the viewport. Captures use logical item size:
[Qt applies the window's device scale itself](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquickitemgrabresult.cpp#L244-L280).
The checker rejects missing, stale, translucent, incorrectly sized or differing
captures and requires visible ink outside the `j` advance box. Portable tests
cover both image inputs and the one-byte channel comparison tolerance.

The extended captures passed software/offscreen and OpenGL RHI on Mesa software
graphics on unmodified Quickshell 0.3.1 / Qt 6.11.2 at scales 1, 1.25, 2 and 1.3.
The probe asserts the actual `GraphicsInfo.api`, rejecting renderer substitution.
Prior integer-size captures passed
software/Wayland and OpenGL RHI on Mesa software graphics; those prior captures
did not assert the reference's selected font, so their font-mapping evidence is
limited. Software `grabToImage` at 0.5 produces
an invalid partially black capture and fails the checker when requested.
Live software presentation at 0.5, hardware GPUs, runtime CI, arbitrary font
mapping, other fonts and complete component/accessibility conformance remain
unverified.

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

## Native command accessibility prerequisite

The [Qt availability fixture](../../../conformance/quickshell/availability-probe.qml)
and [native checker](../../../tools/check_qt_native_availability.py) measure the
actual Linux AT-SPI tree and invoke its advertised Press actions. The diagnostic
host uses pinned Qt 6.11.2 through PySide6; it does not run Quickshell. Reports
identify the default `reference-protocol` binding. Setting
`RESINA_QT_RUNTIME_MODULE` to the packaged runtime import root instead runs the
compiled Rust controller, reported as `rust-qt-runtime`; see its README above.
Install the native X11 or Wayland platform libraries and AT-SPI services, then run on a live
display (Xvfb is sufficient for this tree/action probe):

```sh
python -m pip install -r tools/requirements-qt-native.txt
python -m pip install -r tools/requirements-schema.txt
cargo build -p resina-resolver --bin resina-activation --locked
python tools/check_activation_backend.py -- target/debug/resina-activation -
python tools/check_qt_native_availability.py --report-only
python tools/check_qt_native_availability.py
```

The checker owns a fresh D-Bus session, bounds execution and rejects missing or
duplicate nodes/actions, invalid state replies, failed action delivery and
incorrect callback records. `--report-only` permits measured semantic mismatches;
execution failures still exit nonzero. The strict command retains the existing
discoverable-disabled expectation and currently fails it.

Executed on Qt 6.11.2 with the native X11 platform, software scene graph and Xvfb:
enabled commands publish enabled/sensitive/focusable; disabled commands publish
neither enabled nor sensitive. Both disabled policies publish non-focusable,
including the fixture that requests discoverable focus. Qt explicitly
[clears focusability when the item is disabled](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/accessible/qaccessiblequickitem.cpp#L451-L460).
The excluded-disabled policy matches; the discoverable-disabled policy remains
unsupported. Resina semantics have not been changed to accommodate this limit.

Native AT-SPI Press is delivered even to disabled items. The diagnostic host
routes each callback through the existing `resina-activation` public protocol,
validates its result schema and commits next state before counting its activation
intent. The fixture's explicit product behavior revokes availability after the
first activation through that same lifecycle. Disabled invocations produce no
activation, the enabled invocation activates once, and a subsequent invocation
produces no additional activation. The checker verifies the complete transition
results, committed state, intent count and native availability after revocation.
Set `RESINA_ACTIVATION_BACKEND` to the absolute path of another built reference
executable if using a different target directory. This subprocess-backed host
is a conformance diagnostic, not an interactive production binding. The fixture
uses Qt's `accessiblePressAction()` method dispatch rather than an attached
`onPressAction` signal, because Qt's Button role already advertises Press and the
attached signal produces a duplicate. These behaviors follow the pinned
[action implementation](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/accessible/qaccessiblequickitem.cpp#L515-L574).

This evidence establishes native AT-SPI invocation through the canonical
activation protocol and a live-availability guard requirement. It does not
establish an interactive production binding, complete component conformance,
keyboard/pointer input, screen-reader operation, Wayland behavior,
Quickshell hosting or runtime CI. The fixture is diagnostic and supplies no
production control or Resina appearance. Complete controls must map existing
normative snapshots and operations, compose verified paint/text/focus, and obtain
their own native integration evidence.

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
