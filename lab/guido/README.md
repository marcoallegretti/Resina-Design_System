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
cargo run --manifest-path lab/guido/Cargo.toml --locked -- material-board target/material-scenes.json /path/to/Regular.otf:/path/to/Medium.otf:/path/to/SemiBold.otf "Font Family" target/material-board.png 1.25 1
```

The final arguments are device scale and text scale, defaulting to 1. Device
scale must be finite in [0.5, 4], text scale in [1, 3]. Captures are limited to
16,777,216 pixels. Header text reflows at its measured width; the board grows
vertically with text scaling. The font argument lists one or more font files
with the platform path-list separator (`:` on Linux), and the named family must
exist in every file. Supply a face for each authored weight, for example the
Regular, Medium and SemiBold masters; GUIdo otherwise matches the nearest weight.
GUIdo performs weight matching and shaping; arbitrary scripts and every font face
are not certified by this English material catalog.

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

### Continuous-relief specimens

`relief-board` uses the same prepared theme catalog to compare flat Cast structure,
raised Elastomer controls and recessed Cast wells. Each page includes the five
authored shape intents in Light and Dark. Its explicit
[recipes](relief-recipes.json) are Lab specimen choices, with a separate
[tool schema](schemas/relief-recipes.schema.json); they do not define component
defaults. Corner extents and pigment strengths are authored explicitly for these
specimens. Quartic extents increase from structural through rounded; organic
corners alternate large and small logical extents. The resolver normalizes them
against each actual face size before checking the relief profile. Wider shoulders
remain outside the outline stroke, and Cast's specimen pigment strengths make
inward relief visible in both palette polarities. These choices do not replace
the Tier 0 circular shape fallback or its pigment defaults.
Capsules use circular geometry; the other specimens explicitly select quartic
geometry. Nominal body thickness does not project a side or create a shadow.

```sh
cargo run --release --locked --manifest-path lab/guido/Cargo.toml -- relief-board target/material-scenes.json /path/to/Regular.otf:/path/to/Medium.otf:/path/to/SemiBold.otf "Font Family" target/relief-board.png 1.25 1 convex rest
```

The last two arguments select `flat`, `convex` or `recessed`, then `rest` or
`focused`. Focused specimens preserve concurrent selected, focused and hover
states with the existing ring around the actual unprojected footprint. They use
the actual theme canvas, foreground and outline candidates without substituting
diagnostic colors. Content remains in the uniform plateau; insufficient contrast
or unsupported theme bindings fail explicitly. Faces grow vertically with text
scale, and headings and captions reserve their measured wrapped height. The
16-megapixel limit still applies. These are static appearance specimens, with no
activation or live state handling.

`relief-board-native` captures the same reference and presents its prepared
panels, complete text and material images through native widget painting. Native
surface dimensions must be integral; the board reserves logical height in
multiples of four for the common fractional scales. Existing output files are
preserved. Run the checker under a live parent Wayland display with Sway and grim:

```sh
export CONFORMANCE_FONTS="$PWD/target/conformance-fonts"
python tools/conformance_font.py fetch --output "$CONFORMANCE_FONTS"
cargo build --release --locked --manifest-path lab/guido/Cargo.toml
python tools/check_guido_relief_native.py --probe lab/guido/target/release/resina-lab --request target/material-scenes.json --content-font $CONFORMANCE_FONTS/Inter-Regular.otf --relief-board convex --board-state focused --output-dir target/relief-board-native
```

Supply the actual binary path when `CARGO_TARGET_DIR` is set. Preflight determines
the complete opaque reference size before the checker starts an owned nested
Wayland output. The ten-specimen board has a bounded 300-second preparation
deadline for software rendering. Every native canvas pixel is compared, including text and
antialiasing. Board GPU tests separately compare prepared opaque pixels and
complete text against expanded text rectangles; an omitted caption must change
the result. Nonflat specimens must retain opposite top/bottom shoulder tones
and a multi-pixel interior cue after excluding the outline. Recipe checks also
reject shape collisions after corner normalization. These checks constrain the
authored specimens; visual inspection still evaluates their material identity.
Native checks establish presentation, not input or assistive
technology integration.

Set `RESINA_SCENES` to the prepared catalog and `RESINA_LABEL_FONT_DIR` to the
directory holding the pinned Inter static masters
([`tools/conformance_font.json`](../../tools/conformance_font.json)), then run:

```sh
cargo test --manifest-path lab/guido/Cargo.toml --locked
cargo clippy --manifest-path lab/guido/Cargo.toml --all-targets --locked -- -D warnings
cargo fmt --manifest-path lab/guido/Cargo.toml -- --check
```

The workspace builds its dev profile at `opt-level = 1`; debug assertions and
overflow checks stay enabled. Set `CARGO_PROFILE_DEV_OPT_LEVEL=0` for an
unoptimized build when debugging.

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
revokes the current owner's availability. State is committed before the counter notification;
focus paint follows observed widget and surface focus. The native runtime
retains pointer delivery outside the target while a mouse button is held.

Build the example after preparing the catalog above:

```sh
cargo build --manifest-path lab/guido/Cargo.toml --locked --example command_input_probe
cargo run --manifest-path lab/guido/Cargo.toml --locked --example command_input_probe -- \
  "$PWD" target/material-scenes.json \
  $CONFORMANCE_FONTS/Inter-Regular.otf:$CONFORMANCE_FONTS/Inter-Medium.otf:$CONFORMANCE_FONTS/Inter-SemiBold.otf
