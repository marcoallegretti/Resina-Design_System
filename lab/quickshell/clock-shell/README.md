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
  focus to "Close" once it is ready. "Close" and Escape unmap it.
- The bar and the clock surface size themselves from their Commands and text, and
  the clock text follows the authored text scale. Each Command, including its
  focus ring, is placed within its surface's content region and is surrounded by
  that surface's body colour.
- `SystemClock` runs at second precision only while the surface is open. The time
  uses the pattern `HH:mm:ss`; the date uses the long date format of the authored
  locale. This text is shell content, not a Resina text contract (#196). It uses
  the surface's guarded foreground colour inside its content region, and is
  exposed to assistive technology as static text: it sits beside the decorative
  surface view rather than inside it, because the view is ignored by
  accessibility together with its children.
- The open state is a `PersistentProperties` value: it survives a soft reload and
  is reset by a hard reload or restart.
- An IPC target `clock` provides `open`, `close`, `reload(hard)`, `observe`,
  `request(part)`, `record(enabled)` and `frames`. `record` discards the
  recorded frames and starts or stops recording; while recording, the shell
  keeps every motion frame of either Command as it is published, and `frames`
  returns them.

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
at runtime. `RESINA_CLOCK_THEME=light` or `dark` fixes the theme. Geometry and
device scale come from the first output.

## Failure diagnostics

- A missing source file or font logs `RESINA_CLOCK_ERROR` and leaves the shell
  unpainted.
- Failures to resolve the backdrop colour, or to prepare a surface or Command,
  log `RESINA_CLOCK_PREPARATION` with the Resina diagnostic and clear when the
  input recovers; the affected part stays withdrawn meanwhile, and no fallback
  colour or placeholder is painted.
- `qs ipc call clock observe` reports the current error of every part, the
  portal state and the preferences in effect.

## Verification

[`check_quickshell_clock_shell.py`](../../../tools/check_quickshell_clock_shell.py)
runs the shell in a caller-provided compositor session and drives it with
compositor-delivered pointer and keyboard input. It starts a private session bus
with the real `xdg-desktop-portal` frontend and GNOME backend, with isolated
GSettings. It requires:

- the backdrop to equal `resina-theme-resolve`'s opaque `surface.base`, every
  surface publication to equal `resina-surface-paint` for its request, and each
  Command snapshot to match its request and state;
- every declared adjacent and surrounding colour to be the colour actually
  painted next to it; with `--grim`, the composited output is sampled on the
  backdrop, the bar body and the clock body, and on a ring just outside each
  Command's complete footprint, including its focus ring;
- each surface publication to be compared only once it matches its current
  request;
- a pointer click on "Show clock" to map the surface with keyboard focus on
  "Close", and "Show clock" to be unavailable while the surface is open;
- the displayed time to match `SystemClock`, advance by one second and agree
  with host wall time within 1.5 s;
- Space on "Close" and Escape to unmap the surface and pause the clock;
- a soft reload to keep the surface open in a new engine generation, and a hard
  reload to close it;
- the portal's preferences to reach the Command environment;
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
- on a private AT-SPI bus, the native tree to expose exactly one Button per mapped
  Command with its complete name and description, "Show clock" unavailable and
  "Close" available and focused while open, and the displayed time and date as
  static text; a native Press must open and close the clock surface.

With Quickshell 0.3.2 (revision 4f508be), Qt 6.11.2, xdg-desktop-portal 1.22.1
and xdg-desktop-portal-gnome 50.0:

- all cases pass with the platform-selected theme in headless Sway 1.12, with
  `grim` composition checks at output scales 1 and 2 (a 2048×1200 pixel capture
  of a 1024×600 logical output), and in nested niri 26.04 at scales 1 and 2;
- with the software renderer, a frame at scale 2 lasted about as long as the
  spring takes to settle, so a held "Show clock" published one or two unsettled
  frames before settling, against three at scale 1;
- the fixed dark theme passes in headless Sway at output scale 1.

## Boundaries

- Keyboard focus does not return to the bar when the clock surface closes. In
  both compositors, Space after closing did not activate "Show clock". Layer
  surfaces cannot request activation, so returning focus is compositor policy.
- xdg-desktop-portal-gnome before 50 does not provide `reduced-motion`; the
  authored value is then used and the reduced-motion case is skipped.
- One output at integer scales 1 and 2, with text scale 1 and the `en-GB` locale,
  is verified. Larger text scales, other locales, multiple outputs, fractional
  or changing scale, and output hot-plug are not.
- The time and date text and its locale handling are shell content. Its value
  changes every second without being announced; live-value semantics belong to
  #196. Screen-reader presentation of the shell is not verified.
- Translucent Frost and Gel treatments, hardware renderers and physical input
  are not covered.
