# Quickshell focus realization (0.1.0)

This Shell backend slice realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as a complete static Qt Quick `Shape`. It is independent of GUIdo and the Web SVG mapper. It does not implement a component, surface body, interaction, layout, animation, or a complete Shell conformance claim.

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