```

Run it in an isolated layer-shell session with a fixed unit-scale output. The
tested host is Sway 1.10.1 with an X11 backend, an Xvfb parent display and software
Vulkan rendering. The fixture requires Inter in every supplied font file,
uses the scene's authored canvas, preserves its direction and rejects native
layout that cannot contain the reserved target. Observed non-unit integer output
scale fails explicitly. Font replacement, fractional scale changes, dynamic
layout and sampled motion are outside this fixed fixture's scope.
Dimensions and padding are verification choices, not calibrated component defaults.

Escape is a consumer verification shortcut that replaces the mounted Command
with a fresh enabled owner in the same immutable context. It is not a Command
activation key. The retired owner cancels through Rust, becomes unavailable and
unfocused, and cannot publish later state, paint or intents. Retirement state
does not depend on reactive handles: GUIdo disposes a dynamic child's scope before
dropping its widget. Complete prepared content remains shared; the replacement
has its own activation state, reactive scope and generational widget identity.

The checker drives parent XTest events through Sway's native Wayland input.
It starts the Command consumer and a separate GUIdo process whose top-right
surface is labelled "Focus target". Clicking that surface while Space is held
must deliver native focus loss to the Command, clear its hold without an intent,
and deliver the release to the other client. Returning the pointer must restore
observed focus; a fresh Space gesture must still activate exactly once. The
tested Sway session uses `focus_follows_mouse yes`. This tests surface focus
transfer between clients, not widget focus arbitration within one process.
Both child processes are terminated and reaped on success or failure.

To run the focus target alone, append `--focus-peer` to the example's three
arguments. It is a verification surface, not a Command or a product action.

Run the complete checker:

```sh
python tools/check_guido_command_input.py \
  --probe lab/guido/target/debug/examples/command_input_probe \
  --scenes target/material-scenes.json \
  --font-dir $CONFORMANCE_FONTS \
  --sway-socket /path/to/sway-ipc.sock \
  --parent-display :92 --parent-window 0x200004
