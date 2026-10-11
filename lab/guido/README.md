# Native Material Board

This Lab capture command renders the public Tier 0 material catalog through
GUIdo's real GPU pipeline. It compares Cast, Frost, Elastomer and Gel in Light
and Dark, with complete rest and focus paint. Specimens are visual samples,
not interactive controls. The Lab is a product consuming the reference and
backend APIs; its layout and capture policy do not define Resina.

Prepare the catalog with the existing theme resolver, then capture it on Linux:

```sh
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
cargo run --manifest-path lab/guido/Cargo.toml --locked -- material-board target/material-scenes.json /path/to/font.ttf "Font Family" target/material-board.png 1.25 1
```

The final arguments are device scale and text scale, defaulting to 1. Device
scale must be finite in [0.5, 4], text scale in [1, 3]. Captures are limited to
16,777,216 pixels. Header text reflows at its measured width; the board grows
vertically with text scaling. The named font family must exist in the supplied
font file. GUIdo performs weight matching and shaping; arbitrary scripts and
every font face are not certified by this English material catalog.

The output is a native RGBA PNG tagged as sRGB. An existing output is never
overwritten. Invalid catalogs, inconsistent names/themes/backdrops, unsupported
capabilities, font or typography failures, resource limits and missing GPU
adapters produce explicit errors. No substitute specimen or approximate text
measurement is drawn. Board text uses the authored Heading, Body and Caption
typography unchanged.

The tool requires all 16 complete paint scenes, verifies their material and
focus identity, and uses their actual theme canvas. Every renderer capability
must be false for this Tier 0 board. Source palettes, forms, pigment, depth and
focus laws remain unchanged. Surface placement happens before device-grid
sampling. Gel appears as the catalog's effect surface, not a persistent control.

## Verification

Set `RESINA_SCENES` to the prepared catalog and `RESINA_LABEL_FONT` to a DejaVu
Sans font file, then run:

```sh
cargo test --manifest-path lab/guido/Cargo.toml --locked
cargo clippy --manifest-path lab/guido/Cargo.toml --all-targets --locked -- -D warnings
cargo fmt --manifest-path lab/guido/Cargo.toml -- --check
```

Required GPU readback checks every placed opaque specimen pixel, complete text
coverage and opaque canvas output across fractional/device and text scales.
Command tests decode the PNG and verify that an existing output is preserved.
CI runs these gates using the same public catalog and installed test font.

This is a complete static material capture operation. It does not complete the
blueprint's interactive Lab, Component/State/Adaptation/Localization/Accessibility
Boards, complex wallpapers, higher capability materials, motion or native
assistive technology delivery. Those remain required for the full system.

## Native Command input fixture

`command_input_probe` is a fixed-context verification example using the authored
standard Command anatomy, Light theme, Label typography, complete native text
measurement and checked paint/target snapshots. It renders static endpoints at
device scale 1 and increments a consumer counter on activation. The third intent
revokes availability. State is committed before the counter notification;
focus paint follows observed widget and surface focus. The native runtime
retains pointer delivery outside the target while a mouse button is held.

Build the example after preparing the catalog above:

```sh
cargo build --manifest-path lab/guido/Cargo.toml --locked --example command_input_probe
cargo run --manifest-path lab/guido/Cargo.toml --locked --example command_input_probe -- \
  "$PWD" target/material-scenes.json /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

Run it in an isolated layer-shell session with a fixed unit-scale output. The
tested host is Sway 1.10.1 with an X11 backend, an Xvfb parent display and software
Vulkan rendering. The fixture requires DejaVu Sans in the supplied font file,
uses the scene's authored canvas, preserves its direction and rejects native
layout that cannot contain the reserved target. Observed non-unit integer output
scale fails explicitly. Font replacement, fractional scale changes, dynamic
layout/replacement and sampled motion are outside this fixed fixture's scope.
Dimensions and padding are verification choices, not calibrated component defaults.

The checker drives parent XTest events through Sway's native Wayland input:

```sh
python tools/check_guido_command_input.py \
  --probe lab/guido/target/debug/examples/command_input_probe \
  --scenes target/material-scenes.json \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --sway-socket /path/to/sway-ipc.sock \
  --parent-display :92 --parent-window 0x200004
```

Supply the actual isolated Sway socket and its parent X11 output window ID;
the example values identify no particular running session. `WAYLAND_DISPLAY`
and `XDG_RUNTIME_DIR` must select that same live session. `swaymsg`, libX11 and
libXtst are required. If `CARGO_TARGET_DIR` is set, supply its example binary
instead of the default path. The checker verifies a single active `X11-1` output
at scale 1 and measures the parent window's origin before injecting input.

Acceptance for this fixture is one pointer intent after outside/reentry, one
Space intent after actual repeat delivery, one Enter intent, zero effects from
subsequent unavailable input, no remaining hold, stable target publication and
painted focus matching native observation. This is compositor-delivered injected
input, distinct from the mounted widget-tree tests. It does not execute a native
capture API, publish an accessibility tree or establish AT-SPI, screen-reader or
physical-device behavior. It is a verification consumer, not a reusable native
Command component. Native accessibility integration remains tracked in
[GUIdo issue #653](https://github.com/MalpenZibo/guido/issues/653).
