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
- DejaVu Sans as a font file.

## Running

```sh
QML_IMPORT_PATH=target/qt-runtime-import \
RESINA_ROOT="$PWD" \
RESINA_CLOCK_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
quickshell --path lab/quickshell/clock-shell/shell.qml
```

`RESINA_ROOT` selects the repository whose theme tokens and definitions are read
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
  portal state, the preferences in effect and each Command's device scale.

## Verification

[`check_quickshell_clock_shell.py`](../../../tools/check_quickshell_clock_shell.py)
runs the shell in a caller-provided compositor session and drives it with
compositor-delivered pointer and keyboard input. It starts a private session bus
with the real `xdg-desktop-portal` frontend and GNOME backend, with isolated
GSettings. It runs at the output scale given by `--scale` and requires:

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
  that scale, and the composited output to be sampled on the backdrop, at every
  corner and edge midpoint of the capture, on the bar body and the clock body,
  and on a ring just outside each Command's complete footprint, including its
  focus ring;
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
- the platform reduced-motion setting, changed through GSettings, to switch the
  Commands to immediate settled frames, and its reset to restore calibrated
  spring frames;
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

With Quickshell 0.3.2 (revision 4f508be), Qt 6.11.2, xdg-desktop-portal 1.22.1
and xdg-desktop-portal-gnome 50.0:

- all cases pass with the platform-selected theme in headless Sway 1.12, with
  `grim` composition checks at output scales 1 and 2 on a 1024×600 output, 1.25
  on a 1280×720 output and 1.5 on a 1920×1080 output, and, at revision
  9abb974, in nested niri 26.04 at scales 1 and 2;
- in headless Sway 1.12 the live changes from 1 to 2 and 2 to 1 on a 1024×600
  output, 2 to 1.25 and 1.25 to 2 on a 1280×720 output and 1.25 to 1.5 on a
  1920×1080 output pass, including the `grim` checks after the change. The
  `grim` checks compare, within one 8-bit step, only points where a flat body or
  backdrop colour is expected. No compared point lies on a rounded corner, an
  edge or highlight band or a Command's own paint, so the composited arcs and
  edges are not checked, and complete captures are not compared;
- a 1024×600 output at scale 1.25 or 1.5 is 819.2 or 682.67 logical pixels
  wide. Its layer surfaces cover 819 or 682 logical pixels, so the last device
  column is not covered by the backdrop; the capture size check rejects such an
  output;
- with the software renderer, a frame at scale 2 lasted about as long as the
  spring takes to settle, so a held "Show clock" published one or two unsettled
  frames before settling, against three at scale 1;
- the fixed dark theme passes in headless Sway at output scale 1;
- the native focus-event cases have been run at output scale 1, in headless Sway
  with the platform-selected theme and, at revision 9abb974, in nested niri
  26.04;
- `--screen-reader` passed with Orca 50.3 in nested niri 26.04 locally at
  revision 9abb974. The shell and its check have changed since, and neither the
  niri runs nor the `--screen-reader` run have been repeated; CI does not run
  the clock shell check in niri or with `--screen-reader`.

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
  `org.gnome.desktop.interface enable-animations`, as a fallback. CI runs the
  GNOME 46 backend, so its platform reduced-motion case is a `SKIP` there and is
  verified only locally with xdg-desktop-portal-gnome 50.0. Every platform case
  that cannot run, because the portal does not provide its key or `--theme`
  fixes the theme, is printed as a `SKIP` line with its reason and kept under
  `skips` in `report.json`.
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