```

Supply the actual isolated Sway socket and its parent X11 output window ID;
the example values identify no particular running session. `WAYLAND_DISPLAY`
and `XDG_RUNTIME_DIR` must select that same live session. `swaymsg`, libX11 and
libXtst are required. If `CARGO_TARGET_DIR` is set, supply its example binary
instead of the default path. The checker verifies a single active `X11-1` output
at scale 1, at least 720 × 240 logical pixels so the surfaces cannot overlap,
and measures the parent window's origin before injecting input.
Before gestures, actual pointer delivery verifies that both surfaces are mapped;
a paint callback alone can precede native buffer presentation. The checker
replaces the Command during a held Space and during a held pointer press. Each
retired owner must be clean and distinct from its replacement, and the eventual
release must reach the replacement without an intent. Keyboard focus loss may
clear its hold before destruction; pointer retirement must issue the reference
capture-release request. This still does not execute an explicit native capture
API. A fresh pointer and key gesture must work after both replacements.

Acceptance for this fixture is one pointer intent after outside/reentry, one
Space intent after actual repeat delivery, one Enter intent, zero effects from
surface-focus interruption and subsequent unavailable input, successful native
focus recovery, no remaining hold, stable target publication and
painted focus matching native observation. Replacement must produce no intent
or publication from a retired owner. This is compositor-delivered injected
input, distinct from the mounted widget-tree tests. It does not execute a native
capture API, publish an accessibility tree or establish AT-SPI, screen-reader or
physical-device behavior. It is a verification consumer, not a reusable native
Command component. Native accessibility integration remains tracked in
[GUIdo issue #653](https://github.com/MalpenZibo/guido/issues/653).

## Native continuous-outline focus inspection

`outline_focus_probe` prepares the complete circular/quartic analytic focus ring
on a fixed opaque canvas, captures a GPU reference and presents the same prepared
paint on a native surface. The surrounding color comes from the checked focus
request; insufficient contrast fails before presentation. The checker compares
the entire opaque canvas, including the composited antialiased ring, on owned
nested Wayland outputs at scales 1/1.25/1.5/2/3. Complete outer bounds must fit
inside the diagnostic canvas. Other inspection scales must map its 160x96
logical canvas to integral physical pixels.

```sh
cargo build --release --locked --manifest-path lab/guido/Cargo.toml --example outline_focus_probe
python tools/check_guido_relief_native.py --outline-focus --probe lab/guido/target/release/examples/outline_focus_probe --output-dir target/outline-focus-native
```

Supply `--request` for another valid public outline-focus request and its actual
opaque surrounding color. The default uses the shared quartic conformance case,
RTL corners and explicit signed projection. Under a live parent Wayland display,
the checker requires Sway and grim; use the actual `CARGO_TARGET_DIR` binary path
when that variable is set. These diagnostic frames establish native pixel
realization, rather than accepted component appearance, input or accessibility.

## Native relief content inspection

Build the `relief_content_probe` example to compare complete face and text pixels
on an owned nested Wayland output. The checker isolates font discovery in its
owned children and uses the pinned Inter Regular master; the Relief Board also
registers the Medium and SemiBold masters from the same directory.
The probe rejects discovered system fonts, authors weight 400 through existing
theme resolution, and registers supplied bytes before each rendering lifetime.
The checker records the input hash and compares the whole opaque canvas against
a GPU reference at device scales 1/1.25/1.5/2/3. The default fixture exercises RTL placement and
supports text scale 2, including complete wrapping of `j Reconnect now`.
The backend GPU ink matrix uses the same complete longer label. Its expanded
reference preserves wrapping and admits the complete measured height; a deliberate
shortened-height render must expose a missing wrapped line while retaining ink.

```sh
cargo build --locked --manifest-path lab/guido/Cargo.toml --example relief_content_probe
python tools/check_guido_relief_native.py --probe lab/guido/target/debug/examples/relief_content_probe --content-font $CONFORMANCE_FONTS/Inter-Regular.otf --text-scale 2 --output-dir target/relief-content-native
```

If `CARGO_TARGET_DIR` is set, supply its example binary path. Run under a live
parent Wayland display with Sway and grim available. Complete
content reference preparation has a 120-second deadline for software renderers.
The checker retains the same pixel comparison threshold and terminates only its
own client and nested compositor.

Add `--content-focus` to compose an outline focus ring behind the complete face
and label. The checker supplies a guarded focus request using the actual canvas,
material family and unprojected footprint, with concurrent selected, focused and
hover states. The probe rejects mismatched material families or outlines,
projected focus and clipped outer bounds before capture or presentation. Its
optional sixth argument accepts the same public outline-focus request directly.
The GPU composition tests also verify retained opaque focus pixels and unchanged content wherever the ring is
transparent in both canvas polarities, directions and outline curvatures.

These diagnostic pixels prove native composition; they do not establish accepted
material appearance, interactive controls or assistive technology integration.

Add `--material-light` to the checker with `--content-font` to use the complete
material-lit relief request. Its default diagnostic retains the existing guarded
outline, pigment and foreground, supplying one physical key plus active bounded
ambient and interaction channels. The same probe accepts `--material-light`
before its positional arguments. It resolves the complete source through the
material-relief protocol and uses the backend's checked content preparation.
The checker requires explicit confirmation of the selected lighting mode from
the probe; `--content-focus` retains the actual face's independently guarded ring.
Custom request bytes reach the probe unchanged after strict JSON parsing for
fixture derivation, preserving source-number rejection before numeric rounding.
Whole-canvas native pixel agreement remains separate from accepted appearance,
component interaction, disabled-state styling and accessibility publication.

Use the optimized probe for the complete material-lit fractional-scale matrix:

```sh
cargo build --release --locked --manifest-path lab/guido/Cargo.toml --example relief_content_probe
python tools/check_guido_relief_native.py --probe lab/guido/target/release/examples/relief_content_probe --material-light --content-font $CONFORMANCE_FONTS/Inter-Regular.otf --content-focus --text-scale 2 --output-dir target/material-relief-content-native
```

Supply the actual font and `CARGO_TARGET_DIR` paths on the host. The same
120-second preparation deadline and complete opaque-canvas comparison apply.
