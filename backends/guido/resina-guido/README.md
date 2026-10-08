# GUIdo backend primitives

This Linux backend prepares the existing Tier 0 opaque surface and focus-ring IR
as complete raw images for GUIdo. Resina IR remains independent of the renderer.
This package has its own Cargo workspace and lockfile so GUIdo's Linux runtime
dependencies do not enter the portable reference workspace.

GUIdo is pinned to upstream main revision
`ae29dc97a869434b51b768ad39b974e76db9a4dc`. It includes the merged
[image-cache correction](https://github.com/MalpenZibo/guido/pull/607),
[letter spacing](https://github.com/MalpenZibo/guido/pull/619) and
[key-repeat metadata](https://github.com/MalpenZibo/guido/pull/616), with
[text placement in the laid-out width](https://github.com/MalpenZibo/guido/pull/627)
and [image downsampling to the device texture limit](https://github.com/MalpenZibo/guido/pull/612).
Required renderer tests cover image identity, first-frame readiness, replacement
and spaced text at that pin.

## Live Linux runtime

The pinned GUIdo runtime creates Wayland layer-shell surfaces. Its
[connection setup](https://github.com/MalpenZibo/guido/blob/ae29dc97a869434b51b768ad39b974e76db9a4dc/src/platform/wayland.rs#L296-L317)
requires `wl_compositor` and `zwlr_layer_shell_v1`; an `xdg_wm_base` global alone
is insufficient. Check the actual session registry with `wayland-info` before
launching a live GUIdo product. A missing layer-shell global produces
`PlatformError::MissingLayerShell`. Live presentation also requires a usable GPU
adapter for the created surface.

Offscreen GPU tests and the static Material Board capture do not create Wayland
surfaces. They can pass on a host whose compositor lacks layer-shell. Their
pixels establish rendering evidence, without certifying live focus/input,
compositor behavior or native assistive technology. Those need separate tests
on a compatible compositor; changing portable Resina contracts cannot resolve a
missing host protocol. A separate test compositor can supply that protocol
without changing the active desktop session.

## Native conformance limits

The checked paint and content paths are implemented primitives. The following
limits prevent claiming complete native command conformance:

| Requirement | Verified boundary | Current behavior |
| --- | --- | --- |
| Initial key press versus repeat | The pinned [initial press](https://github.com/MalpenZibo/guido/blob/ae29dc97a869434b51b768ad39b974e76db9a4dc/src/platform/input.rs#L956-L960) and [repeat](https://github.com/MalpenZibo/guido/blob/ae29dc97a869434b51b768ad39b974e76db9a4dc/src/platform/input.rs#L969-L972) emit `KeyDown` with distinct `repeat` values. | No conforming native activation adapter consumes that flag yet. Do not infer repeat from the semantic hold: an independent invocation can clear that hold while the physical key remains down. |
| Mandatory line breaks | The pinned shaper's [line iterator](https://github.com/pop-os/cosmic-text/blob/0.19.0/src/line_ending.rs#L51-L72) ends lines only at LF, CR and CRLF, and consumes an LF followed by CR as one ending unless that LF completes a CRLF; [Unicode line breaking](https://www.unicode.org/reports/tr14/tr14-57.html#LB5) keeps only CR LF together. | Label measurement and preparation reject VT, FF, NEL, U+2028, U+2029 and any CR/LF sequence whose lines differ from Unicode line breaking with `UnsupportedLineBreak`. |
| Bidirectional paragraph separators | U+001C–U+001E separate bidirectional paragraphs without breaking lines. The pinned shaper [asserts](https://github.com/pop-os/cosmic-text/blob/0.19.0/src/shape.rs#L1357-L1359) that every paragraph in one line has the same direction and panics otherwise. | Label measurement and preparation reject them with `UnsupportedParagraphSeparator` before shaping. |
| Deep bidirectional embedding | When a wrapped line resolves to level 126, the pinned shaper [requests](https://github.com/pop-os/cosmic-text/blob/0.19.0/src/shape.rs#L1537) a right-to-left level above 125, the highest one, and panics. Text with n embedding or isolate initiators resolves to level 2n + 2 at most. | More than 60 initiators between line breaks fail with `BidiInitiators`, keeping every resolved level at 122 or below even when the bidirectional algorithm ignores terminators. |
| Native assistive technology delivery | This package resolves/consumes portable [command semantics](../../../spec/47-command-accessibility.md) but supplies no native semantic-tree publication. | Headless accessibility checks and rendered pixels do not establish native screen-reader discovery or action delivery. |

The repeat flag also reaches the
[`Container::on_key_down` repeat argument](https://github.com/MalpenZibo/guido/blob/ae29dc97a869434b51b768ad39b974e76db9a4dc/src/widgets/container/mod.rs#L1111-L1124).
Its availability does not establish input routing or live keyboard conformance.

The accessibility boundary describes this package's implemented scope; it does
not certify or diagnose every upstream integration. Keep the authored Resina
contracts intact while addressing the remaining native integration work.

## Integration

Call `prepare_surface`, `prepare_focus` or `prepare_surface_paint` with validated resolved IR, the actual
GUIdo device scale and a sampling grid from 1 to 8 samples per axis. Preparation
finishes synchronously before returning a `PreparedPaint`. Reuse this value
between frames, and prepare a replacement when geometry, appearance or device
scale changes. Publish the replacement only after successful preparation.

`prepare_surface_paint` consumes the complete body/navigation result and prepares
one image with shared sampling and linear-light integration. Focused paint reserves
the full outer ring bounds; unfocused paint uses the body bounds. Publish this one
prepared value to replace both channels together. The ring gap remains transparent,
and coarse pixels preserve combined body/ring coverage before quantization.

Use `paint.image_source()` with GUIdo's `image` and `ContentFit::Fill`, inside a
container sized to `paint.logical_size()`. Position that container at
`paint.origin()` relative to the IR's coordinate origin. Allocate enough parent
space for the complete painted bounds, including negative focus-ring origins.
Keep the parent placement aligned to device pixels and use the same device scale;
additional transforms or fractional parent placement can resample the image.

Preparation encloses the complete silhouette or placed outer focus contour in an
outward-aligned device-pixel viewport. Coordinates use GUIdo's binary32 values;
conversion errors above 1/1024 logical px fail explicitly. This budget also checks
GUIdo's binary32 origin-plus-size addition at both far image corners before
rasterization; individually representable origin and size do not certify those
corners. Parent transforms remain the caller's responsibility. Pixel buffers contain
straight RGBA8, as required by GUIdo. Converting the owned raster buffer to an
`Arc` can allocate and copy once; subsequent source and prepared-paint clones
share that allocation. Raw images require no asynchronous image decoding.

GUIdo creates its device with `wgpu::Limits::default()`, whose 8192 px
`max_texture_dimension_2d` applies even when the adapter supports more, and it
downsamples a larger raster with only a log warning. That would replace the
prepared device-grid paint, so preparation fails with `TextureLimit` when either
axis exceeds `MAX_TEXTURE_DIMENSION`. The CPU pixel limit alone does not bound
one dimension. A GPU test checks the constant against the native device. An
application that supplies its own GUIdo device must allow at least this dimension.

The canonical CPU renderer enforces its pixel and sample limits before raster
allocation. Invalid scales, unrepresentable bounds, excessive coordinate
rounding, invalid sampling and resource limits return diagnostic errors. This
paint preparation supplies no input, text, accessibility semantics or component
state management; consuming applications must provide those behaviors.

## Native focus transfer

After native layout, map opaque focus-scope IDs to mounted GUIdo `WidgetRef`s
using `FocusBinding`. Pass the validated headless `FocusTraversalResult` and
these bindings to `resina_guido::request_focus`. The owner supplies the actual
localized order and eligibility to headless resolution; this adapter does not
derive eligibility from native enabled state.

A null target returns `None` without inspecting bindings or changing current or
pending focus. For a selected target, all binding IDs must be nonempty and unique,
and mounted native widgets must not be aliased by different IDs. The selected
binding must exist and be mounted. Unselected unmounted bindings are permitted.
Validation failures queue no request and leave any existing pending request intact.

Success queues GUIdo's deferred native request and returns `RequestedFocus`.
It does not acknowledge transfer. `is_focused()` observes actual native focus,
subscribing to focus changes inside a GUIdo reactive scope. It returns an error
if the handle no longer names the original mounted widget. A later request can
supersede an earlier request, and native removal can cancel it. GUIdo queues a
handle: rebinding that handle before the frame can change its destination; the
owner must keep bindings stable during transfer and recover explicitly after
tree changes. The observer detects a changed identity rather than accepting it.

Resolve and publish focused paint from observed native focus. The owner remains
responsible for keyboard events, focus scope boundaries, recovery, scrolling,
activation and accessibility semantics. This adapter does not establish complete
component keyboard conformance.

## Complete label drawing

Load the selected font before GUIdo initializes its font systems. Supply an
explicit family to `measure_command_label` when resolving portable label layout.
Select and verify that family and its fallback policy before calling the adapter. Then call `prepare_command_label` with that IR, the same
family, the actual guarded content color and the actual device scale. It returns
a native `DrawCommand` for the caller's render node. The rectangle is relative to
the command origin; parent transforms and clipping remain the caller's
responsibility.

Resolved letter spacing reaches GUIdo unchanged in logical pixels and follows
every shaped glyph, including the last, in both measurement and drawing. Text
scaling is already part of the resolved value. GUIdo shapes drawn text in device
pixels, so a line that exactly fills its box can wrap again at a device scale:
scaled advances and spacing round independently of the scaled width. Following
the [layout drawing rule](../../../spec/46-command-label-layout.md), a label whose
fitted measurement wrapped no text is drawn without wrapping, which keeps one
line per hard break at every scale. Text with a mandatory break GUIdo does not
honor fails first; see the limits above. A
wrapping label keeps its offered width and is shaped again at the device scale.
GUIdo measurement reports no individual lines, so every prefix ending at a
character boundary is shaped at both scales: greedy breaking makes the first
differing break change the line count of the prefix ending at one of the two
break positions. A different prefix height or widest line fails with
`ScaledLineMismatch`. A prefix cut can shape differently from its full context,
through cursive joining, kerning or bidirectional resolution at the cut, so a
break at such a cut is checked only as closely as its prefix shaping matches. The
check costs time quadratic in label length; it suits command labels, not long
text. Lines that exactly fill the box can fail at common scales such as 1.5, as
can a prefix ending inside a word that exactly fills the box even when the drawn
lines would agree; choose another valid layout for that device scale rather than
drawing different lines. Prepare again when the device scale changes.

Preparation remeasures complete text and rejects a mismatch with resolved layout.
It keeps the offered box width, uses centered line alignment and emits
no line limit or ellipsis. Fractional weight, metrics or spacing beyond the
binary32 precision budget, invalid device scales, device-scaled metrics GUIdo
would clamp or drop (`ScaledMetrics`) and invalid color channels fail explicitly. Do not substitute the ordinary GUIdo text widget without preserving
these constraints: its layout can shrink the text box to the longest line before
drawing.

The native label test requires `RESINA_LABEL_FONT` to name the installed
DejaVuSans.ttf file and loads it before measurement and rendering. On Ubuntu with
`fonts-dejavu-core`, this is `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`.
Actual GPU readback checks Latin, expanded German and Arabic labels plus the
public 100/150/200% string-length matrix at three text scales and four device
scales: 72 frames, each with 0.15 px tracking before text scaling and no ink a
line below the resolved box. That check cannot see a line GUIdo drops beyond its
shaping height, so every unwrapped label must also match the ink bounds of the
same text drawn in a box 200 px wider, and wrapped labels rely on the
device-scale line check. GUIdo's shaping height is the box
height with a 50 px minimum. Hard breaks, an empty paragraph, a leading break and an RTL
Hebrew/Latin pair draw one inked line per nonempty paragraph at device scales 1,
1.5 and 3, as do CR, CRLF, consecutive CRLF and LF before CRLF. VT, FF, NEL, U+2028,
U+2029 and CR/LF sequences GUIdo splits into different lines fail measurement,
and preparation rejects them in IR from another producer. U+001C–U+001E fail
measurement, including the mixed-direction texts that would otherwise panic.
Sixty left-to-right embeddings over Arabic-Indic digits, the level-122 worst case,
measure in a wrapped line; 61 initiators, or 63 isolates each followed by an
ignored terminator, fail measurement and preparation with `BidiInitiators`.
Character counts classify the ASCII test strings only; all widths come from
actual font shaping. Spacing evidence measures “Save” four spacings wider
than unspaced text, and its drawn ink span changes by the three interior
spacings at device scales 1 and 2. A wrapped fixture whose lines exactly fill
their box agrees at device scale 2 and fails with `ScaledLineMismatch` at 1.5
and 3, as do two labels whose rebreak at those scales keeps the complete height
and widest line. Set `RESINA_LABEL_CAPTURE_DIR` to a fresh output directory
to save each tested frame as a PPM for visual review. These are static typography
conformance probes, not interactive controls. Component accessibility and native
event delivery remain separate work.

The separate [glyph-ink test](tests/label_ink.rs) verifies that a command's
advance box does not crop a centered DejaVu Sans “j” at 60 logical px. It
compares the adapter's exact resolved label rectangle with a wider diagnostic
rectangle at an identical glyph position, at device scales 1, 1.25, 2 and 3.
Every corresponding pixel must match, with visible ink outside the resolved
box on each scale. The production label box is unchanged. This establishes overhang for the tested
upright glyph and unclipped render node, not arbitrary fonts, transforms or
surrounding clip contours. Optional PPM captures use exclusive creation and
never replace an existing frame.

## Command content composition

Use `prepare_command_content` for a checked `CommandSnapshot`, the same verified
font family, device scale and sampling count. Resolve the snapshot with current
activation, explicit hover, a stable reserved target and accessible semantics
before native preparation. Its existing guards validate shared front size/origin
and actual content-contour containment. Native preparation remeasures complete
text, takes its foreground from the body's readability
result, and prepares material and navigation paint. A failure returns no command
pair and preserves its diagnostic cause.

The result is exactly two native draw commands, in order: the prepared RGBA image
and complete centered text. Both use the command's physical origin. The image
rectangle includes its prepared negative origin and full focus/side extent;
text stays relative to the front origin. Insert both into the same render node
and replace them together, including when consuming a sampled motion command.
Do not apply the image rectangle's origin a second time to the node or label.
The prepared RGBA source uses no deferred image decode or tint.

The caller still owns the common environment/typography context, font mapping,
glyph overhang verification and surrounding clipping. Color conversion and
antialiasing are native operations; nominal IR contrast alone is not evidence of
final pixel contrast. This API prepares content paint, not invocation, keyboard
routing, an accessibility tree or a complete Button. The owner still publishes
the snapshot's target and semantics through its native mechanisms, replaces
stale snapshots after state/layout changes, and checks current activation when
an action is delivered.

Required Linux GPU tests compose actual text over authored Light/Dark material
paint with the authored Label profile, including its wide tracking. Tests include the existing six-label expansion/script
matrix at three text scales and four device scales, all four command phases
across Cast, opaque Frost fallback and Elastomer in both schemes, and four sampled
Elastomer motion times, plus unfocused replacement: 188 first composite frames.
Every composition consumes a checked snapshot with an explicit test interaction
state and a reservation derived from the same profile's neutral rest body.
Compositions use the authored surrounding color. Subsequent material-only and
text-only readbacks identify actual glyph coverage. Every covered pixel footprint
must remain inside the content contour on uniform opaque pigment; solid native
foreground/background pairs must retain at least 4.5 contrast. Missing lines,
material changes outside coverage and diagnostic failures are checked. These
fixtures do not establish all fonts/scripts, capability tiers or full component
conformance. Set `RESINA_COMMAND_CAPTURE_DIR` to save PPM captures for visual
review.

The same test adds 256 complete frames from the authored Command anatomy:
standard and primary variants in both unmodified themes, all four phases,
independent focus, text scales 1 and 2, and device scales 1, 1.25, 2 and 3.
It preserves each variant's material, color, content and form bindings and uses
the anatomy's 4.5:1 content and 3:1 edge requirements. These frames pass the same
snapshot, native ink, containment, placement and pixel-contrast checks. Their
explicit test dimensions and padding are not calibrated component defaults;
native interaction and assistive technology delivery remain separate work.

## Activation key events

`activation_key_event` maps a GUIdo key event against the current committed
activation state to an optional portable activation event. Pass that event to
`resolve_activation` for a Command or `resolve_toggle_activation` for a Toggle,
then commit the returned state before delivering its activation effect.

Space and Enter presses are accepted without Ctrl, Alt, Shift or Logo. Caps Lock
is a latch and does not alter these keys. The native repeat flag is preserved;
repeat suppression and eligibility belong to the resolver. A held key's matching
release is forwarded even after modifiers change. Other chords, keys and non-key
events return `None` for their appropriate owners. This mapper does not consume
an event, request focus, route pointers or install a widget handler. Owners must
still report actual focus, availability and interruption through the activation
contract and route held-key termination.

The pinned GUIdo [input implementation](https://github.com/MalpenZibo/guido/blob/ae29dc97a869434b51b768ad39b974e76db9a4dc/src/platform/input.rs)
publishes press/repeat metadata and retains the pressed key identity for release.
Tests check both command keys over all modifier combinations, repeated delivery,
changed-modifier releases, unrelated input and one-activation Command/Toggle
sequences. These tests verify event translation, not compositor delivery or a
complete native control.

## Toggle content composition

Call `prepare_toggle_content` with a checked `ToggleSnapshot`, the verified font
family, actual device scale and sampling count. It returns exactly three commands:
track material/navigation image, selected thumb image, then complete measured
external label. Insert them into one render node in that order and replace all
three together after preparation succeeds. A failure returns no array and
preserves the label, track or thumb diagnostic cause.

Preparation uses the snapshot's checked external-label foreground directly;
the track's foreground is not the label's color. The caller must actually supply
the checked uniform opaque label backdrop in its scene. The adapter does not paint
an invented background behind the label. Complete text is remeasured with the
supplied family. Signed placement preserves RTL coordinates without changing
text or internal layout. Every native origin, extent and origin-plus-size corner
stays within the existing 1/1024 logical px precision budget.

Thumb geometry remains part-local in portable IR. Preparation translates the
selected endpoint before outward device-grid alignment, then samples local paint
against that placed grid. It avoids shifting an already rasterized local image
through a second fractional placement. Lighting remains physical; only logical
thumb/label placement changes in RTL. Parent transforms, their alignment and
clipping remain caller-owned, as do glyph-overhang and actual font-fallback
verification. These commands supply no native event routing, accessibility-tree
delivery or complete interactive Toggle.

Required Linux GPU tests cover 396 first compositions: actual Light/Dark themes,
Cast/opaque Frost fallback/Elastomer, off/on selection, rest/hover/pressed/disabled
and unfocused replacement, both directions and scales 1, 1.25 and 2. Wrapped Latin
and Arabic labels use 200% text scaling with the authored Label tracking. The external
label uses a distinct, validated semantic foreground. Each part's raw image is
checked against the canonical CPU renderer on the placed sampling grid; opaque
GPU pixels must agree within one byte. Separate label masks identify real glyph
coverage, complete lines, slot containment and solid-glyph contrast against the
actual backdrop. Invalid scale/sampling and approximate label measurements fail.
Set `RESINA_TOGGLE_CAPTURE_DIR` to save the tested PPM frames for visual review.
These fixtures verify composition transport and typography, not complete
component styling, every script/font, motion or native accessibility conformance.

The composition test adds 512 frames using the authored Toggle anatomy and both
unmodified themes. They cover four phases, independent focus, off/on selection,
both directions, text scales 1 and 2, and device scales 1, 1.25, 2 and 3. The
track and thumb retain their authored material, selected color, content and form
bindings; the external label uses its own authored color role. Each frame passes
the same complete snapshot, placed CPU/GPU comparison, native label containment
and 4.5:1 pixel-contrast checks. Part edges require 3:1 against their actual
adjacent colors. Dimensions, insets and label padding remain explicit fixtures,
not calibrated component defaults. Native input and assistive technology delivery
remain separate obligations.

## Checked thumb travel

Resolve `resolve_toggle_travel` from a current complete snapshot, explicit scalar
dynamics, retained unprojected initial state and elapsed seconds. Call
`prepare_toggle_travel_content` with that checked view, verified font family,
actual device scale and sampling grid. It prepares the same complete three-command
array at the sampled thumb bounds. Translation still precedes device-grid
alignment. Track navigation, complete label, checked palette, target and semantics
come from the original snapshot; the backend performs no separate interpolation.
Replace all commands together after success. Preserve unprojected state for
retargeting, and replace the underlying snapshot after context/state changes.

The [travel contract](../../../spec/56-toggle-thumb-travel.md) supplies immediate
Cast/reduced-motion endpoints without hiding invalid inputs. Required GPU tests
add 32 frames across Light/Dark Elastomer, both directions/selections, four sample
times and scale 1.25. The existing complete pixel-grid and label checks apply to
these frames. These coefficients are arithmetic fixtures, not calibrated motion.
Native clocks, interaction routing and assistive technology publication remain
owner responsibilities; no complete animated product Toggle is supplied.

The ordinary-travel GPU fixture explicitly sets reducedMotion false before
resolving paint and snapshots, and asserts spring policy, opposite initial
placement, intermediate non-endpoint placement and exact settled placement.
Reduced-motion policy is tested separately in the portable snapshot matrix.

## Placed surface and text preparation

The public `prepare_surface_paint_at` and `prepare_command_label_at` functions
accept a parent-relative origin. Their returned image/text command already
includes that placement; do not translate it a second time. Surface placement
precedes device-grid sampling, and both functions apply the existing coordinate
precision checks. The separate [native Material Board](../../../lab/guido/README.md)
uses these APIs to inspect the public opaque material catalog at real device
scales.

## Checked Slider content

`prepare_slider_content` accepts a complete
[Slider snapshot](../../../spec/77-slider-snapshot.md), an explicitly mapped font,
the actual device scale and a sampling grid. It returns track, thumb and native
label commands together. Both parts use their actual layout origins before
device-grid alignment. The thumb image includes its checked navigation paint;
the track has no separate focus ring. The label uses the snapshot's checked
foreground and origin, and is measured again with the supplied native font.
Replace the complete command array only after preparation succeeds.

Label, track and thumb failures identify the failed part and preserve their
underlying error. This path does not alter authored typography or silently
substitute measurements. Native input, pointer routing, clocks and
assistive technology publication remain component-owner responsibilities.

Required GPU tests cover 192 complete opaque frames: Cast, Frost and Elastomer,
both axes and directions, rest/focused/disabled/read-only/pointer-preview states,
scales 1/1.25/2, and wrapped English/Arabic labels at textScale 2. Every part's
placed sampling grid is checked against the canonical CPU paint, opaque native
pixels against the prepared image, and actual glyph coverage against the
reserved label slot and checked backdrop. Labels use nonzero tracking.
Unrepresentable spacing, approximate measurements and invalid raster inputs fail
before command publication.
`RESINA_SLIDER_CAPTURE_DIR` saves the tested PPM frames.

These shared conformance fixtures deliberately use structural geometry and a
black/white palette. Visual inspection confirms placement, focus and complete
text, but their appearance is not accepted as release component styling: material
differentiation, proportions, theme coverage and calibrated motion still need
the component and Lab implementation. No interactive Slider widget is supplied
by this paint-preparation API.

## Verification

Linux needs Wayland and xkbcommon development libraries and a working GPU or
software Vulkan implementation. The required renderer test fails if a GPU
context cannot be created; it never skips rendering checks.

```sh
python -m pip install -r tools/requirements-schema.txt
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
RESINA_SCENES="$PWD/target/material-scenes.json" RESINA_LABEL_FONT="$RESINA_LABEL_FONT" cargo test --manifest-path backends/guido/resina-guido/Cargo.toml --locked --features testing
cargo clippy --manifest-path backends/guido/resina-guido/Cargo.toml --all-targets --features testing --locked -- -D warnings
```

The public scenes cover Light and Dark, all four material families, opaque bodies
and focus rings. Tests check their first rendered frame at scales 1, 1.25, 2 and
3 while decoding is held, then replace sources in the same surface.
Complete focused-only and rest requests authored in the shared catalog also
exercise single-image replacement and removal of the navigation channel. Opaque,
partial-alpha and clear pixels are checked against the prepared straight pixels,
allowing one byte of GPU rounding after premultiplication. Preparation tests also
check complete bounds, shared image storage and explicit failures. These checks
establish the implemented paint slice, not complete GUIdo component conformance.
The native focus test uses real mounted widgets and frames to verify deferred
transfer, superseding requests, null outcomes, validation before side effects,
pending-request preservation and stale-handle diagnostics after disposal.

## Sampled Toggle part paint

`resolve_toggle_part_motion` returns complete checked part paint with retained raw
pigment/depth motion state. Pass `part_paint().paint()` to the existing surface
preparation path; no native interpolation is required. The [portable contract](../../../spec/57-toggle-part-motion.md)
owns motion policy and each sample's contrast/geometry checks. Resolve the current
track first and supply its actual pigment as thumb adjacency before complete
snapshot assembly and optional checked thumb travel.

Required GPU readback retains 72 static frames and adds 216 sampled frames over
both parts/selections/focus states, three families, times 0/0.25/100 and scales
1/1.25/2. Intermediate spring responses must differ from both endpoints. Set
`RESINA_TOGGLE_PART_CAPTURE_DIR` for isolated diagnostic PPM captures. These tests
do not certify complete component styling, calibrated motion, native scheduling,
input or assistive technology publication.

The complete content test adds 180 joint motion frames to its existing 428
static/travel frames. Actual current track pigment feeds thumb adjacency before
snapshot validation; stale static adjacency fails. Independent response channels
and travel retain raw retarget state, current switch semantics and a stable target.
Coverage includes both themes, three families, both directions/selections,
disabling during motion, reduced-motion endpoints and wrapped text at textScale 2.
Placed-grid/GPU and complete-label checks remain required. Use
`RESINA_TOGGLE_CAPTURE_DIR` to inspect the complete frames. Their broad diagnostic
track/thumb proportions and explicit arithmetic springs are not release styling
or calibrated material defaults. Native input, clocks and AT publication remain
unimplemented component obligations.
