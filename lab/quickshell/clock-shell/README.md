# Quickshell clock shell

A bounded Quickshell 0.3.2 shell that consumes Resina surfaces and Commands:

- a flat backdrop on the background layer;
- a Frost chrome bar with a "Show clock" Command;
- an exclusive overlay surface showing the live time and date, with a "Close" Command.

It is a Lab product built on the released Quickshell backend. Its window, layer,
focus, layout and preference policy belong to this shell, not to Resina.

## What it does

- The backdrop is painted in the theme's resolved opaque `surface.base` colour,
  obtained through `SurfacePaintResolver.resolveOpaqueColor`. The bar and the
  clock surface declare that colour as their adjacent colour, so their edge
  contrast guards describe what is actually painted. A wallpaper would be a
  variable backdrop that the guards do not certify.
- The bar (`surface.chrome`, Frost) and the clock surface (`surface.transient`,
  Gel) are painted live by `ResinaSurfaceView` from complete opaque surface
  requests. Every renderer capability is declared false, so Frost uses its opaque
  representation.
- Each Command declares its surface's resolved body colour as its adjacent,
  backdrop and surrounding colour, and is loaded only once that surface is
  published.
- The environment's `reducedMotion` and `highContrast` come from the platform
  through `PlatformAppearance`. A preference the portal does not provide, or every
  preference when the portal is unavailable, falls back to
  [`preferences.json`](preferences.json) and logs `RESINA_CLOCK_PREFERENCE`;
  `reducedTransparency` is always authored because the portal defines no such key.
  Theme files are tagged with the theme they were read for, so a theme change
  never combines files from both themes.
- Without `RESINA_CLOCK_THEME`, the platform colour scheme selects the dark theme
  for a dark preference and the light theme otherwise.
- Both Commands sample motion with the calibrated Elastomer profiles of
  [material response calibration](../../../spec/80-material-response-calibration.md)
  (`elastomer-body`, `elastomer-depth`). Under reduced motion they publish
  immediate settled frames.
- "Show clock" maps the clock surface and is unavailable while the surface is
  open. The surface requests exclusive keyboard interactivity and moves keyboard
  focus to "Close" from its `ready` change; the control exposes its accessible
  node before announcing readiness, so assistive technology receives that focus.
  "Close" and Escape unmap it.
- The bar and the clock surface size themselves from their Commands and text, and
  the clock text is sized by the `textScale` authored in
  [`preferences.json`](preferences.json); the shell reads no platform text
  scale. Each Command, including its focus ring, is placed within its surface's
  content region and is surrounded by that surface's body colour.
