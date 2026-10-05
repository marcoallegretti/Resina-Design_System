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
measurement is drawn. The public Heading, Body and Caption roles have supported
zero spacing; authored Label/Display tracking is not rewritten to hide GUIdo's
current letter-spacing limitation.

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
