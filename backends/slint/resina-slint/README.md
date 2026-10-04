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

## Native focus eligibility and recovery

The [focus recovery fixture](../../../conformance/slint/focus-recovery.slint) and [checker](../../../tools/check_slint_focus_recovery.py) exercise Slint 1.18.1 focus delivery independently of the painted cue. Run from the repository root with a fresh capture path:

```sh
python tools/check_slint_focus_recovery.py --image target/slint-proof/focus-recovery.png -- slint-viewer
```

Map command focusability to `FocusScope.enabled` and action availability separately to `accessible-enabled`. The pinned runtime accepts focus on a disabled but discoverable command when its scope remains enabled. Changing action availability alone retains actual focus. This matches the distinction in the [ordinary command contract](../../../spec/47-command-accessibility.md); it does not establish native assistive technology delivery or activation handling.

Before excluding a focused command from focusability, transfer focus to an explicitly chosen eligible successor and verify successful native transfer, then disable the old scope. Slint 1.18.1's [focus event handler](https://github.com/slint-ui/slint/blob/v1.18.1/internal/core/items/input_items.rs) ignores focus events when `enabled` is false, including focus loss. Disabling the old scope first can therefore leave its `has-focus` true even after the successor receives focus. The fixture reproduces that unsafe order and verifies that transferring first clears the old state and retains the successor's focus. Do not derive focused state from a requested target or assume that disabling a scope clears it.

The checker requires the pinned viewer, all nine exact observations, a successful bounded process and a fresh capture. Linux CI uses the existing digest-verified official viewer. The one-pixel capture only drives native item initialization; this fixture supplies no product UI or visual conformance evidence. It checks programmatic focus properties in the software screenshot runtime, not OS window activation, tab order, focus recovery without an eligible successor, platform accessibility trees or screen-reader behavior. Those remain separate integration requirements. The unsafe-order expectation documents a pinned upstream behavior rather than a normative Resina requirement.

## Native availability conformance

The [availability fixture](../../../conformance/slint/availability-probe.slint) and [Linux AT-SPI checker](../../../tools/check_slint_native_availability.py) inspect three ordinary command policies through a fresh native accessibility bus: enabled/focusable, disabled/discoverable, and disabled/excluded. They require the official Slint viewer 1.18.1, a native X11 or Wayland compositor, `dbus-send`, `dbus-run-session`, AT-SPI services and the desktop GSettings schemas. X11 additionally requires `libxkbcommon-x11.so.0` (Ubuntu package `libxkbcommon-x11-0`); the pinned Winit keyboard loader opens that library dynamically. Run from the repository root:

```sh
python tools/check_slint_native_availability.py -- slint-viewer
```

The checker requires exactly the three singular button nodes, their full names and descriptions, and both AT-SPI state words. It compares native enabled, sensitive and focusable states with the fixture policies. It discovers actual bus names and object paths, bounds tree size and execution, and closes the test window and isolated session. It does not invoke a product operation or establish keyboard, screen-reader, native focus or complete Button conformance. The fixture is a diagnostic window, not product UI or a visual reference.

The pinned Winit/AccessKit path currently fails this check: disabled buttons expose enabled/sensitive, and the excluded button still exposes focusable. The [AccessKit AT-SPI conversion](https://github.com/AccessKit/accesskit/blob/c88605b96d04431f9c3c792464a0f2f253480e94/platforms/atspi-common/src/node.rs) emits enabled/sensitive for a button even when disabled; Slint's [native mapper](https://github.com/slint-ui/slint/blob/372cf0ee5577c3dfec309a45e7b778ba4e81b734/internal/backends/winit/accesskit.rs) advertises focus for the button role independently of `FocusScope.enabled`. A fresh compiled application reproduces the disabled-state discrepancy. These unsupported semantics prevent this path from claiming ordinary command accessibility conformance. Keep the current canonical activation guard; a published accessible action is not permission to invoke an unavailable command.

Linux CI runs the digest-verified viewer under Xvfb with `--report-only` and saves `slint-native-availability`. This diagnostic mode exits successfully after a valid native measurement while retaining `conformant: false` and every mismatch in the JSON report; it is not a conformance gate. Without that option, any mismatch exits 1. Missing services, malformed replies, missing/duplicate nodes, startup failure and timeouts fail in both modes. A future repair must pass the strict check before native command accessibility is claimed. Normative Resina semantics remain unchanged.
