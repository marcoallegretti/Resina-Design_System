# Quickshell paint and measurement (0.0.1)

This Shell backend realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as a complete static Qt Quick `Shape`, and complete [body/navigation paint IR](../../../spec/39-surface-paint-ir.md) as prepared Qt Quick image paint. The public `prepare_lit_control_paint` and `render_lit_control_paint` functions realize a [lit control body](../../../spec/88-lit-control-body.md) and its focus ring the same way, with placement and sampling guards checked before pixels are sampled; `render_lit_control_paint` takes a `ReliefReuse` that keeps the face's relief terms between renders, such as motion frames, without changing any pixel. It also supplies native label measurement and drawing for the portable layout policy. It is independent of GUIdo and the Web SVG mapper. The integrated command profiles below connect these foundations to native interaction and accessibility. Calibrated motion and complete Shell conformance remain unqualified.

The separately locked [Qt runtime adapter](../resina-qt-runtime/README.md) supplies
a Rust-owned command lifecycle QObject and native invocation evidence. The integrated profile now supplies native command input/accessibility in a standalone Qt host. Released Quickshell 0.3.2 hosting is verified by the runtime README's [authored updates](../resina-qt-runtime/README.md#released-quickshell-authored-updates), [lifecycle](../resina-qt-runtime/README.md#compositor-delivered-command-lifecycle), [accessibility](../resina-qt-runtime/README.md#released-quickshell-command-accessibility) and [motion](../resina-qt-runtime/README.md#released-quickshell-command-motion) sections.

## Coherent command drawing