- `SystemClock` runs at second precision only while the surface is open. The time
  uses the pattern `HH:mm:ss`; the date uses the long date format of the authored
  locale. This text is shell content, not a Resina text contract (#196). It uses
  the surface's guarded foreground colour inside its content region, and is
  exposed to assistive technology as static text: it sits beside the decorative
  surface view rather than inside it, because the view is ignored by
  accessibility together with its children.
- The open state is a `PersistentProperties` value: it survives a soft reload and
  is reset by a hard reload or restart.
- Each window resolves its own environment: its `scale` is the device pixel
  ratio of that window, and the backdrop colour each window paints is resolved
  from that environment. A change of a window's ratio, including the change from
  the fallback ratio a window reports before it is exposed, resolves its
  environment, surface requests and Command requests again.
- An IPC target `clock` provides `open`, `close`, `reload(hard)`, `observe`,
  `request(part)`, `record(enabled)`, `frame(index)` and `publications`.
  `record` discards the recorded frames and starts or stops recording; while
  recording, the shell keeps every motion frame of either Command as it is
  published, with the device pixel ratio of the Command's window, and
  `frame(index)` returns one of them, or `null` past the last. One frame per
  reply keeps each reply within what the Quickshell 0.3.2 IPC client reliably
  receives. `publications` returns, for the bar, the clock surface and each
  Command, the device pixel ratio of every publication since the current shell generation loaded,
  as runs of `[scale, count]`.

## Prerequisites

- Quickshell 0.3.2 built against Qt 6.11.2, and the packaged `Resina.Runtime`
  module from [`resina-qt-runtime`](../../../backends/quickshell/resina-qt-runtime/README.md).
- A Wayland compositor with `wlr-layer-shell`.
- For platform preferences, an XDG Desktop Portal with a backend that implements
  the `org.freedesktop.appearance` settings on the session bus.
- A Linux checkout: `controls` is a symbolic link to the reusable QML controls,
  because Quickshell does not load QML modules from outside its configuration
  folder.
- The font files of one family, for example the Inter static masters that the
  checks use (see [`tools/conformance_font.json`](../../../tools/conformance_font.json)).

## Running

```sh
QML_IMPORT_PATH=target/qt-runtime-import \
RESINA_ROOT="$PWD" \
RESINA_CLOCK_FONTS=/usr/share/fonts/opentype/inter/Inter-Regular.otf:/usr/share/fonts/opentype/inter/Inter-Medium.otf:/usr/share/fonts/opentype/inter/Inter-SemiBold.otf \
quickshell --path lab/quickshell/clock-shell/shell.qml
```

`RESINA_CLOCK_FONTS` is a colon-separated list of font files that must name one
family; the shell waits until all of them are loaded. `RESINA_ROOT` selects the repository whose theme tokens and definitions are read
at runtime. `RESINA_CLOCK_THEME=light` or `dark` fixes the theme. Geometry comes
from the first output; each window's device scale comes from that window.

## Failure diagnostics

- A missing source file or font logs `RESINA_CLOCK_ERROR` and leaves the shell
  unpainted.
- Failures to resolve the backdrop colour, or to prepare a surface or Command,
  log `RESINA_CLOCK_PREPARATION` with the Resina diagnostic and clear when the
  input recovers; the affected part stays withdrawn meanwhile, and no fallback
  colour or placeholder is painted.
- `qs ipc call clock observe` reports the current error of every part, the
  portal state, the preferences in effect and each Command's device scale. It
  also reports each window's logical bounds, the window placement and samples
  per axis of each surface and Command view, and the bounds of the time and
  date text.

## Verification

[`check_quickshell_clock_shell.py`](../../../tools/check_quickshell_clock_shell.py)
runs the shell in a caller-provided compositor session and drives it with
compositor-delivered pointer and keyboard input. It starts a private session bus
with the real `xdg-desktop-portal` frontend and GNOME backend, with isolated
GSettings, and changes platform preferences through GSettings. With
`--test-settings`, the frontend instead loads only the Settings backend
[`portal_settings_backend.py`](../../../tools/portal_settings_backend.py)
through `XDG_DESKTOP_PORTAL_DIR`. That backend provides `reduced-motion`,
`contrast` and `color-scheme` in `org.freedesktop.appearance`, each starting at
0 (no preference). The check sets a key to 1 and resets it to 0 over a D-Bus
control interface, and the backend emits `SettingChanged` for each change. The
shell still reads the frontend's `org.freedesktop.portal.Settings`, exactly as
with a desktop backend. The check prints the backend it used as an
`INFO portal Settings` line and records it as `settingsBackend` in
`report.json`. `--require` fails the check, instead of skipping the case, when
the portal does not provide a listed key. It runs at the output scale given by
`--scale` and requires:

- every surface and Command request environment, the device pixel ratio of each
  Command's window and every recorded motion frame to use that scale;
- every publication of the bar, the clock surface and each Command from startup
  on, checked before each reload and at the end, to use its window's scale, and
  with `--live-scale` to move once from the old scale to the new one;
- the backdrop to equal `resina-theme-resolve`'s opaque `surface.base`, every
  surface publication to equal `resina-surface-paint` for its request, and each
  Command snapshot to match its request and state;
- every declared adjacent and surrounding colour to be the colour actually
  painted next to it; with `--grim`, the capture to be exactly the logical
  output at the checked scale, so the output's logical size must be whole at
  that scale, and every pixel of the capture to equal the reference composition
  described under [composited captures](#composited-captures);
- each surface publication to be compared only once it matches its current
  request;
- a pointer click on "Show clock" to map the surface with keyboard focus on
  "Close", and "Show clock" to be unavailable while the surface is open;
- the displayed time to match `SystemClock`, advance by one second and agree
  with host wall time within 1.5 s;
- Space on "Close" and Escape to unmap the surface and pause the clock;
- a soft reload to keep the surface open in a new engine generation, and a hard
  reload to close it;
- the portal's preferences to reach the Command environment, the shell's
  platform values to equal an independent portal read, and each preference the
  portal provides to be taken from the platform rather than the authored file;
- every motion frame published while "Show clock" is held, recorded by the
  shell as it is published until a settled frame, to use the calibrated
  Elastomer dynamics and match the independent spring equations and complete
  snapshot, with at least one unsettled spring frame;
- the platform reduced-motion setting, changed through the portal backend, to
  switch the Commands to immediate settled frames, and its reset to restore
  calibrated spring frames;
- the platform high-contrast setting to reach every surface and Command, each
  re-verified against the reference paint, and its reset likewise;
- without a fixed theme, the platform colour scheme to select the dark and light
  theme, each re-verified against the reference paint and required to use that
  theme's source and Command appearance;
- with `--live-scale`, after a change of the single active Sway output's scale
  through the only `sway-ipc.*.sock` in `XDG_RUNTIME_DIR` (a set `SWAYSOCK`
  must name that socket), the bar and "Show clock" to be republished at the new
  device scale with the same target, the reference paint, the composited pixels
  with `--grim`, one available native "Show clock" Button and calibrated spring
  frames while held; the accessibility cases below then run at the new scale;
- on a private AT-SPI bus, the native tree of the shell to be exactly, by role
  and name, one unnamed frame per mapped window (bar, backdrop and, while open,
  the clock surface), one Button per mapped Command with its complete name and
  description and, while open, the displayed time and date as two static text
  labels, before opening, while open and after closing. Every Button carries
  `Press`; frames carry no action, and the static text carries at most the
  `SetFocus` action that Qt 6.11.2 gives every static text, because it
  [makes text roles focusable](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/accessible/qaccessiblequickitem.cpp#L451-L452).
  "Show clock" must be unavailable and
  "Close" available and focused while open; a native Press must open and close
  the clock surface;
- with an AT-SPI focus listener registered, a native focus event for "Close" when
  the surface opens, and for "Show clock" when, after closing, a click on the bar
  body and Tab move keyboard focus to it;
- with `--screen-reader`, run in the same way as for
  [Command accessibility](../../../backends/quickshell/resina-qt-runtime/README.md#released-quickshell-command-accessibility),
  the focus presentation verified for the running Orca release when the surface
  opens and after Tab; unverified releases fail before Orca starts. With Orca
  50.3 this is the speech "Close", "button.", "Close the clock surface." with the
  braille line "quickshell application frame Close button Close the clock
  surface.", and "Show clock", "button.", "Open the clock surface." with
  "quickshell application frame Show clock button Open the clock surface.".

### Composited captures

With `--grim`, which also needs `--paint-raster` (`resina-paint-raster`),
`--command-raster` (`resina-command-raster`) and `--command-resolver`
(`resina-command-paint`), the check captures the output while closed
(`closed.png`), while open (`open.png`) and, with `--live-scale`, after the
scale change (`rescaled.png`). It compares every pixel of each capture with an
expected capture that it builds independently of the shell's own images, saved
beside it as `<capture>-expected.png`:

- The expected capture starts as the backdrop's resolved `surface.base` colour.
  The bar and the clock window are opaque buffers of the same colour, which
  their surfaces declare as adjacent.
- Each published surface and Command is rendered by the CPU reference raster:
  a surface from its request with `resina-paint-raster`, and a Command from
  the Command paint request formed from its shell request, its published label
  size and its current state. The members that
  [`command-paint-request`](../../../schemas/command-paint-request.schema.json)
  carries beside the surface are taken from the Command body, and the request
  must validate against that schema. `resina-surface-paint` and
  `resina-command-paint` must reproduce the captured publications exactly, so
  each raster is the raster of the published IR. Each raster uses the device
  scale, the samples per axis and the window placement that the view was
  prepared with, and its paint bounds, including a focus ring, are rounded
  outward on the device grid after placement, as the Qt realization prepares
  them.
- In each window buffer the rasters are composited source-over in 8-bit sRGB:
  the bar's surface and then "Show clock", and the clock surface and then
  "Close". A pixel that a raster covers completely takes the raster's colour,
  and a pixel it leaves clear keeps the colour below. A partially covered pixel
  is the exactly rounded blend `(c·a + d·(255 − a)) / 255`.
- Each window buffer is placed where Sway places it. Every edge of its logical
  box, which must be whole, is multiplied by the output scale and rounded half
  away from zero, as the wlroots 0.20 scene does
  ([`scale_box`](https://gitlab.freedesktop.org/wlroots/wlroots/-/blob/0.20.2/types/scene/wlr_scene.c#L352-L361)).
  Qt sizes the buffer as the logical size times the scale rounded by `qRound`
  ([`QWaylandShmBackingStore`](https://github.com/qt/qtbase/blob/v6.11.2/src/plugins/platforms/wayland/qwaylandshmbackingstore.cpp#L409-L410)).
  At scale 1.25 on the 1280×720 output the clock window's top margin is 99
  logical pixels, so its origin of 123.75 device pixels lands on 124. Where the
  rounded box has the buffer's size, every buffer pixel lands on one capture
  pixel; this held for every window in the runs below. Where it does not, for
  example a 198 px high window at 99, whose 247.5 device pixels give a 248 px
  buffer in a 247 px box, Sway resamples the buffer. Each pixel of such a window
  is then only bracketed: every channel must lie within the bounds of the buffer
  pixels between which its sample position falls on the scaled axis, and of its
  own buffer pixel on an unscaled axis. Where the position falls beyond the
  buffer's edge, the pixel may also darken towards black: Sway was observed to
  darken such an edge row below both the buffer and the colour beneath it, as
  when an opaque buffer is copied without blending. `observe` reports
  each window's logical bounds, and `report.json` names every resampled window.

The tolerance is stated per pixel, in the manner of a
[WPT fuzzy reftest](https://web-platform-tests.org/writing-tests/reftests.html#fuzzy-matching)
(maximum channel difference and number of differing pixels):

- a pixel covered completely or not at all by every raster, which includes the
  whole backdrop, every flat body, the opaque edge and highlight bands and the
  opaque part of every focus ring, must match exactly: maximum difference 0;
- a partially covered pixel, on a corner arc or the antialiased border of a
  footprint, may differ by one 8-bit step per raster that covers it partially.
  Qt's software renderer premultiplies the source and scales the destination
  with its own rounding; each of those roundings is within half a step, so its
  result is within one step of the exactly rounded blend, and an error below
  carries into the next blend by less than one step;
- only partially covered pixels may differ, so the number of differing pixels is
  at most the number of partially covered pixels, which the check reports;
- in a resampled window, the bracketing bounds above replace these bounds. They
  still fail a flat colour that changes by one step and a raster moved by one
  buffer pixel, but they admit smaller changes along a scaled edge.

Native text is drawn by Qt, not by the reference raster, so its glyphs are not
compared. The excluded regions are exactly the device pixels touched by the
published bounds of each Command label (`labelBounds` in its snapshot, placed
at the Command) and of the clock surface's time and date text (`texts` in
`observe`), widened by one pixel on each side in a resampled window. Every
channel of a pixel in such a region must lie between the bounds of the expected
colour below it and the text's foreground colour, widened by one step for the
text's own blend. Text that escapes its
published bounds is therefore compared with the raster and fails, as does a
pixel in a text region outside that range. A difference in a text region that
stays between the background and the foreground is not detected.

Each comparison is recorded under `composition` in `report.json` with each
window's device origin, box and buffer size, footprints and text regions, the
number of compared, partially covered, differing, resampled and excluded pixels
and the largest difference, and is printed as an `INFO` line.

At the revision that added this comparison (#254), with Quickshell 0.3.2, Qt
6.11.2 and the pinned Inter masters, in headless Sway 1.12 with the pixman
renderer:

| Run | Capture | Scale | Compared | Partially covered | Differing (max) | Text pixels |
| --- | --- | --- | --- | --- | --- | --- |
| 1024×600, `--test-settings` | closed | 1 | 612,820 | 1,251 | 1,161 (1) | 1,580 |
| | open | 1 | 597,018 | 1,915 | 1,544 (1) | 17,382 |
| 1024×600 | closed | 2 | 608,434 | 1,579 | 1,237 (1) | 5,966 |
| | open | 2 | 545,548 | 3,683 | 3,002 (1) | 68,852 |
| 1280×720, 1.25 then 2 | closed | 1.25 | 919,224 | 402 | 194 (1) | 2,376 |
| | open | 1.25 | 894,344 | 1,791 | 1,300 (1) | 27,256 |
| | rescaled | 2 | 915,634 | 1,835 | 1,493 (1) | 5,966 |
| 1280×720, 2 then 1.25 | closed | 2 | 915,634 | 1,835 | 1,493 (1) | 5,966 |
| | open | 2 | 852,748 | 3,939 | 3,258 (1) | 68,852 |
| | rescaled | 1.25 | 919,224 | 402 | 194 (1) | 2,376 |

The fixed dark theme passes at scale 1 on 1024×600. Each of these changes to the realization, made in a copy of the checkout,
fails the comparison with the unchanged checker:

- the Command image moved by one device pixel at scale 1.25, and the bar's
  surface image moved by one device pixel at scale 2;
- every opaque pixel on the outer border of each prepared image made 2 steps
  lighter at scale 1, which fails on the first edge band pixel of the bar;
- every corner radius made 2 px larger in the resolver of the Qt runtime at
  scale 2, which fails because the bar publication is no longer the reference
  surface paint; with that publication check and the Command paint check
  removed from the copy, the comparison fails at scale 1.25 on the bar's
  corner arcs.

With Quickshell 0.3.2 (revision 4f508be), Qt 6.11.2, xdg-desktop-portal 1.22.1
and xdg-desktop-portal-gnome 50.0, at revision 4cb60fe:

- all cases pass with the platform-selected theme in headless Sway 1.12 at
  output scales 1 and 2 on a 1024×600 output and in the live change from 1.25
  to 2 on a 1280×720 output, including the `grim` composition checks, and in
  nested niri 26.04 at scale 1, including the native focus-event cases;
- the fixed dark theme passes in headless Sway 1.12 at output scales 1 and 2
  and in the live change from 1.25 to 2;
- `--screen-reader` passes with Orca 50.3 in nested niri 26.04 at scale 1;
- CI ran the check at 4cb60fe in headless Sway at output scale 1 and in the
  live changes from 1.25 to 2 and from 2 to 1.25;
- the 1.5 output on a 1920×1080 output, the live changes from 1 to 2, from 2
  to 1 and from 1.25 to 1.5, and nested niri at scale 2 passed at earlier
  revisions and were not repeated at 4cb60fe;
- CI does not run the clock shell check in niri or with `--screen-reader`;
- at 4cb60fe the `grim` checks compared, within one 8-bit step, only points
  where a flat body or backdrop colour is expected; the complete comparison
  under [composited captures](#composited-captures) replaced them;
- a 1024×600 output at scale 1.25 or 1.5 is 819.2 or 682.67 logical pixels
  wide. Its layer surfaces cover 819 or 682 logical pixels, so the last device
  column is not covered by the backdrop; the capture size check rejects such an
  output;
- with the software renderer, a frame at scale 2 lasted about as long as the
  spring takes to settle, so a held "Show clock" published one or two unsettled
  frames before settling, against three at scale 1;
- with `--test-settings --require reduced-motion contrast color-scheme`, at the
  revision that added the test backend (#264), all cases pass in headless Sway
  at output scale 1. With `reduced-motion` removed
  from the test backend, the check fails on the missing required key, and
  without `--require` the case is a `SKIP`. With the runtime element changed to
  ignore `SettingChanged` for `reduced-motion`, the reduced-motion case fails
  because the change never reaches the shell environment.

## Boundaries

- No Command is focused when the clock surface closes. In headless Sway keyboard
  focus does not return to the bar; in nested niri the bar was observed to regain
  keyboard focus without a focused item. In both compositors, Space after closing did not
  activate "Show clock". Removing the focused Command requires its owner to
  choose explicit focus recovery ([focus traversal](../../../spec/41-focus-traversal.md)).
  [Command activation](../../../spec/42-command-activation.md) leaves focus
  recovery to the component or product, and
  [Command accessibility](../../../spec/47-command-accessibility.md) keeps it a
  separate requirement. That owner is this shell, which chooses
  no recovery, so closing the surface leaves focus unrecovered. The check makes
  no assertion about focus after closing; it reports whether Space then
  activates "Show clock" as `spaceActivatesShowClockAfterClose`.
- Each window is mapped once the shell's sources and preferences have loaded,
  before its theme is resolved, because its environment needs that window's
  device scale. If a window's theme resolution fails, the window stays mapped,
  transparent and without content, and the bar keeps its exclusive zone; the
  error is logged as `RESINA_CLOCK_PREPARATION backdrop:` and reported by
  `observe` under `errors.backdrop`.
- xdg-desktop-portal-gnome before 50 does not provide `reduced-motion`; the
  authored value is then used, so with xdg-desktop-portal-gnome 46 to 49 turning
  animations off in GNOME does not reduce the shell's motion. Neither the shell
  nor the platform appearance element reads GSettings directly, including
  `org.gnome.desktop.interface enable-animations`, as a fallback. Every platform
  case that cannot run, because the portal does not provide its key or `--theme`
  fixes the theme, is printed as a `SKIP` line with its reason and kept under
  `skips` in `report.json`; a key listed with `--require` fails the check
  instead.
- CI runs Ubuntu 24.04's xdg-desktop-portal 1.18.4 and xdg-desktop-portal-gnome
  46.2. At output scale 1 it runs the check with `--test-settings --require
  reduced-motion contrast color-scheme`, so the platform reduced-motion,
  high-contrast and colour-scheme cases all run through the real frontend with
  the test backend, and a missing key fails the step. This shows that values the
  frontend delivers, and their changes, reach the Commands. It does not show
  that a desktop backend provides them. At output scales 1.25 and 2, CI runs the
  GNOME 46.2 backend with `--require contrast color-scheme`; the reduced-motion
  case is a `SKIP` there. A desktop backend's `reduced-motion` reaching the
  Commands is verified only locally, with xdg-desktop-portal-gnome 50.0.
- One output is verified, with the authored text scale 1, the `en-GB` locale and
  left-to-right layout direction of [`preferences.json`](preferences.json). The
  shell reads no platform text scale, and no check runs it at another text
  scale, locale or direction. CI runs the check in headless Sway at output scale
  1 on a 1024×600 output, and at 1.25 and 2 on a 1280×720 output with live
  changes from 1.25 to 2 and from 2 to 1.25. Multiple outputs and output
  hot-plug are not covered.
- The time and date text and its locale handling are shell content. Its value
  changes every second without being announced; live-value semantics belong to
  #196. Screen-reader presentation is verified only for the focused Commands,
  locally.
- Translucent Frost and Gel treatments, hardware renderers and physical input
  are not covered.
