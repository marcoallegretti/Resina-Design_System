# Quickshell clock shell

A bounded Quickshell 0.3.2 shell that consumes Resina surfaces and Commands:

- a flat backdrop on the background layer;
- a Frost chrome bar with a "Show clock" Command;
- an exclusive overlay surface showing the live time and date, with a "Close" Command.

It is a Lab product built on the released Quickshell backend. Its window, layer,
focus and layout policy belong to this shell, not to Resina.

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
- "Show clock" maps the clock surface. The surface requests exclusive keyboard
  interactivity and moves keyboard focus to "Close" once it is ready. "Close" and
  Escape unmap it.
- `SystemClock` runs at second precision only while the surface is open. The time
  uses the pattern `HH:mm:ss`; the date uses the long date format of the authored
  locale. This text is shell content, not a Resina text contract (#196). It uses
  the surface's guarded foreground colour inside its content region.
- The open state is a `PersistentProperties` value: it survives a soft reload and
  is reset by a hard reload or restart.
- An IPC target `clock` provides `open`, `close`, `reload(hard)`, `observe` and
  `request(part)`.

## Prerequisites

- Quickshell 0.3.2 built against Qt 6.11.2, and the packaged `Resina.Runtime`
  module from [`resina-qt-runtime`](../../../backends/quickshell/resina-qt-runtime/README.md).
- A Wayland compositor with `wlr-layer-shell`.
- A Linux checkout: `controls` is a symbolic link to the reusable QML controls,
  because Quickshell does not load QML modules from outside its configuration
  folder.
- DejaVu Sans as a font file.

## Running

```sh
QML_IMPORT_PATH=target/qt-runtime-import \
RESINA_ROOT="$PWD" \
RESINA_CLOCK_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
RESINA_CLOCK_THEME=dark \
quickshell --path lab/quickshell/clock-shell/shell.qml
```

`RESINA_ROOT` selects the repository whose theme tokens and definitions are read
at runtime. `RESINA_CLOCK_THEME` is `light` (default) or `dark`.
[`preferences.json`](preferences.json) holds the authored environment
preferences; geometry and device scale come from the first output.

## Failure diagnostics

- A missing source file, font or opaque backdrop colour logs
  `RESINA_CLOCK_ERROR` and leaves the shell unpainted.
- Preparation failures of a surface or Command log `RESINA_CLOCK_PREPARATION`
  with the Resina diagnostic, and the affected part stays withdrawn; no fallback
  colour or placeholder is painted.
- `qs ipc call clock observe` reports the current error of every part.

## Verification

[`check_quickshell_clock_shell.py`](../../../tools/check_quickshell_clock_shell.py)
runs the shell in a caller-provided compositor session and drives it with
compositor-delivered pointer and keyboard input. It requires:

- the backdrop to equal `resina-theme-resolve`'s opaque `surface.base`, every
  surface publication to equal `resina-surface-paint` for its request, and each
  Command snapshot to match its request and state;
- every declared adjacent and surrounding colour to be the colour actually
  painted next to it; with `--grim`, the composited output is sampled on the
  backdrop, the bar body and the clock body;
- a pointer click on "Show clock" to map the surface with keyboard focus on
  "Close";
- the displayed time to match `SystemClock`, advance by one second and agree
  with host wall time within 1.5 s;
- Space on "Close" and Escape to unmap the surface and pause the clock;
- a soft reload to keep the surface open in a new engine generation, and a hard
  reload to close it.

With Quickshell 0.3.2 (revision 4f508be) and Qt 6.11.2:

- light and dark themes pass in headless Sway 1.12 with `grim` composition
  checks at output scale 1;
- the light theme also passes at output scale 2 (a 2048×1200 pixel capture of a
  1024×600 logical output);
- both themes pass in nested niri 26.04 at scale 1.

## Boundaries

- Keyboard focus does not return to the bar when the clock surface closes. In
  both compositors, Space after closing did not activate "Show clock". Layer
  surfaces cannot request activation, so returning focus is compositor policy.
- One output at integer scales 1 and 2 is verified. Multiple outputs, fractional
  or changing scale, and output hot-plug are not.
- The time and date text, its locale handling and its accessible exposure are
  shell content. A clock value changing every second is not announced.
- Translucent Frost and Gel treatments, hardware renderers and physical input
  are not covered.
