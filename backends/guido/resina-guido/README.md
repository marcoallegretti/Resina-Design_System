# GUIdo backend primitives

This Linux backend prepares the existing Tier 0 opaque surface and focus-ring IR
as complete raw images for GUIdo. Resina IR remains independent of the renderer.
This package has its own Cargo workspace and lockfile so GUIdo's Linux runtime
dependencies do not enter the portable reference workspace.

GUIdo is pinned to maintainer commit
`e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb` from
[upstream image-cache correction](https://github.com/MalpenZibo/guido/pull/607).
That correction was merged upstream on 2026-10-04. This package retains its
verified exact revision; required renderer tests cover image identity, first-frame
readiness and replacement at that pin.

## Native conformance limits

The checked paint and content paths are implemented primitives. The following
limits prevent claiming complete native command conformance:

| Requirement | Verified boundary | Current behavior |
| --- | --- | --- |
| Authored nonzero letter spacing | The pinned GUIdo [text command](https://github.com/MalpenZibo/guido/blob/e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb/src/renderer/commands.rs#L246-L268) and [shared shaper](https://github.com/MalpenZibo/guido/blob/e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb/src/renderer/text.rs#L112-L165) have no spacing input. | Native measurement/preparation returns `LabelMeasureError::LetterSpacing`. The zero-tracking probe profile does not certify the authored default label profile. |
| Initial key press versus repeat | The pinned [initial press](https://github.com/MalpenZibo/guido/blob/e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb/src/platform/input.rs#L850-L880) and [repeat](https://github.com/MalpenZibo/guido/blob/e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb/src/platform/input.rs#L950-L978) both emit the same `KeyDown` fields. | No conforming native activation adapter is supplied. Do not infer repeat from the semantic hold: an independent invocation can clear that hold while the physical key remains down. |
| Native assistive technology delivery | This package resolves/consumes portable [command semantics](../../../spec/47-command-accessibility.md) but supplies no native semantic-tree publication. | Headless accessibility checks and rendered pixels do not establish native screen-reader discovery or action delivery. |

The first two upstream API gaps were also verified at main revision
`04f67b4854f79cc080ba65fe4773f9f610b37874` on 2026-10-04. These are source-backed
capability limits, not claims that a newly implemented GUIdo Button failed a
native interaction test. The accessibility boundary describes this package's
implemented scope; it does not certify or diagnose every upstream integration.
Keep the authored Resina contracts intact while resolving these native gaps.

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
conversion errors above 1/1024 logical px fail explicitly. Pixel buffers contain
straight RGBA8, as required by GUIdo. Converting the owned raster buffer to an
`Arc` can allocate and copy once; subsequent source and prepared-paint clones
share that allocation. Raw images require no asynchronous image decoding.

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
family and the actual guarded content color. It returns a native `DrawCommand`
for the caller's render node. The rectangle is relative to the command origin;
parent transforms and clipping remain the caller's responsibility.

Preparation remeasures complete text and rejects a mismatch with resolved layout.
It preserves the offered wrapping width, uses centered line alignment and emits
no line limit or ellipsis. Unsupported tracking, fractional weight, excessive
numeric rounding and invalid color channels fail explicitly. Do not substitute
the ordinary GUIdo text widget without preserving these constraints: its layout
can shrink the text box to the longest line before drawing.

The native label test requires `RESINA_LABEL_FONT` to name the installed
DejaVuSans.ttf file and loads it before measurement and rendering. On Ubuntu with
`fonts-dejavu-core`, this is `/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf`.
Actual GPU readback checks Latin, expanded German and Arabic labels plus the
public 100/150/200% string-length matrix at three text scales and four device
scales: 72 frames. Character counts classify those ASCII test strings only; all
widths come from actual font shaping. Set `RESINA_LABEL_CAPTURE_DIR` to an output directory
to save each tested frame as a PPM for visual review. These are static typography
conformance probes, not interactive controls. Component accessibility and native
event delivery remain separate work.

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
paint. The scene-derived label profile explicitly selects `type.tracking.normal`
because pinned GUIdo rejects nonzero tracking; it does not silently alter the
public default typography. Tests include the existing six-label expansion/script
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
