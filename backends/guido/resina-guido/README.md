# GUIdo backend primitives

This Linux backend prepares the existing Tier 0 opaque surface and focus-ring IR
as complete raw images for GUIdo. Resina IR remains independent of the renderer.
This package has its own Cargo workspace and lockfile so GUIdo's Linux runtime
dependencies do not enter the portable reference workspace.

GUIdo is pinned to maintainer commit
`e04cc6bc36f8bf3760d2a6fd2bf626f91cb3ccdb` from
[upstream image-cache correction](https://github.com/MalpenZibo/guido/pull/607).
That correction is currently unmerged. Required renderer tests cover image
identity, first-frame readiness and replacement at this exact revision.

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

## Verification

Linux needs Wayland and xkbcommon development libraries and a working GPU or
software Vulkan implementation. The required renderer test fails if a GPU
context cannot be created; it never skips rendering checks.

```sh
python -m pip install -r tools/requirements-schema.txt
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
RESINA_SCENES="$PWD/target/material-scenes.json" cargo test --manifest-path backends/guido/resina-guido/Cargo.toml --locked --features testing
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