[`ResinaCommandView.qml`](qml/ResinaCommandView.qml) prepares Rust-checked body,
complete label, target and semantic data as one native drawing publication. It
uses the compiled [Qt runtime](../resina-qt-runtime/README.md#coherent-native-command-view),
waits for local image readiness, prepares complete glyphs with the checked body
foreground, and withdraws all visible channels on replacement/failure/context
loss. Its API, coordinate/lifetime requirements and executed capture matrix are
specified in that runtime README. It is decorative, ignored by accessibility and
has no native gesture handler. It does not implement a Button or animation.

[`ResinaSurfaceView.qml`](qml/ResinaSurfaceView.qml) realizes a complete surface
paint request live through the same compiled runtime, at its actual scene origin
and pixel density, and prepares again when either changes. It exposes the guarded
foreground colour and the bounding rectangle of the content region for consumer
content placed on the surface.
Its API and executed capture matrix are in the
[runtime README](../resina-qt-runtime/README.md#live-native-surface-view). It is
decorative and owns no input, accessibility, window or layer policy.

[`ResinaCommandKeyboard.qml`](qml/ResinaCommandKeyboard.qml) owns a native focus
item and the Rust command lifecycle for keyboard delivery. Its
[runtime contract and evidence](../resina-qt-runtime/README.md#native-command-keyboard-owner)
cover repeat filtering, modified termination, interruption and lifetime.
Pointer targeting, complete component composition and AT publication remain
separate requirements.

[`ResinaCommandInput.qml`](qml/ResinaCommandInput.qml) combines that keyboard
owner with native primary mouse targeting/capture through the same Rust
activation lifecycle. Its [native input contract](../resina-qt-runtime/README.md#native-command-pointer-owner)
defines explicit stable reservations, coordinate mapping, capture cancellation,
read-only native hover observations and executed Wayland evidence. Supply its
current hover and committed activation together when preparing command drawing.
It does not automatically prepare paint or publish an accessible Button.

## Integrated static command control

[`ResinaCommandControl.qml`](qml/ResinaCommandControl.qml) composes native input,
Rust-validated complete content/body/target/semantics and a singular native
accessible Button. Supply a complete snapshot request template, an explicit
`disabledFocusable: false` policy and raster sample count. Fonts must already be
registered. Consumers connect `activated` to product effects. The
[runtime API and evidence](../resina-qt-runtime/README.md#integrated-static-command-control)
define coherent readiness, cancellation, feedback publication and native
AT-SPI verification. The default profile realizes static feedback endpoints. Explicit `motionChannels`
selects the [sampled native profile](../resina-qt-runtime/README.md#integrated-sampled-command-frames),
with Qt scheduling and Rust-owned trajectory/paint guards. Fixture motion is
verified separately. Released Quickshell hosting is verified in the
[runtime README](../resina-qt-runtime/README.md#released-quickshell-authored-updates)'s released-host
sections. The [Lab clock shell](../../../lab/quickshell/clock-shell/README.md)
samples Command motion with the calibrated Elastomer profiles of
[material response calibration](../../../spec/80-material-response-calibration.md);
backend qualification of those profiles remains a separate contract.
`disabledFocusable: true` is rejected with a diagnostic, preserving the existing
strict discoverable-disabled fixture and its known Qt mismatch.

## Native paint image mapping

Complete paint images use integer local pixel dimensions and an explicit inverse
device-density `Scale`. Qt 6.11.2's software image-node dirty bounds round the
local rectangle before transformation; a fractional logical rectangle can lose
the final sampled edge column. Integer image geometry preserves those pixels
without changing PNG sampling or portable bounds. Native float32 transformed
extents must retain the existing 1/1024 logical-pixel precision budget or fail.
Actual Command and moving Toggle readback at fractional density verifies this
mapping against the unchanged public CPU raster oracle.

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
It returns `{width, height, nativeText, paragraphs, face, fontMapping,
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

The selected primary family and mapped native pixel size must match for every
nonempty paragraph. [Qt's empty-layout path](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquicktext.cpp#L430-L455)
uses current native font metrics without refreshing `fontInfo`; blank entries
retain those metrics, while nonempty entries validate the actual selected face.
`fontInfo.weight` cannot detect a substituted weight: on Qt 6.11.2 it repeats
the requested weight even when only a Regular master is registered, while
`fontInfo.styleName` names the master Qt matched. `face` therefore publishes
`{family, style}` of that master, and every nonempty paragraph must resolve the
same one. The font mapping owns the registered masters and must reject a face
whose own weight differs from the requested weight; the label checks do this
against the pinned Inter masters. A style name identifies a file only when no
other visible face of the family has that style, so the label checks also list
the fontconfig-visible faces of the family in the probe's environment and fail
unless every upright, normal-width face at weight 400, 500 or 600 is
byte-identical to that weight's pinned master. `measureJson`, the production
entry point of the Rust Command path, returns only `{kind, width, height}`: it
drops `face`, so production measurement cannot detect a substituted weight. Only
the label checks, which call `measure`, verify it. Per-glyph fallback availability remains the
application's explicit font mapping policy.
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
python tools/check_quickshell_label_runtime.py --font-dir /usr/share/fonts/opentype/inter
python tools/check_quickshell_label_runtime.py --font-dir /usr/share/fonts/opentype/inter --platform wayland
```

The checker takes the directory holding the pinned Inter static masters
(`tools/conformance_font.json`; `python tools/conformance_font.py fetch --output DIR`
provides them where no package does). The native probe registers all three and
checks 27 labels, four text scales and four signed spacing values on each
requested actual device scale (by default 0.5, 1, 1.25, 2 and 1.3). It includes
100.06% text scaling and the authored 0.15 px wide tracking, alongside
Latin, expanded German, Arabic, 100/150/200% string-length fixtures, literal
markup, LF/CRLF/CR, VT/FF/NEL, line separators and leading/trailing/consecutive
blank lines and mixed-script/blank paragraphs. A separate native Text oracle
checks Arabic, neutral and Latin paragraph extents and natural alignment;
each fitted paragraph also matches its isolated native measurement. The probe
checks complete line heights, contiguous paragraph offsets, wide-word reflow,
spacing effects, weights with blank paragraphs, fractional line heights and
13 invalid/unsupported inputs, including failure after a valid paragraph and
recovery afterward. Before the runs the checker lists the Inter faces
fontconfig makes visible in the probe's environment and fails on an upright,
normal-width face at weight 400, 500 or 600 that is not byte-identical to that
weight's pinned master. At each scale it records the master that weights 400, 500
and 600 resolve to (family, style, weight, file and SHA-256) and fails unless
they are the three distinct pinned masters. A second run registers only the
Regular master, with a fontconfig file that lists no font directories, and must
report weight 500 as a substitution. The probe also measures the digits 0 to 9
in the Medium master at 20 px, once with `font.features: {"tnum": 1}` and once
without, and publishes both as
[digit-advance records](../../../schemas/digit-advance-record.schema.json). With
`tnum` the advances must satisfy the
[numeric digit rule](../../../spec/11-typography.md#numeric-digits); the face's
proportional default digits must fail it. Evidence recorded on Quickshell 0.3.1: the extended matrix
passed software/offscreen with unmodified Quickshell 0.3.1 and Qt 6.11.2. Earlier integer-size evidence passed on
software in the tested Wayland session; the extended matrix has not been rerun there.
CI runs this checker with the released Quickshell host at device scales 1, 1.25
and 2 on offscreen software. Other fonts and compositors remain unverified. The
host comes from `PATH` or `--quickshell` without a version check. The checker
itself is covered by portable CI tests that reject incomplete, duplicate,
failed, wrong-scale and substituted-master evidence.

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
python tools/check_quickshell_label_render_runtime.py --font-dir /usr/share/fonts/opentype/inter --output-dir target/qt-label-software
python tools/check_quickshell_label_render_runtime.py --font-dir /usr/share/fonts/opentype/inter --scene-graph rhi --scales 0.5 1 1.25 2 1.3 --output-dir target/qt-label-rhi
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

Evidence recorded on Quickshell 0.3.1: the extended captures passed
software/offscreen and OpenGL RHI on Mesa software
graphics on unmodified Quickshell 0.3.1 / Qt 6.11.2 at scales 1, 1.25, 2 and 1.3.
The probe asserts the actual `GraphicsInfo.api`, rejecting renderer substitution.
Prior integer-size captures passed
software/Wayland and OpenGL RHI on Mesa software graphics; those prior captures
did not assert the reference's selected font, so their font-mapping evidence is
limited. Software `grabToImage` at 0.5 produces
an invalid partially black capture and fails the checker when requested.
Each run also records the master that its weight-400 measurement resolved and
rejects any other. CI runs the software captures with the released Quickshell
host at device scales 1, 1.25 and 2. Live software presentation at 0.5,
hardware GPUs, arbitrary font mapping, other fonts and complete
component/accessibility conformance remain unverified. The host comes from
`PATH` or `--quickshell` without a version check.

## Complete opaque paint

`resina_qml::render_surface_paint(&SurfacePaintIr, placement, device_scale, samples_per_axis)` prepares the full body and optional focus ring on one device grid. It uses the existing bounded CPU raster backend, including circular contours, physical extrusion, region ordering, directional normal-weighted highlights and linear-light coverage integration. This is shared compiled rasterization, not an independent QML geometry evaluator or a GUIdo dependency. No renderer concepts enter normative IR.

The result is standalone QML with one synchronous `Image` containing a percent-encoded sRGB RGBA8 PNG. It requires neither an external image file nor a JavaScript sampler. The CPU backend's pixel/sample limits and the Qt coordinate precision guard apply before output. PNG encoding also completes before publication; invalid scale, placement, sampling, bounds or resource use produces an error and no component.

`paintOriginX` and `paintOriginY` already include the supplied parent placement, rounded outward to the device grid. Place the item at those coordinates without adding placement a second time. Its implicit dimensions reserve the complete pixel footprint, including negative focus origins and less than one pixel of transparent padding. Changing the root item's dimensions does not scale the image. Keep ancestors from clipping it, preserve its scale and translation, and publish it only when `paintReady` is true. `paintError` reports native PNG decoding failure; a pending image is not ready paint. The paint is decorative and owns no input or accessible semantics.

Preparation requires the actual destination device scale: the `devicePixelRatio` of the `QQuickWindow` that hosts the item (`Window.window.devicePixelRatio`, a notifying property since Qt 6.11), read once that window is exposed. Qt Quick's attached `Screen.devicePixelRatio` is not that value. Under fractional output scaling it reports the output's integer scale (2 for an output at 1.25), and it does not notify when the ratio of the same screen changes. `preparedDeviceScale` records the value used. Prepare a complete replacement when placement, device scale or resolved paint changes; changing the display scale cannot regenerate an exported static image. Schedule CPU preparation outside the interactive frame loop and reuse the result. This path does not claim animated, live monitor-transfer or component-state integration.

```sh
cargo run --release -p resina-qml --bin resina-paint-qml -- request.json 1.25 4 44.3 26.1
```

The command accepts one strict UTF-8 surface paint request from a file or `-`, up to 1 MiB, followed by device scale, samples per axis and both placement coordinates. Success exits 0 with complete QML and no diagnostics. Input/preparation failure exits 1 with a diagnostic and no QML; usage errors exit 2. It preserves authored theme and capability resolution rather than forcing opacity for unsupported treatments. No external package or package version was added to the lockfile.

The [native paint probe](../../../conformance/quickshell/paint-probe.qml) checks actual scale, readiness and decorative semantics before capture. The [runtime checker](../../../tools/check_quickshell_paint_runtime.py) derives the tight prepared viewport from reference IR and compares every captured alpha, opaque RGB and premultiplied RGB channel with the compiled raster result within one byte. Matching the prepared viewport matters at exact boundaries: algebraically equivalent larger viewports can round sample positions differently. This checks native placement/upload/compositing, not independently implemented geometry or accessibility.

```sh
cargo build --release -p resina-qml -p resina-raster -p resina-resolver --bins --locked
python tools/check_quickshell_paint_runtime.py \
  --backend target/release/resina-paint-qml --raster target/release/resina-paint-raster \
  --resolver target/release/resina-surface-paint --scenes target/material-scenes.json \
  --output-dir target/quickshell-paint-proof --scales 1 1.25 2
```

Generate the catalog using the existing [material scene tooling](../../../tools/material_scenarios.py), install the pinned render-test requirements below, and use a fresh output directory. Evidence recorded on Quickshell 0.3.1: on local Quickshell 0.3.1 / Qt 6.11.2, all 16 Light/Dark, Cast/Frost/Elastomer/Gel and resting/focused scenes passed software/offscreen captures at scales 1.25 and 2, and graphics/offscreen captures at scales 0.5, 1.25 and 2 using OpenGL on Mesa software graphics (`--scene-graph rhi`). Software `grabToImage` at scale 0.5 clipped the capture and fails the checker; live presentation at that scale remains unverified. The checker retains that failure when explicitly requested. Software captures in the tested Wayland session also passed all 16 scenes at scales 1, 1.25 and 2 (`--platform wayland`). Hardware GPU, compositor diversity and runtime CI remain unverified. This checker is not run in CI and runs the `quickshell` found on `PATH` without a version check.

## Native command accessibility prerequisite

The [Qt availability fixture](../../../conformance/quickshell/availability-probe.qml)
and [native checker](../../../tools/check_qt_native_availability.py) measure the
actual Linux AT-SPI tree and invoke its advertised Press actions. The diagnostic
host defaults to pinned Qt 6.11.2 through PySide6. Setting
`RESINA_QUICKSHELL_HOST` together with `RESINA_QT_RUNTIME_MODULE` selects a
released Quickshell host instead; see the
[Quickshell hosting boundary](../resina-qt-runtime/README.md#quickshell-hosting-boundary). The
[`qt-runtime` CI job](../../../.github/workflows/reference.yml) runs both hosts
with `--known-deviation`. Reports
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
python tools/check_qt_native_availability.py --known-deviation
python tools/check_qt_native_availability.py
```

The checker owns a fresh D-Bus session, bounds execution and rejects missing or
duplicate nodes/actions, invalid state replies, failed action delivery and
incorrect callback records. `--report-only` permits measured semantic mismatches;
execution failures still exit nonzero. `--known-deviation` exits successfully
only when the mismatch list is exactly one entry: the Disabled command is not
focusable. An additional, changed or missing mismatch, including a fully
conformant result, fails. The strict command retains the existing
discoverable-disabled expectation and currently fails it.

Executed on Qt 6.11.2 with the native X11 platform, software scene graph and Xvfb:
enabled commands publish enabled/sensitive/focusable; disabled commands publish
neither enabled nor sensitive. Both disabled policies publish non-focusable,
including the fixture that requests discoverable focus. Qt explicitly
[clears focusability when the item is disabled](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/accessible/qaccessiblequickitem.cpp#L451-L460).
The excluded-disabled policy matches; the discoverable-disabled policy remains
unsupported. Native accessibility evidence for the Qt hosts therefore covers the
excluded-disabled policy only. Resina semantics have not been changed to
accommodate this limit.

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
keyboard/pointer input, screen-reader operation or Wayland behavior. The fixture is diagnostic and supplies no
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

The command reads a strict UTF-8 focus IR request from a file or `-` (stdin), up to 1 MiB. It resolves and renders completely before writing QML. Success exits 0 without diagnostics; invalid input or unsupported precision exits 1 with diagnostics and no output; usage errors exit 2. The external checker compares the full native profile and both translated boundaries against all public focus cases, rejects malformed input, and checks repeated output. These checks run in Linux and Windows CI without a Qt runtime.

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

Use a fresh image path for every run. Repeat with `QT_QUICK_BACKEND=rhi QSG_RHI_BACKEND=opengl` for the graphics path and with `QT_QPA_PLATFORM=wayland QT_QUICK_BACKEND=software` on an actual Wayland session. Install Quickshell using its [upstream instructions](https://quickshell.org/docs/v0.3.2/guide/install-setup/), keeping its private Qt ABI aligned with the installed Qt release. Native captures and containment passed locally on Quickshell 0.3.1 with Qt 6.11.2: software/offscreen, curve/OpenGL on Mesa software graphics, and software in the tested Wayland session. Hardware GPU, compositor diversity and runtime CI remain unverified.

The runtime checker requires a fresh, fully decoded 160 by 128 PNG at four capture pixels per logical px. An independent distance-to-rectangle oracle checks 17,696 off-boundary pixels: the ring interior must be opaque, and the hole and exterior must be clear. Every painted pixel must have the baseline's white pigment. A half-logical-px exclusion around each boundary allows native antialiasing and curve approximation; the alpha bounds must still preserve the complete expected footprint, and integrated alpha coverage must be within 1 logical px² of the analytic ring area.

These are quality checks for this fixed baseline, not a normative geometry tolerance or proof of every boundary pixel. The generated cue passed these pixel checks on the three local rendering paths listed above. Regression tests reject blank, filled, shifted, clipped, incorrectly colored, faded and malformed captures, including boundary damage that escapes the off-boundary oracle. This primitive alone does not establish WCAG conformance, navigation behavior, complete state rendering or perceptual equivalence with other backends.

## Integrated static Toggle

`ResinaToggleControl.qml` composes the existing binary Toggle contracts into
native input, complete track/thumb/label drawing, stable targeting and a single
native Switch. Supply explicit authored layout/paint/content/reservation inputs;
use `setChecked(boolean)` for external selection and `activated(checked)` for
product effects. The [runtime contract](../resina-qt-runtime/README.md#integrated-static-toggle-control)
defines strict preparation, checked replacement during a hold, excluding-disabled
focus policy and acceptance checks. Static feedback is integrated; no default
motion or material calibration is introduced.

Native Switch qualification depends on the Qt build. Standard PySide6 6.11.2
publishes CheckBox and fails the strict test. An isolated, unmodified Qt 6.11.2
build against AT-SPI 2.62 headers demonstrated Switch and actual native Toggle
action through the Rust adapter. This does not make Resina own a toolkit
distribution. Unmodified Quickshell 0.3.2 removes the separate 0.3.1
accessibility-bootstrap defect; the remaining Switch-role limitation comes from
the stock Qt 6.11.2 AT-SPI build, not Quickshell.
The [decision on #186](https://github.com/marcoallegretti/Resina-Design_System/issues/186) accepts the CheckBox role on
stock Qt as a Qt-owned limitation.

The Toggle's optional explicit `motionChannels` profile connects checked travel
and independent track/thumb interaction response to the same complete native
component. Its [sampled frame contract](../resina-qt-runtime/README.md#integrated-sampled-toggle-frames)
defines all required raw channels, public native scheduling, retargeting,
coherent buffered publication, per-sample guards and native verification. The
static profile remains the default. Checked semantics and selected roles are
current throughout travel; fixture dynamics do not establish material calibration.

## Native Slider accessibility bridge

`qml/ResinaSliderAccessibility.qml` wraps the Resina-owned public Qt accessibility
factory. Its continuous-value profile maps qualified Core Slider semantics and
routes real native adjustment proposals through Rust guards. The composing
presentation owner owns complete frame readiness, live edit adoption and
publication before consumer effects. This is an accessibility bridge; complete
Slider paint and pointer/keyboard editing are not supplied. Unmodified Qt 6.11.2
still omits AT-SPI `Value.Text`. See the [runtime API, lifetime rules and actual
native evidence](../resina-qt-runtime/README.md#continuous-slider-accessibility-bridge)
before making a component or native accessibility qualification claim.

## Material-lit relief face

`prepare_material_relief_paint` and `render_material_relief_paint` consume the
resolved shared `MaterialReliefPaint`. They retain its physical light, exact
exterior edge, uniform content reservation and foreground guards. They neither
select appearance nor derive another light law. The existing bounded CPU raster
supplies pixels; Qt Quick realizes the decorative image.

Preparation applies the existing binary32 placement budget, complete device-grid
viewport, inverse-density image transform and sampling/resource guards. Returned
paint origins already include parent placement. Keep the emitted image at its
prepared dimensions and device scale; filtering and asynchronous replacement
remain disabled. Readiness and decoding errors are explicit. This face primitive
owns no text, input, focus or accessible component semantics; the composing
component must prepare and verify those independently.

```sh
cargo build -p resina-qml --bin resina-material-relief-qml --locked
python tools/check_material_relief_qml_backend.py -- target/debug/resina-material-relief-qml
```

The command accepts the existing complete material-relief request followed by
device scale, samples per axis and parent x/y. It preserves original source
through strict shared admission, accepts file or stdin up to 1 MiB, and returns
only a complete QML component. Success, invalid input and usage retain status
0/1/2. No Resina schema, IR, capability or light semantics change.

The external checker computes rectangular relief normals, bounded light and
sampled pixels independently. It verifies five scales, fractional placement,
four pigment families, flat/convex/recessed profiles, source-sign controls,
strict negative cases, complete embedded PNGs and native profile metadata.
For actual Quickshell drawing, supply a scale-1 Wayland session and a fresh
output directory:

```sh
python tools/check_material_relief_qml_backend.py --native-output-dir target/material-relief-native -- target/debug/resina-material-relief-qml
```

The native mode records each original request, component, log and capture. It
requires the actual native device scale to match each requested scale and
compares the full captured canvas against independently computed pixels.
This is realization evidence, not acceptance of component appearance,
interaction/disabled identification, content shaping, accessibility or a release.
