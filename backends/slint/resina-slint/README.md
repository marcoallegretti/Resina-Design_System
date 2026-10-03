# Slint focus realization (0.1.0)

This application backend slice realizes validated [focus indicator IR](../../../spec/37-focus-indicator-ir.md) as an exported native Slint `ResinaFocusIndicator` component. It is independent of GUIdo, Qt Quick and the Web mapper. It implements the static navigation cue; component input, body appearance, layout, motion and accessibility behavior remain separate contracts.

`resina_slint::render_focus(&FocusIndicatorIr)` emits complete `.slint` source using `MoveTo`, `LineTo`, `ArcTo` and `Close` in one compound `Path`. The outer boundary is clockwise and the inner boundary counterclockwise. Nonzero filling realizes the specified outer-minus-inner set without a toolkit stroke, image, mask or SVG intermediary. Only validated numeric constants are emitted. The mapper has no Slint runtime dependency, and normative models and IR acquire no toolkit types.

Slint 1.18.1's software renderer [uses nonzero filling](https://github.com/slint-ui/slint/blob/v1.18.1/internal/renderers/software/path.rs), regardless of the declared path fill rule. A same-winding even-odd path therefore paints its hole in that renderer. Opposite winding with explicit nonzero filling is correct for both the specified Slint property and this software implementation. The [native path reference](https://docs.slint.dev/latest/docs/slint/reference/elements/path/) documents the arc and viewbox properties.

The component reserves its full outer paint bounds. `paint-origin-x` and `paint-origin-y` retain the physical origin relative to the surface: place the cue at the surface position plus these values. Transform the cue consistently with the surface. Its fixed viewbox and `fit: preserve` retain the geometry when its item dimensions change. Keep ancestors from clipping or occluding it. It has no input area, focus scope, event callback or animation; both elements are excluded from the accessibility tree. The owning interactive component supplies navigation and accessible semantics.

Slint [colors have eight-bit channels](https://docs.slint.dev/latest/docs/slint/reference/property-types/colors-and-brushes/). The mapper explicitly rounds each opaque resolved sRGB channel to its nearest byte, with error at most `0.5/255`; it does not rely on percentage conversion or discard alpha. Actual pixel contrast and area still require evaluation against actual surroundings. Coordinates, radii and bounds must remain finite in binary32 with at most `1/1024` logical px rounding error, or rendering fails explicitly. These are backend realization limits, not restrictions on normative Resina IR.

`resina-focus-slint <path|->` reads a UTF-8 focus request up to 1 MiB. Success exits 0 with complete source and no diagnostics; invalid input or unsupported coordinate precision exits 1 with diagnostics and no output; usage errors exit 2. The [external checker](../../../tools/check_focus_slint_backend.py) checks all public focus IR cases, both winding directions, placement, pigment, decorative semantics, strict input and deterministic output. Rust and checker tests run in Linux and Windows CI.

## Native render evidence

The [baseline](../../../conformance/slint/focus-baseline.slint) is the public sharp raised surface's ring; the [probe](../../../conformance/slint/focus-probe.slint) places it over a known background. With the official `slint-viewer` 1.18.1 and the render-check requirements installed:

```sh
python -m pip install -r tools/requirements-render.txt
slint-viewer --check conformance/slint/focus-probe.slint
mkdir -p target/slint-proof
python tools/check_slint_focus_runtime.py --image target/slint-proof/focus-2.png --scale 2 -- \
  env SLINT_SCALE_FACTOR=2 slint-viewer --screenshot target/slint-proof/focus-2.png conformance/slint/focus-probe.slint
```

Use a fresh image path. The checker decodes PNG pixels and compares opaque ring, clear hole, gap and exterior against an independent distance-to-rectangle oracle. It excludes centers within half a pixel diagonal of either boundary, so each checked pixel footprint lies entirely in one region, and requires both painted and clear pixels. It bounds execution and image dimensions. Linux CI downloads the official viewer with a pinned SHA-256 digest and checks scales 1, 1.25, 2 and 3. Local screenshots also passed with larger item dimensions without geometric scaling. Hardware rendering and complete application accessibility remain unverified. This primitive does not establish full Slint conformance or WCAG compliance.
