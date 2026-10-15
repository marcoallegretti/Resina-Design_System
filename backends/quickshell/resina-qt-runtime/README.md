# Qt command runtime (0.1.0)

This separately locked native workspace implements the Resina-owned
Rust/QML runtime boundary. CXX-Qt 0.8.1 packages Rust-owned command activation
behind a Qt `QObject` facade in the `Resina.Runtime` QML plugin. The portable
reference owns semantics, Qt owns object lifetime and native publication, and
consumers own product effects. Qt and binding dependencies stay outside the
portable workspace and normative artifacts.

The native content path additionally resolves authored label typography and the
existing complete command-label policy in Rust, using actual Qt measurement
for both natural and fitted extents.

## CommandController

Import `Resina.Runtime` and instantiate one `CommandController` per command.
Its initial state is disabled, unfocused and without a hold. Bind it to that
command's native node for the same lifetime; names are content, not identities.
On the Qt UI thread, call `dispatch(eventJson)` with one event from the existing
[activation contract](../../../spec/42-command-activation.md). The adapter bounds
event source to 64 KiB and uses the shared parser, preserving duplicate-member,
unknown-field and event validation before availability guards. This resource
bound does not change normative identifiers.

Success returns `true` for an accepted event, including unavailable invocation
without an intent. Failure returns `false`; read the non-notifying, read-only
`lastError` property immediately. Failure preserves state/publication and emits
no capture or activation effect.

The single read-only `publication` property contains complete existing
`ActivationResult` JSON. Rust commits next state before the Qt facade updates the property and emits
`publicationChanged`, capture effects or product intents. The Rust borrow has
ended before any observer runs. Qt checks QObject lifetime between notifications;
destroying the controller during a notification suppresses subsequent effects
without accessing destroyed Rust storage. Every accepted event
publishes once, including unchanged state. Recursive dispatch during publication
notification fails explicitly. Once publication completes, product and capture
callbacks can synchronously dispatch another event.

`captureRequested(id, acquire)` supplies the exact capture effect after commit.
The native gesture owner must maintain continued pointer delivery, cancel if
capture cannot be maintained, and route capture loss, deactivation and removal
through the lifecycle. Pointer-item removal queues cancellation on the surviving
controller, rather than on the removed item. That task observes capture release
so a later gesture reusing the native pointer id cannot be cancelled by the old
removal. `activated()` delivers one product intent after state and
capture publication. Execute effects from these signals once; retained
`publication.activate` and `publication.capture` do not authorize replay.

Supply actual focus observations and current availability through their events.
Mutation must stay on the Qt UI thread. Wrong-thread calls return false without
accessing Rust state or writing a diagnostic from another thread. Use deferred Qt destruction
and cancel active gestures before removing their owner. This controller does not
acquire focus, perform pointer capture, measure text, render paint or provide a
Button. Complete component input/accessibility integration remains separate.

## Native command keyboard owner

[`ResinaCommandKeyboard.qml`](../resina-qml/qml/ResinaCommandKeyboard.qml) is a
nonvisual Qt focus item owning one `CommandController` for the same lifetime.
Its read-only `controller` and immutable `publication` string expose that existing
Rust lifecycle. Supply availability through `controller.dispatch`; the item
starts disabled. Preserve its `enabled` binding to committed availability.
Consumers receive product intents through `controller.activated`, after commit.
Place the decorative command view inside it, using the same current activation
for preparation; invalidate pending/published drawing whenever that state changes.
This owner does not prepare paint automatically.

Only an eligible, visible item with actual Qt active focus in a visible active
window handles command keys. Native focus observations feed Rust; a focus request
alone never sets the reference state. Hiding the item or its window clears its
native item focus; consumers restore focus through native navigation or a
successful native request. Eligibility does not replace the separate focus
observation. Unmodified Space starts on press and
activates on actual release; Return and keypad Enter activate on press. Keypad
metadata does not modify Enter. Other keys and new modified gestures propagate.
Held releases still terminate after modifiers change. Native repeat presses reach
Rust with their repeat flag; repeat releases do not terminate or activate.
Shortcut override reserves these focused command keys without executing an
additional product shortcut. No synthesized click is delivered.

Focus loss cancels keyboard holds. Effective item hiding/disabling, parent/window
replacement, window hiding/deactivation and destruction cancel interruption
through Rust. Brief focus loss is retained across publication callbacks, even
when focus returns before the callback ends. Synchronization deferred during
publication reads current native state, and routing checks eligibility again
after callbacks. `routingError` exposes rejected delivery; interrupted delivery
is cancelled rather than replayed. Cancel before deferred removal or external
opacity/occlusion changes that Qt visibility cannot describe. Native thread and
deferred destruction requirements remain those of the controller.

This is keyboard/focus integration, not a Button, pointer owner, animation or
accessible control. It is ignored by accessibility. Explicit semantics/native AT
publication and the existing toolkit focusability discrepancy remain separate.
Layout still owns the complete stable pointer reservation; this item does not
derive or claim a target from its width/height.

Acceptance connects actual focus and key routing to Rust activation and a
checked pressed/focused body/glyph snapshot, including cancellation without
stale release activation. The checker executed on Qt 6.11.2 with offscreen,
X11/Xvfb and nested Wayland/Weston 14.0.2 (X11 backend, Pixman renderer).
Adversarial publication callbacks, modified termination, repeat releases,
hidden item/window recovery, product revocation and actual armed QObject
destruction were exercised. Direct `QKeyEvent` cases are injected Qt delivery.
An additional parent XTest run traversed Weston and `wl_keyboard` into
spontaneous Qt events with nonzero native scan codes and real platform repeat
metadata. It verified Space repeat/release, modified held release, Enter timing,
modified new-gesture exclusion and shortcut exclusion. This establishes the
tested compositor/toolkit path, not physical-device or screen-reader qualification.
Run focus-dependent GUI checks serially on a shared display.

## Native command pointer owner

[`ResinaCommandInput.qml`](../resina-qml/qml/ResinaCommandInput.qml) extends the
keyboard owner with one compiled `CommandPointerItem`, using the same
`CommandController`. Qt Quick/GUI dependencies remain inside this separate
native workspace. The helper is nonvisual and ignored by accessibility.
Native test executables must load this SDK's runtime libraries. Python verification
hosts explicitly load their pinned Qt Quick module before the compiled plugin,
so a system Qt installation cannot supply a mismatched Quick dependency.
Consumers still own layout, paint preparation, semantic publication and product
effects. This is primary mouse and keyboard integration, not a complete Button.

Set its `coordinateFrame` to the native item whose local coordinates represent
the body/hit-region frame, normally the decorative command view. Size the input
owner to the actual carrier rectangle reserved by layout, independently of the
paint contour. Call `prepareTarget(requestJson)` with the existing complete
[hit-region request](../../../spec/40-hit-region-ir.md), including the actual
environment, complete visual footprint, available bounds, component minimum
and neighboring occupied regions. Use one stable reservation over the supported
feedback domain, and validate actual body coverage through the command snapshot.
Preparation never infers neighboring reservations or changes target policy.
Disabled availability retains the reservation.

Rust strictly resolves that request and preserves exact endpoint predicates for
membership. Qt maps each delivered point into `coordinateFrame` once; no rounded
JavaScript endpoint comparison decides membership. The full target must fit the
native carrier, client window and every clipping ancestor. Unsupported scale,
rotation or distorted clipping fails explicitly. Reparenting, carrier size,
window size, coordinate-frame placement or clipping changes withdraw publication
and cancel a held mouse gesture, observed before delivery and during the GUI
frame phase. Call `invalidateTarget()` before external occlusion/opacity or late
layout changes outside those observations, then reserve and prepare again.
The read-only `targetPublication` is complete hit IR or empty after withdrawal;
`pointerError` exposes preparation/delivery failure. Recursive target preparation
during publication fails, and changed context cannot publish a successful target.

The read-only `hovered` observation on the input helper and compiled owner comes
from native window enter and unsynthesized mouse positions, without consuming
those events. Each identified mouse retains its own latest observed position;
hover is true when any observed mouse lies inside the current Rust target. Qt
maps window positions directly into the coordinate frame once and uses the same
exact membership predicate. Window-enter positions are provisional: Qt's Wayland
[enter path](https://github.com/qt/qtbase/blob/v6.11.2/src/plugins/platforms/wayland/qwaylandwindow.cpp#L1243)
supplies no seat device identity, so the next identified mouse event
supersedes that position rather than creating a fictitious second mouse.
Qt device-object identity is not hardware identity: the pinned Wayland backend
reports mice through a seat's primary pointing device. Window leave also supplies
no device identity and withdraws all retained observations. Separate-device
fixtures use injected Qt objects; native multi-seat and hardware qualification
remain open.
Captured movement still updates hover even though Qt Quick suppresses ordinary
item hover delivery during an exclusive grab. A second mouse may hover while the
held mouse is outside. Focus, availability, hold membership and capture effects
never manufacture or suppress that independent observation. Disabled hover
therefore remains representable through the existing command-state projection.
Touch-synthesized mouse positions and non-mouse devices do not supply hover.

Target withdrawal makes hover false. Re-preparation checks retained native
window positions against the new valid target, including a stationary mouse.
Window leave, hiding/deactivation, effective owner hiding, window replacement
and destruction discard observations; device destruction removes that device's
observation. Recovery uses fresh native observations rather than guessed global
cursor coordinates. As for targeting, consumers must invalidate before external
occlusion/opacity changes that this reserved-layout owner cannot observe.
Prepare command drawing with both current `controller.publication.state` and
`hovered`; invalidate pending/published drawing on either notification. The
read-only QML alias avoids reentrant binding evaluation when a notification
callback withdraws the target. Paint preparation remains consumer-owned.

Only an unmodified primary mouse press inside the complete target starts a
gesture. Pointer activation does not request or require keyboard focus. Rust
commits before the adapter acquires Qt's actual exclusive grab for the delivered
event point. Native device-object identity plus event-point identity form the
opaque pointer ID for that live device; different devices cannot release each
other's holds. The first held keyboard or pointer gesture remains the owner.
Motion/release continues outside the target and client while captured. Release
membership is freshly computed, including after modifiers change. No synthesized
click or composed double-click action is dispatched. Native grab loss, device
removal, disabling/hiding, deactivation, target withdrawal and owner destruction
route cancellation through Rust. Semantic invocation releases the native grab;
the subsequent raw release cannot activate again. Deferred controller replacement
keeps its old connection until interrupted publication can finish cancellation.
Keep controller/frame mutation and deferred destruction on the UI thread, and
cancel before removing a live owner. One pointer owner supplies raw mouse events
for each controller; other consumers use the semantic invocation path.

Acceptance connects the stable target, native exclusive capture, Rust activation
and checked hover/pressed/disabled body and glyph drawing. The Rust bridge
exercises all shared exact membership vectors. Qt fixtures independently check mapped membership,
actual grab state per delivered device, outside/re-entry/release, focus
independence, competing keyboard/device gestures, modified termination, native
ancestor clipping, mapping invalidation, hide/disable, product revocation,
semantic supersession, capture loss and actual armed QObject destruction. Hover
checks include independent devices, provisional enter identity, disabled motion,
stationary target replacement, device retirement and callback-time withdrawal.
The helper's hover property remains read-only and agrees with the native owner.
Direct `QMouseEvent` and second-device fixtures are injected Qt evidence. The
nested Weston 14.0.2/Pixman run additionally observed spontaneous Qt events
through `wl_pointer`, including motion/release outside the client, invocation
and capture-loss cancellation. Parent XTest injection establishes that tested
compositor/toolkit path, not physical mouse, touch, pen or device-hotplug
qualification. The native Wayland run additionally verifies hover on captured
outside/re-entry movement. Motion and complete accessibility publication remain
separate integration requirements.

## CommandLabelResolver and native content

`CommandLabelResolver.resolveLabel(requestJson, producer)` synchronously runs
[complete label layout](../../../spec/46-command-label-layout.md). Its backend
request has exactly these fields:

- `themeSource` and `externalSources`: authored theme JSON and explicit source
  strings, using the existing theme compiler. No file access or ambient theme.
- `environment`: the existing validated environment snapshot. Its text scale
  reaches semantic typography once; its direction maps logical padding.
- `text`, `minimumSize`, `maximumSize`, `padding`: the existing layout inputs.
- `fontFamily`: `{role, name}`, an explicit registered native family mapping for
  the resolved Label family role. A mismatching role is diagnostic.

The request source is bounded to 8 MiB and rejects duplicate members, unknown
fields and positional record representations. These are backend transport
limits, not changes to portable label semantics. Success returns `{label}`
containing existing `CommandLabelIr`; failure returns `{error}` and no label.
There is no new portable snapshot or typography wire contract.

Supply the actual `ResinaLabelMeasure` QML instance as `producer`. The native
bridge calls its `measureJson` method directly on the Qt application thread.
It retains only a `QPointer` during that synchronous call and rejects a missing,
destroyed, foreign-thread or malformed producer. It does not enter an event loop,
retain native pointers in Rust, or approximate glyph widths. Native failures
propagate through the existing Rust measurement-error channel. Producer responses
are bounded to 64 KiB and strictly parsed, including duplicate rejection.
Keep the producer and its registered font alive for the whole operation; use
deferred Qt destruction and the same font context for later drawing.

[`ResinaCommandLabel.qml`](../resina-qml/qml/ResinaCommandLabel.qml) composes this
resolver with the native measurement/drawing pair. Call
`prepare(requestJson, guardedSrgbColor)` on the UI thread. Its read-only `layout`
contains the resolved label IR after drawing preparation succeeds; `paintReady`
reports prepared native content. Failure clears layout and hides prior content,
with `paintError` preserving the cause. Replacement first withdraws readiness;
recursive preparation during publication fails without changing the outer
operation. The item reserves the complete command
front size and positions the label at its Rust-resolved box. It is decorative,
has no input target and is ignored by accessibility.

The composing owner still must validate the label against the body's uniform
content contour and supply its guarded foreground, retain common provenance,
publish coherent paint/target/semantics, and own native gestures and actions.
The label helper does not provide any of those channels or a Button.

## CommandSnapshotResolver

`CommandSnapshotResolver.resolveSnapshot(requestJson, producer)` prepares existing
command content, paint, stable target and accessibility together. It shares the
label resolver's actual native producer, strict parsing and 8 MiB request limit.
This is a backend request/result, not a new portable snapshot wire contract.

Supply every field explicitly:

- `content`: the complete `CommandLabelResolver` request above. Its single
  compiled theme/environment owns typography, paint and target resolution.
- `body`: `surface`, `appearance`, `commandAppearance`, `foregroundRole`,
  `adjacentColor`, `postTreatmentBackdrop`, `surroundingColor`,
  `minimumContentContrast` and `minimumEdgeContrast`. These are existing authored
  surface/appearance inputs and color records. Both nullable backdrop fields
  are required. Color conversion uses the existing sRGB fallback resolver.
  The surface template must have exactly the neutral rest state; current states
  come exclusively from the canonical activation/hover projection.
- `activation`, `hovered`, nullable `description`, and `focusable`: existing
  command state and semantic inputs. The native owner must supply actual focus
  observations and its chosen availability policy.
- `availableBounds`, `componentMinimum`, `occupiedRegions`: explicit existing
  target inputs. Bounds are named `{x, y, width, height}` objects. Coordinates
  share the command-front origin; consumers account for scene placement.

Rust resolves complete content first, uses its front size for the neutral and
current body, reserves the target against the neutral maximum extrusion, then
runs the existing snapshot validator. This checks state/direction coherence,
complete label-box containment in the uniform body contour, current silhouette
coverage, available/occupied target bounds and complete accessibility semantics.
Contrast, surrounding colors and unsupported geometry retain their diagnostics.
No padding or contrast threshold is invented to repair an invalid request.

Success returns `{snapshot: {label, paint, hitRegion, accessibility}}`, with each
member in its existing IR format. Any failure returns only `{error}`. The
operation retains no partial state and subsequent valid requests can recover.
This endpoint prepares static state endpoints; it does not publish an AT tree,
render a body, interpolate motion, capture input, or implement a Button.
A composing native owner must prepare every visual channel before coherently
publishing the result and retain the common font/environment context.

## Checked native paint

`CommandSnapshotResolver.resolvePaintedSnapshot(requestJson, producer, optionsJson)`
runs the same complete snapshot preparation and then renders that exact checked
paint through the existing `resina-qml`/bounded CPU raster path. Options require
named `x`, `y`, `deviceScale` and `samplesPerAxis` fields. They specify actual
scene placement, native pixel density and explicit sampling (1 through 8).
Duplicate/unknown members, positional records and options sources exceeding
64 KiB are diagnostic. Existing raster size/work limits and native coordinate
precision guards still apply.

Success returns `{snapshot, paintSource}`. `paintSource` is the existing
decorative QML Item with a prepared sRGB PNG and complete body/focus paint.
A rendering failure returns only `{error}`, even if snapshot resolution succeeded.
Create the source in the owning Qt engine and place the returned item at its
`paintOriginX`/`paintOriginY`; these already include the requested scene placement.
Wait for its `paintReady` before publishing or showing a composite. Do not apply
placement again or let ambient Image scaling resample the prepared profile.
Keep the native pixel density aligned and prepare again after placement/scale
changes. Retire prior native objects through Qt's owning-thread lifecycle.

The returned label remains existing complete layout IR. Prepare its drawing with
the same registered font mapping and its checked body foreground; this endpoint
renders the body/focus channels, not label glyphs. It provides no input or AT node,
and does not define motion or a complete Button. The composing owner remains
responsible for coherent readiness/publication of all channels.

Native paint acceptance compares actual Qt software window readback against both
the returned prepared PNG and an independently invoked public
`resina-command-raster` request built from the original authored inputs, native
label size and projected states. Executed Light/Dark rest, hover, pointer-held,
disabled and focused frames at DPR 1, 1.25 and 2 preserve the existing one-byte
pigment/alpha comparison bound. Fractional scene placement is included; complete
focus and extrusion are captured. The state inputs are synthetic. These are
body/focus raster and native transfer checks, not complete component glyph,
native gesture, motion or accessibility qualification. Hardware graphics,
Wayland and the integrated command graphics-renderer matrix remain unverified.

## Coherent native command view

[`ResinaCommandView.qml`](../resina-qml/qml/ResinaCommandView.qml) composes the
checked snapshot, body/focus image and complete native glyph drawing. Call
`prepare(requestJson, optionsJson)` on the GUI thread using the same requests
as `resolvePaintedSnapshot`. The view must already belong to its actual window.
Register and retain the explicit font before preparation.

Its read-only `snapshot` string publishes the existing four checked channels
only after body and glyph preparation succeeds. `paintReady` and default visibility
become true together with that publication; `paintError` retains failures.
Replacement withdraws the old publication and hides/retires old native paint
before preparing another request. The native local PNG can finish loading after
`prepare` returns, so observe readiness/error rather than treating the return
as a completed publication. Image errors or a five-second native preparation
timeout withdraw the pending request. Recursive preparation during pending work,
publication or withdrawal is diagnostic and preserves the outer operation.
`invalidate()` cancels pending work and withdraws a published result; a valid
later preparation can recover. An owner replacing a pending request after a
controller/content change must invalidate it before preparing the new request.
External visibility, opacity and ancestor occlusion remain consumer-owned.

The view owns its command-front implicit size. Its local glyph box comes directly
from checked label IR and uses the body's checked foreground. Set its position
before preparation, then compute the actual scene origin with
`view.mapToItem(null, 0, 0)` and use `view.nativeDeviceScale` for `deviceScale`.
These options describe the sampling context, not a second placement transform.
Do not force a different front width/height or add scale/rotation. Native
translation is checked within the existing 1/1024 logical-pixel precision budget;
the compiled `CommandSnapshotResolver.matchesNativeContext(item, x, y,
deviceScale)` checks the complete native scene transform. It requires a live
UI-thread `QQuickItem` in a native window, finite placement, matching actual
window pixel density and an affine translation with an exact identity linear
part. Missing/non-item/foreign-thread contexts and invalid numeric inputs return
false. It retains no native object. The same translation predicate guards native
pointer targeting. Sampling the origin and unit basis alone is insufficient:
a projective matrix can preserve all three samples while distorting the body
elsewhere. Scale, rotation, shear and projective transforms are rejected before
publication and on the GUI frame check. Keep ancestor clipping/occlusion and
actual surrounding/backdrop colors consistent with the supplied paint context.

Own position, size, parent, window and pixel-density changes withdraw readiness.
Before scene-graph synchronization, a GUI-thread
[`afterAnimating` check](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/items/qquickwindow.cpp)
also rejects changed ancestor coordinate contexts. Call `invalidate()` before
external layout/transform mutations, especially from other late frame callbacks,
and prepare again after layout settles. The view never silently resamples its
stored raster after a context change. Queued completion observes only the current
pending request, and native child destruction remains Qt-owned.

This is a decorative static view, with no input target or native accessible node.
The published semantic member is data for a future native control owner; it does
not implement AT publication or authorize an action. The existing controller
still owns current activation and consumers own product effects. Complete
Button, motion, native gesture routing and accessibility qualification remain
separate dependencies.

Acceptance is coherent publication and actual combined body/glyph drawing, with
no stale visible channels after failure or cancellation. The compiled view
checker executed authored Light/Dark styles, asymmetric LTR/RTL padding,
Latin, expanded German, Arabic/mixed and blank paragraphs at 100.06%, 150% and
200% text scale. Static hover, pointer-held, disabled and focused fixtures also
reach native drawing. Captures at DPR 1, 1.25, 2 and 1.3 compare against separately
assembled native body/label primitives with the existing one-byte image bound.
This establishes composition; independent font shaping/drawing and public raster
agreement retain their separate earlier evidence. Publication observers see
complete readiness, the snapshot is read-only, recursive publication/withdrawal
is rejected, pending context changes cancel, ancestor translation/distortion
withdraws visible paint before rendering, and valid preparation recovers. The
pinned Qt regression also verifies a projective matrix whose origin/unit samples
appear to be a translation, rejects preparation and live publication under that
matrix, checks withdrawn pixels and recovers after restoring identity.
Actual pending QObject destruction and native PNG decode-error reporting were
also executed. These are synthetic state and rendering/lifetime fixtures, not
native gesture or AT evidence. The same composition/lifetime matrix also passed
at DPR 1 and 1.25 as a native Wayland client under nested Weston 14.0.2/Pixman, using Qt
software client readback. This is not compositor-output or physical-screen capture.
Hardware/graphics-renderer composition, all font/fallback policies, motion and
complete control qualification remain open.

## Build and verification

Install the tested Qt 6.11.2 development SDK with its Core/Qml tools and a C++17
compiler. Keep the host's Qt ABI aligned. From the repository root on Linux:

```sh
export QMAKE=/path/to/Qt/6.11.2/gcc_64/bin/qmake
export CXX_QT_EXPORT_DIR="$PWD/target/qt-runtime-export"
export CARGO_TARGET_DIR="$PWD/target/qt-runtime"
cargo build --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --locked
python tools/prepare_qt_runtime_module.py \
  --library target/qt-runtime/debug/libresina_qt_runtime.so \
  --metadata-dir target/qt-runtime-export/qml_modules/Resina/Runtime \
  --output-dir target/qt-runtime-import
CARGO_TARGET_DIR="$PWD/target" cargo build -p resina-raster --bin resina-command-raster --locked
LD_LIBRARY_PATH="$(dirname "$QMAKE")/../lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" \
  cargo test --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --locked
cargo clippy --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --all-targets --locked -- -D warnings
python -m pip install -r tools/requirements-qt-native.txt -r tools/requirements-schema.txt -r tools/requirements-render.txt
python tools/check_qt_command_runtime.py --module-path target/qt-runtime-import
python tools/check_qt_command_label_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
python tools/check_qt_command_snapshot_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
python tools/check_qt_command_keyboard_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
python tools/check_qt_command_pointer_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
QT_SCALE_FACTOR=1.25 python tools/check_qt_command_paint_runtime.py \
  --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --raster target/debug/resina-command-raster --output-dir target/qt-command-paint-1.25
QT_SCALE_FACTOR=1.25 python tools/check_qt_command_view_runtime.py \
  --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-command-view-1.25
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" python tools/check_qt_native_availability.py --report-only
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" python tools/check_qt_native_availability.py
```

For a locally nested Wayland run, launch Weston 14.0.2 on an available X11 display
in a private runtime directory, then select that socket explicitly:

```sh
mkdir -p target/wayland-runtime
chmod 700 target/wayland-runtime
DISPLAY=:0 XDG_RUNTIME_DIR="$PWD/target/wayland-runtime" weston \
  --backend=x11 --renderer=pixman --shell=desktop --no-config --idle-time=0 \
  --socket=resina-wayland --log="$PWD/target/weston.log"
# In another terminal, using the same native runtime module and font:
export XDG_RUNTIME_DIR="$PWD/target/wayland-runtime"
export WAYLAND_DISPLAY=resina-wayland
export QT_QPA_PLATFORM=wayland
python tools/check_qt_command_keyboard_runtime.py --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

For compositor-delivered input, set `resina_weston_window` to the actual X11 output
window ID reported in that Weston log. Parent injection also requires the
X11/XTest runtime libraries:

```sh
python tools/check_qt_command_keyboard_runtime.py --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --parent-x11-display :0 --parent-x11-window "$resina_weston_window"
python tools/check_qt_command_pointer_runtime.py --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --parent-x11-display :0 --parent-x11-window "$resina_weston_window"
```

The keyboard checker rejects that mode on a non-Wayland Qt platform and requires
spontaneous native arrival, scan codes and observed repeat metadata. The pointer
checker requires DPR 1, spontaneous native pointer arrival, and exclusive grabs
for the actual event's device. Standalone
Wayland AT-SPI invocation was also executed with the existing probe and retained
the same `conformant: false` disabled-focusability observation.

The [`qt-runtime` CI job](../../../.github/workflows/reference.yml) installs
the pinned Qt 6.11.2 base/declarative SDK and its bundled ICU through aqtinstall
3.3.0, together with the pinned Python host. It builds, tests and lints this
separate workspace and executes the compiled controller, content, snapshot,
keyboard and pointer checks. Software paint/composition checks run
at DPR 1, 1.25, 2 and 1.3; capture artifacts remain available on failure. This
job's AT-SPI step measures real invocation and availability under Xvfb using
`--report-only`. Malformed/missing native observations and incorrect action
delivery still fail; the known disabled-focusability discrepancy is recorded as
`conformant: false` rather than certified as passing accessibility. The strict
command above remains the qualification gate. A local reproduction does not
prove that the hosted workflow has executed.

Use a fresh import output directory. The packager verifies generated metadata
and uses Qt's expected plugin filename. Add this import root to the engine or
`QML_IMPORT_PATH`. Actually loading this layout was verified. Consumers need
neither a subprocess resolver nor Python; verification hosts use Python to drive
Qt.

The library explicitly exports Qt's generated `qt_plugin_instance()` factory
through Rust. Rust's `cdylib` export filtering otherwise hides that C++ symbol:
QML type registration can happen during library initialization even when the
plugin factory cannot load. The compiled checker therefore calls
`QPluginLoader.instance()` before testing QML objects; type creation alone is
insufficient evidence for this packaging boundary.

Acceptance is native QObject lifecycle delivery matching shared transitions,
rejection without effects, committed state before intent, safe publication
reentrancy and product availability revocation followed by actual AT-SPI delivery.
The compiled checker instantiates actual Qt objects and prepares shared case
states through legal lifecycle operations. It checks full publication, read-only
properties, capture effects, intent ordering/counts, rejection and recovery.
These are **synthetic event deliveries**, not native keyboard/pointer evidence.

Content acceptance is actual native natural/fitted measurement reaching Rust's
complete label equations and the same resolved IR reaching native drawing
preparation, with rejection clearing stale content and later recovery. The
content checker executed both authored themes, LTR/RTL padding, Latin, expanded
German, Arabic and independent/blank paragraphs at 100%, 100.06%, 150% and 200%
text scaling. It independently checks the authored 14 px / 500 / 1.34 / 0.15 px
Label values and the layout equations; matching producer and IR values alone
would not detect applying text scale twice. It also rejects duplicate/unknown
members, positional records, missing fonts, impossible complete height and
missing/malformed producers. This is native measurement and drawing preparation,
not a capture of a complete painted component or native input evidence. The
separate label-pair captures establish drawing for their tested font/renderer
matrix; complete content/background containment is checked by the snapshot preparation
endpoint described above.

Snapshot acceptance is actual native text reaching coherent guarded paint,
reserved target and complete semantics through Rust's existing snapshot validator.
The compiled checker executed both authored themes, Latin, expanded German and
mixed Arabic paragraphs at 100.06% text scaling across every positive public
command-state case. It independently checks each IR schema, projected signals,
phase precedence, all semantic dimensions, focus-channel ownership and target
stability across feedback. Layout is checked with the portable exact-rational
arithmetic oracle. The native fractional German height that previously failed
now succeeds at the original 80 px minimum height under conservative rounding.
Invalid containment, clipped/occupied targets, conflicting template states,
impossible contrast and malformed requests produce no partial snapshot and
recover. These state fixtures are synthetic inputs, not native focus, gesture,
rendering or accessibility publication evidence.

The native checker requires Linux, a display and AT-SPI services. Executed on Qt
6.11.2 with X11/Xvfb, real Press actions reached the loaded Rust plugin: disabled
commands produced no intent, an enabled command produced one, its product callback
committed unavailable state, and the next Press produced none. Reports identify
`binding: rust-qt-runtime`. The strict checker still fails discoverable-disabled
focusability because Qt clears it when disabling an item. `--report-only` retains
that mismatch and never hides execution failures; no contract was weakened.

### Quickshell hosting boundary

The same diagnostic controls can run in a Quickshell `FloatingWindow`. The host
runner accepts Quickshell 0.3.1 or 0.3.2 and validates its build/runtime Qt 6.11.2 versions,
adds the packaged module import root, and executes Quickshell directly. It does
not instantiate the Python lifecycle or a second Qt engine. For example:

```sh
RESINA_QUICKSHELL_HOST=/absolute/path/to/quickshell \
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" \
python tools/check_qt_native_availability.py --report-only
```

Reports identify `host: quickshell-0.3.1-qt-6.11.2` or
`host: quickshell-0.3.2-qt-6.11.2` from the verified host version, separately from the binding.
Standalone reports identify `host: qt-quick-6.11.2`. Host versions do not certify
a distribution's build or patches. Missing native nodes remain execution
failures, including with `--report-only`.

The [`qt-runtime` CI job](../../../.github/workflows/reference.yml) builds
unmodified Quickshell 0.3.2 release revision
`4f508be500dea6e5732cc3d50382a0048b17e7b1` with the same Qt 6.11.2 SDK used
to compile the Rust plugin. Its installation, build and native-probe steps are
the reproducible build procedure. The SDK additionally requires `qtshadertools`;
the host requires CLI11, libdrm, jemalloc, Wayland 1.24 or later and Wayland
protocols 1.41 or later. Qt 6.11.2's private headers reference `wl_fixes`,
which older Wayland headers do not declare; use matching headers and libraries.
The configuration retains Wayland/layer-shell and X11, while disabling the crash
handler, screencopy, compositor-specific integrations and optional system services.
Those disabled features are outside this qualification target.

The released-host probe loads the packaged `Resina.Runtime` module and records
AT-SPI discovery and Press delivery through X11/Xvfb. Its diagnostic report
preserves native semantic mismatches; it does not establish complete Command
acceptance, layer-shell behavior, reload lifetime or screen-reader verification.
The standalone and released-host reports are saved as separate CI artifacts.

**Unmodified Quickshell 0.3.1 currently fails this probe.** An executed build
from its release source, with the official Qt 6.11.2 SDK, loaded the Rust plugin
but published an AT-SPI application with no window children. Quickshell
[destroys its initial QCoreApplication before constructing the GUI application](https://github.com/quickshell-mirror/quickshell/blob/v0.3.1/src/launch/launch.cpp#L282-L291).
Qt's application teardown
[clears registered accessibility factories](https://github.com/qt/qtbase/blob/v6.11.2/src/gui/accessible/qaccessible.cpp#L572-L576),
including the Qt Quick factory installed by
[module initialization](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/util/qquickglobal.cpp#L311-L313).

A separate scratch build called Qt's private `QQuick_initializeModule()` after
constructing the GUI application. Actual AT-SPI Press then reached the Rust
controller for all four expected deliveries, with complete committed results
and no stale activation. The only semantic mismatch remained discoverable
disabled focusability. **This is experimental evidence from a patched host,
not working integration in the released Quickshell host.** The experiment used
software rendering and X11/Xvfb; Quickshell's optional services, Wayland and
specialized X11 modules were disabled. It proves no layer-shell behavior,
hot-reload lifecycle or other toolkit factories. No private bootstrap or
replacement accessibility factory is installed by this production adapter.

**Unmodified Quickshell 0.3.2 passes native discovery and action delivery in this
probe.** The release removes the initial application teardown. Executed with
Qt 6.11.2 and Wayland 1.24.0, the packaged Rust module exposed all three fixture
commands through AT-SPI on X11/Xvfb. All four Press deliveries produced their
expected committed results: unavailable commands did not activate, the enabled
command activated once and became unavailable, and its next action did not
activate again. The report retains the same discoverable-disabled focusability
mismatch and identifies `host: quickshell-0.3.2-qt-6.11.2` with
`binding: rust-qt-runtime`.

This is native bootstrap and guarded-action evidence for the recorded build;
it does not qualify the complete reusable control, layer-shell, reload or a
screen reader. No process-wide private bootstrap or replacement accessibility
factory is installed by this adapter.

This verifies a runtime dependency, not a complete Command/Button, Toggle or
Slider. The [native label pair](../resina-qml/README.md#native-label-measurement)
separately verifies fractional sizing and authored tracking through native
measurement and drawing. Complete reusable-control qualification in released
Quickshell, physical-device/screen-reader operation and calibrated motion remain
unverified.
The integrated static profile below now verifies composed command input and
native accessibility in the standalone host. Hosted native runtime checks have
also passed; the sampled profile section links the executed evidence.
The decorative view now prepares body and complete glyph drawing together. The
input owner connects actual focus/key routing and native mouse capture to current
Rust activation and stable target mapping. The integrated static control below connects native hover and complete
publication to AT actions; the sampled profile below adds guarded motion.

### Integrated static command control

`ResinaCommandControl.qml` composes the native input owner and decorative view
into one reusable static command profile. Rust continues to resolve the existing
portable contracts; Qt owns focus, primary mouse capture, native text, UI-thread
lifetime and accessibility publication. The consumer owns `activated` effects.
No portable schema, state rule, checker threshold or native event semantics are
changed by this composition.

Required properties are `configuration` (the complete JSON snapshot request
accepted by `CommandSnapshotResolver`), `disabledFocusable` (explicitly `false`
for this profile) and `samplesPerAxis` (the existing raster sampling policy).
Register the specified font before providing configuration. Author the neutral
rest body template, environment, content, available and occupied bounds,
component minimum, contrast guards and description explicitly. The placeholder
activation, hover and focusability members must themselves have valid transport
shapes; native observations replace them after strict Rust parsing. QML never
normalizes untrusted JSON before duplicate-member rejection.

```qml
Content.ResinaCommandControl {
    configuration: validatedAuthoredTemplate
    disabledFocusable: false
    samplesPerAxis: 4
    onActivated: application.reconnect()
}
```

`configuration` is a source string, not an assertion of prior qualification.
Before arming input the component resolves every enabled/disabled, actual-focus,
hover and pressed static endpoint supported by the profile and verifies that all
retain the same stable target. Endpoint preflight applies the renderer's existing
placement, coordinate precision, sampling and resource guards without allocating
or sampling an unused frame. Actual publication still samples and encodes the
complete current body/navigation paint and waits for native decoding. The first
complete native drawing must be ready.
Carrier implicit size reserves the whole target, including rest extrusion and
minimum hit space; the decorative front is offset within that carrier. Native
translation, density, clipping and full target fit must satisfy the existing
pointer and paint context guards. Nontranslation transforms fail closed.

Readonly `snapshot` aliases a native read-only QString publication, avoiding
recursive QML string bindings when a consumer changes availability/configuration
from its publication callback. The owner commits only a validated complete view
snapshot and checks the current preparation revision after observer delivery.
`snapshot` is the coherent four-channel publication; `ready` additionally
requires native paint, target and armed input readiness. The decorative view and
accessible node become visible only with that complete readiness, including
first publication and requalification. Snapshot existence alone is not action
authorization. `preparationError` reports failure.
`invoke()` routes the semantic action only through an armed available lifecycle;
its Boolean describes accepted delivery, not a product outcome. `activated` is
emitted from the committed Rust activation transition. `reconfigure()` cancels
held input and requalifies the current source after externally repairing native
context. Authored-property, visibility, native-window, placement and density
changes automatically retire the publication and schedule requalification.
Unsupported contexts stay diagnostically withdrawn until repaired.

A feedback update withdraws old paint and semantics while preparing the latest
complete snapshot. It retains its already qualified native target and armed
input owner: image decoding is not authorization for a new gesture. This allows
rapid hover/down/up to reach the lifecycle without waiting for an event-loop
turn between events. Multiple feedback events can coalesce before a frame; no
stale snapshot is retained as current. Existing pointer/key releases remain
routable if the new-gesture gate closes. A configuration/context failure
withdraws input as well, cancels held gestures, publishes no partial channels
and requires successful requalification before arming again.

The explicit excluding-disabled policy follows
[specification 47](../../../spec/47-command-accessibility.md). Availability loss
relinquishes actual Qt focus before publishing unfocused disabled semantics;
Resina does not simulate focus loss. Discoverable disabled focusability remains
unsupported by this Qt profile and is rejected, including when currently enabled.
The original native availability checker and fixture remain unchanged. Ordinary
momentary press is not published as toggle/checked state.

The native accessible node belongs to the input carrier, with the exact snapshot
name and description, Button role and explicit focusability. Decorative children
are ignored. Qt 6.11.2's existing role-based `accessiblePressAction()` route
calls `invoke()`; a simultaneous attached `onPressAction` handler would duplicate
the native Press action in this toolkit and is deliberately not installed.
There is no custom accessibility tree or private Qt bootstrap.

Independent acceptance for this increment is a prepared actual Qt instance
connecting complete guarded drawing, stable native targeting, observed hover and
focus, keyboard/mouse activation, a singular AT-SPI Button and a real Press
through Rust into exactly one product effect, with configuration/context failure
and stale actions producing none. Run:

```sh
python tools/check_qt_command_control_runtime.py \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-control-captures
python tools/check_qt_command_control_accessibility.py \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

The input checker verifies rapid delivery, captured outside/reentry, Space/Enter,
semantic supersession, committed product revocation, configuration replacement
while held, reentrant availability revocation during snapshot delivery,
malformed/duplicate source, impossible contrast, invalid sampling, strict policy
rejection, transform/visibility withdrawal and repaired placement, plus authored
light/dark and German/Arabic paragraph content. Captures verify actual native
body/glyph drawing and changed pressed feedback. It uses injected Qt events;
optional `--parent-x11-display` and `--parent-x11-window` traverse an explicitly
named nested Weston output into spontaneous Wayland pointer/key delivery with
native scan codes and exact-once effects. Parent injection requires DPR 1 and
calibrates placement from observed native coordinates, without assuming Wayland
global window positions. It is still injected input, not physical qualification.

Executed local evidence uses Qt 6.11.2 software rendering: complete-control
checks passed offscreen at DPR 1, 1.3 and 2, and nested Wayland at DPR 1 and 1.25.
The DPR 1 Wayland run also traversed parent XTest into native pointer/key events.
Actual AT-SPI publication and Press passed in both X11/Xvfb and nested Wayland.
Native captures were inspected for body, glyph and pressed feedback; the existing
paint oracle also matched software readback after the shared preflight extraction.
The original strict availability probe still rejects exactly its documented
Qt discoverable-disabled focusability mismatch.

The separate AT-SPI checker runs on an isolated bus in a standalone Qt 6.11.2
host. It requires exactly one Button with complete content and no decorative
children, verifies actual enabled/sensitive/focusable/focused state and one Press
entry, executes native `DoAction`, observes product revocation and proves a stale
Press produces no second effect. It qualifies only this explicitly excluding
static profile; the older discoverable-disabled report retains its mismatch.

This integrated path does not qualify calibrated motion, physical devices,
multi-seat hardware identity, screen-reader operation, hardware GPU rendering,
released Quickshell accessibility or complete GUIdo Command/Button integration. The optional
sampled profile below integrates guarded native motion with these same
publication and cancellation boundaries. Material calibration and released-host
control qualification remain necessary. Hosted CI has executed
this path successfully, separately from the local native Wayland evidence.

### Released Quickshell authored updates

The [released-host probe](../../../conformance/quickshell/command-updates-probe.qml)
mounts `ResinaCommandControl` and replaces its complete authored configuration.
Run with the packaged module and an unmodified Quickshell 0.3.2 built and running
against Qt 6.11.2:

```sh
python tools/check_quickshell_command_updates.py \
  --host target/quickshell-build/src/quickshell \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --alternate-font /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf \
  --output-dir target/quickshell-command-updates --scales 1 1.25 2
```

Use a fresh output directory. Install the existing schema, render and native Qt
test requirements. The checker validates the actual host before starting the
scenario sequence. The default software/offscreen path records actual device
density; `--platform xcb` or `--platform wayland` requires a live display.

The 47 scenarios cover light/dark Latin, expanded German and mixed Arabic/Latin
paragraphs at four text scales, changed typography and asymmetric LTR/RTL padding,
an explicitly loaded alternate font, both published Command variants in Light
and Dark, focused standard/primary variants, authored strong-outline focus fallback,
failed focus contrast and focused recovery, unavailable state, malformed configuration,
unsupported focus policy and rotation, and recovery after each rejection.
Every consumer snapshot observation is checked against its current authored
source and interaction state, including literal pigment bindings from the current
inline theme and the authored material/form/phase response. This fixture lookup
does not resolve arbitrary DTCG aliases or multi-source themes. The selected
literal foundation depth, response and physical key light independently determine
the expected lighting offset and swept silhouette/edge-interior bounds. Final
snapshots and callbacks must retain those bounds and inset placement; this check
does not certify every analytic contour segment. Callback labels and reserved
targets must equal the independently checked final values in each fixed authored
context, while interaction paint follows each callback's own state.
Focused cases request native item focus and independently record actual item/window
focus alongside final and callback state. Each observation retains the raw item,
window, visibility and availability values read at that callback or final sample.
The reported native focus is their conjunction. A request that does not become
observed focus fails within the existing case deadline. Focused rest retains both
`rest` and `focused`; body and navigation bindings and silhouettes must agree.
Offscreen focus observations do not establish compositor focus policy or physical
input. Before publishing a pending prepared frame, the control synchronizes native
focus and pending interruption, then validates the resulting preparation
generation. Final natural and fitted native measurements reach the independent
rational layout checker. These remeasurements verify final
layout arithmetic; they do not observe the control's internal measurement calls.

Each successful case saves a PNG and its complete snapshot. Pixel checks require
a representative uniform body sample above the label to retain the current pigment
and foreground ink on every measured line at the placed label coordinates; they do
not establish every glyph's shape, universal fallback coverage or product styling
acceptance. Focus pigment selection, fallback and unrounded contrast are checked
independently against the authored opaque assignments and surrounding color.
Focused captures also require the opaque indicator color at the
midpoint of each exterior side, using the specified gap and stroke. Their capture
backdrop is the authored opaque surrounding color used by the focus contrast guard;
the checker also verifies that background. These samples
do not certify every rounded corner or the complete perimeter; inspect the focused
captures as well. Logs retain native font diagnostics: Qt's
[software glyph bounds](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/scenegraph/adaptations/software/qsgsoftwareglyphnode.cpp)
and [FreeType metric fallback](https://github.com/qt/qtbase/blob/v6.11.2/src/gui/text/freetype/qfontengine_ft.cpp)
can recover from a bounding-query failure independently of painting. A diagnostic
alone does not prove missing painted glyphs. Inspect captures
and diagnostics together. Invalid cases must withdraw paint, target and accessible
publication before recovery. These authored-update tests do not deliver pointer
or keyboard input, inspect AT-SPI, test reload/closure, sample motion or establish
screen-reader acceptance.

### Compositor-delivered Command lifecycle

[`check_quickshell_command_lifecycle.py`](../../../tools/check_quickshell_command_lifecycle.py)
runs [the lifecycle probe](../../../conformance/quickshell/command-lifecycle-probe.qml)
as a released Quickshell configuration in a live Wayland session. The probe mounts
the control in a layer-shell `PanelWindow` at the output origin and exposes its
observations and lifecycle operations through `IpcHandler`. Input reaches Qt
through the compositor:
[`wayland_virtual_input.py`](../../../tools/wayland_virtual_input.py) drives
`zwlr_virtual_pointer_v1` absolute motion and buttons and a persistent
`zwp_virtual_keyboard_v1` keyboard. Held keys use the compositor's repeat
information, so Qt produces its own auto-repeat.

```sh
python tools/check_quickshell_command_lifecycle.py \
  --host /path/to/quickshell --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/quickshell-command-lifecycle
```

`WAYLAND_DISPLAY` and `XDG_RUNTIME_DIR` must select a compositor providing
`wlr-layer-shell`, virtual pointer and virtual keyboard with one output at
unit scale. Headless Sway 1.12 (`WLR_BACKENDS=headless`, `WLR_RENDERER=pixman`)
and nested niri 26.04 pass all 34 cases; CI uses the Ubuntu 24.04 Sway package.
The checker verifies the host version, fails on QML type, reference and
invalid-context errors in the host log and counts every consumer effect:

- pointer activation, release outside the target, and Space and Enter held through
  auto-repeat each produce the canonical number of effects; each held key must
  deliver at least two repeats to the controller;
- Tab traversal away from and back into the control and requested focus are
  published only when observed; a focus
  request while another surface holds keyboard focus is not published, and focus
  follows once it returns; availability withdrawal clears focus and suppresses
  activation;
- Space or pointer held across configuration replacement, unmount, hidden and
  destroyed surfaces, and soft and hard Quickshell reload, delivers no effect; a
  reload must retire the previous engine generation, observed through Quickshell
  0.3.2 IPC, which answers only while exactly one generation exists; a fresh
  gesture afterwards delivers exactly one effect from the current generation;
- consumer callbacks, reached by pointer and by Space, that replace the
  configuration, withdraw availability, invoke the control, hide, unmount, hide
  and unmount together, or destroy its surface observe committed state and
  produce exactly one effect; a callback-time `invoke()` is a separate semantic
  activation and produces exactly one additional effect, and a key press after
  callback-time availability withdrawal produces none.

When the control is destroyed, its pending prepared frame is retired and its
keyboard owner ignores focus synchronization queued before destruction, so
deferred work does not run in the destroyed QML context. The checker sets
`QS_NO_RELOAD_POPUP`: Quickshell's reload popup runs in its own engine generation
at the top-left of the output, and a hover during its fade-out keeps it open until
it is closed or replaced by another reload. Physical
devices, multiple seats and outputs, fractional scale and screen-reader operation
are not covered.

### Released Quickshell Command accessibility

[`check_quickshell_command_accessibility.py`](../../../tools/check_quickshell_command_accessibility.py)
runs the same lifecycle probe in a private D-Bus session with the AT-SPI bus and
Qt accessibility enabled. It takes the same arguments and compositor session as
the lifecycle checker and inspects the actual native tree:

- the application exposes exactly one Button, without decorative descendants,
  with the complete name and description, enabled, sensitive, focusable and
  focused state, and exactly one `Press` action, which an available or
  unavailable Command must accept;
- a real AT-SPI `Press` produces exactly one committed consumer effect;
- withdrawn availability is published as disabled, unfocusable state and the
  next `Press` produces no effect;
- configuration replacement keeps the native node, republishes the current
  non-ASCII name and acts exactly once;
- unmounting the control or destroying its surface removes the Button, and a
  `Press` on the retired node produces no effect;
- availability withdrawn from the consumer callback of an AT-SPI `Press` rejects
  the next `Press`;
- the unsupported discoverable-disabled focus policy fails with its diagnostic
  and exposes no Button.

All eight cases pass in headless Sway and nested niri 26.04 with Quickshell 0.3.2
and Qt 6.11.2. `--screen-reader` additionally starts Orca on the same bus with an
English locale and isolated settings and reads its generated presentation.
After focus moves to the probe's accessible focus target and Orca's output stops
growing, focusing the control must produce consecutive speech for its name, role and
description and a braille line containing all three. The Orca version and the
matched output are saved to `screen-reader.json`. Orca 50.3 with
speech-dispatcher passes locally; CI does not install Orca. With default settings
Orca announces no change when availability is withdrawn from the focused control,
so availability announcements are not verified. Audible output, other screen readers and physical input are not
covered.

### Released Quickshell Command motion

[`check_quickshell_command_motion.py`](../../../tools/check_quickshell_command_motion.py)
runs the lifecycle probe with the explicit motion channels of the
[motion request](../../../conformance/ir/command-motion-request.json) and authored
preference variants. The probe logs every published motion sample with its
snapshot; after the run, each frame keeps the authored dynamics and its complete
snapshot and motion sample are checked against the request in effect, the
projected state set and the independent spring equations. Input
reaches the control through the compositor:

- holding Space publishes several unsettled spring frames; the release retarget continues from the position
  and velocity the previous trajectory had reached; each sampled epoch's clock
  advances with elapsed native time within 10 % or 50 ms; one channel settles
  before the other, both settle, and no frame follows;
- compositor hover enters and leaves the target through spring frames. Qt's
  Wayland client types the seat pointer as a touchpad when the compositor offers
  pointer gestures, so hover accepts mouse events from mouse and touchpad devices;
- focus loss during motion republishes frames without the focus cue;
- switching to the reduced-motion preference or withdrawing availability during a
  press cancels the held gesture without an effect; reduced motion then publishes
  only immediate settled frames;
- moving the control during a press cancels the held gesture without an effect
  and publishes frames at the new position;
- reduced motion and Cast publish immediate settled frames, and Frost with
  reduced transparency keeps its family and spring policy.

The motion channels are the conformance fixture, not calibrated material
dynamics, and preferences are explicit authored inputs: no platform preference
is discovered. Frame rate, scheduling budgets and fractional or changing output
scale are not measured.

### Integrated sampled command frames

The same control optionally accepts `motionChannels`, a JSON string containing
exactly the existing `bodyMix` and `depthScale` channels from
[specification 45](../../../spec/45-command-motion.md). Each requires explicit
validated dynamics and initial position/velocity. An empty string selects the
static profile. There are no material coefficients or synthesized initial states
in the component. Products must supply calibrated profiles and valid paths
before exposing animation; the arithmetic channels used by verification are
fixtures, not calibrated material definitions.

Rust resolves policy, targets, scalar trajectories, paint projection and every
actual sample's contrast/geometry guards. Qt `FrameAnimation` requests frames;
`CommandClock` supplies monotonic elapsed seconds from public `QElapsedTimer`.
Both channels share one clock origin. Retargeting samples the previous trajectory
at the new clock origin and retains unprojected position and velocity. Native
context requalification retains raw state when the explicit channel profile is
unchanged; replacing that profile uses its new explicit initial state. Scheduling
stops only after both channels settle. Reduced motion and Cast resolve immediately
through the existing reference policy, without toolkit-specific semantics.

Two complete decorative views alternate for motion. While decoding another
sample of the same state/source/context, the last complete frame remains visible
and accessible. A state, configuration or native-context replacement withdraws
old channels before preparing the current state, preserving the qualified target
across ordinary feedback. The input coordinate frame stays fixed; swapping paint
does not change capture or target membership. Sample guard/decode failures retire
paint, semantics and input together with a diagnostic. Focus and semantics describe
the current activation state and never wait for body motion to settle.

The native read-only `snapshot`, `motion` and `currentFrame` aliases expose a
coherent selected frame. `motion` is backend observation metadata containing
elapsed `time`, retained `channels` and the complete existing motion IR; it is
not a new portable IR or action authorization. `CommandSnapshotResolver` commits
these fields together before notification and clears them when the selected
QObject is destroyed. Component publication is deferred beyond the controller's
publication guard, including when a cached image decodes synchronously, so a
consumer may revoke availability from a snapshot callback without recursively
entering the controller commit. There is one native accessible Button throughout
same-state frame replacement; decorative buffers remain ignored.

The lower-level `resolveMotionSnapshot(request, producer, options, motion, render)`
accepts `{schemaVersion:"0.1.0", channels, time}` as its motion JSON. Success
returns the complete `snapshot` plus existing motion IR; `render: true` also
returns checked `paintSource`. Failure returns only `error`. Motion, rendering
options and retarget source have a 64 KiB adapter limit and preserve strict
object, duplicate, unknown, nonfinite and version rejection. `retargetMotion`
accepts `{channels, target, policy, time}` from the previous resolution and returns
both raw retained channels and previous scalar samples; it does not select
policy. Calls involving native measurement/context stay on the Qt UI thread.

Acceptance requires independently checked scalar equations and projections,
coherent actual native drawing, raw overshoot/velocity retention, both-channel
settling, immediate preference/material policy, intermediate guard failure with
no partial publication, recovery, and the existing input/AT action path during
motion. Run after preparing the compiled module:

```sh
python tools/check_qt_command_motion_runtime.py \
  --raster target/qt-runtime-build/debug/resina-command-motion-raster \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-motion-captures
python tools/check_qt_command_control_runtime.py --motion \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-moving-control-captures
python tools/check_qt_command_control_accessibility.py --motion \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
```

The equation checker compares one frozen actual sampled frame against the
public CPU motion-raster output outside the separately verified native label
bounds, using the existing pixel tolerance. It also exercises strict native
input rejection, clock start/
restart/invalidation, paired-cache reentrancy and selected-frame destruction.
The input checker retains its existing cancellation, localization, placement,
rapid-event and exact-once assertions. Optional parent XTest arguments traverse
Weston into spontaneous Wayland input; this remains injected input. The AT checker
requires an unsettled spring publication, stable native node identity across
frames, actual Press through Rust into one product effect and no stale effect.
These checks qualify the supplied fixtures and tested software host, not material
calibration, physical devices, screen-reader operation, frame-rate performance,
hardware GPU realization or released Quickshell accessibility.

Executed local evidence uses Qt 6.11.2 software rendering: the sampled equation,
publication, projection, guard/recovery and frozen readback checker passed
offscreen at DPR 1, 1.3 and 2, and on nested Wayland at DPR 1. The full sampled
input checker passed through parent XTest/Weston into spontaneous Wayland events,
including German wrapping and Arabic paragraph content. Actual AT-SPI Press and
state publication passed on X11/Xvfb and nested Wayland during unsettled motion;
the default static AT profile also passed again. Captures of successive pressed
samples were inspected. The
[hosted native checks](https://github.com/marcoallegretti/Resina-Design_System/actions/runs/38121936167)
also passed. These results apply to the standalone Qt host; GUIdo integration
and released Quickshell accessibility require their own runtime verification.
Material calibration and Shell host accessibility remain unverified.

## Integrated static Toggle control

`ResinaToggleControl.qml` realizes the existing Toggle activation, state,
accessibility, layout, part-paint and complete-snapshot contracts (specifications
50–55). One Qt-owned `ToggleController` commits the Rust activation and binary checked
state before `activated(checked)`. An application replaces selection explicitly with
`control.setChecked(boolean)`; replacement preserves a live hold and produces no
product effect. Accepted release or invocation uses that current value. The
read-only `checked` property is never assigned by QML or Qt's default Toggle
action. Consumers own persistence and product effects.

The component owns native keyboard routing and compiled primary mouse capture,
a complete decorative view and one `Accessible.Switch`. Both native Qt Toggle
and Press actions call the same live guarded invocation. Its track, thumb,
background and complete measured glyphs are decorative and ignored by AT.
Selection, feedback and actual focus are separate dimensions. The explicit
`disabledFocusable: false` profile relinquishes actual Qt focus on disabling;
`true` is rejected with the existing diagnostic rather than substituting
semantics. Static feedback has no inferred dynamics or material calibration.

Supply `configuration`, `disabledFocusable` and `samplesPerAxis`. The backend
request has these required fields (all names are case-sensitive):

- `content`: the existing native complete-label request with theme sources,
  environment, explicit font mapping, text, minimum/maximum size and padding.
- `track` and `thumb`: neutral-rest `surface` intent, opaque `appearance`,
  `checkedColorRole`, `interactionAppearance`, `foregroundRole`,
  `minimumContentContrast` and `minimumEdgeContrast`.
- `layout`: authored `trackSize`, `thumbSize` and logical `insets`.
- `labelOrigin`: explicit physical placement of the complete label slot relative
  to the track; `labelForegroundRole`, `background` and `minimumLabelContrast`.
- `targetBounds`: explicit stable physical reservation; `availableBounds`,
  `componentMinimum`, `occupiedRegions` and `minimumThumbContrast`.
- `activation`, `hovered`, `checked`, `description` and `focusable`: validated
  template observations, replaced from the component's live owner during use.

The view paints the explicitly authored uniform opaque background across that
reservation; label and track backdrop claims therefore describe actual drawing.
Rust resolves the current track first and uses its actual pigment for thumb
backdrop/edge contrast. The portable snapshot rejects occluded labels, incomplete
navigation/endpoint coverage, nonuniform thumb placement and insufficient
contrast. Every checked/availability/hover/press/focus endpoint is qualified
before input is armed; all must retain the same explicit target. No geometry,
color, contrast threshold or layout default is supplied by the component.

`ToggleSnapshotResolver.resolveToggleSnapshot(request, producer, options, render)`
returns an adapter envelope containing the existing complete snapshot members;
`render: true` additionally returns `trackSource` and `thumbSource`. This is not
a new portable wire format. Root requests retain strict object, duplicate and
unknown-field rejection with an 8 MiB source limit; rendering options have a
64 KiB limit. Measurement, context validation and publication stay on the Qt UI
thread. Publication waits for both decoded parts and complete glyph preparation.
Configuration, transform, placement, density, window, visibility or lifetime
loss withdraws stale paint/semantics/input. Ordinary feedback keeps the stable
qualified capture owner while preparing a replacement frame. Read-only
`snapshot` and `currentFrame` identify the same selected complete view.

Acceptance for this static increment is joint checked/activation coherence,
complete native drawing matching the public CPU rasterizer outside verified
label bounds, stable targeting, live selection replacement during held input,
exactly-once product effects, actual Switch publication/action, and rejection
without stale production channels. After building and packaging the adapter:

```sh
cargo build -p resina-raster --bin resina-toggle-part-raster --locked
python tools/check_qt_toggle_runtime.py --module-path target/qt-runtime-import
python tools/check_qt_toggle_control_runtime.py \
  --raster target/debug/resina-toggle-part-raster \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-toggle-captures
python tools/check_qt_toggle_control_accessibility.py \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf --action Toggle
# Repeat the strict native action check with --action Press.
```

The controller check uses shared activation cases and injected QObject events.
The complete-control check uses actual Qt font/layout/window readback, public
Toggle part-raster comparison with the existing pixel tolerance, and injected
Qt window key/pointer events. Optional parent XTest arguments use the existing
measured nested Wayland path; these remain injected, never physical input. The
AT checker requires native role 130 (Switch), binary checked/checkable state,
no pressed/mixed state, stable node identity, live action delivery into Rust and
one product effect, and no stale effect after product revocation.

### Qt Switch qualification boundary

The standard PySide6 6.11.2 build publishes `Accessible.Switch` as AT-SPI CheckBox
(role 7). It fails the strict native Toggle check. Exact upstream Qt 6.11.2
`src/gui/accessible/linux/qspiaccessiblebridge.cpp` enables Switch mapping only
when compiled with `ATSPI_ROLE_COUNT >= 132`; AT-SPI 2.56 headers have count 131
although Switch is already present. AT-SPI 2.62 headers have count 132. A local
isolated build of **unmodified** Qt 6.11.2 against the latter headers demonstrated
actual Switch publication and Toggle delivery over nested Wayland, then the
integrated Rust control passed the strict action check. No private Qt bootstrap,
role substitution, patched Resina semantics or distributed toolkit was added.

For reproduction use the exact upstream `qtbase-everywhere-src-6.11.2` source,
real AT-SPI 2.62 development headers and dependencies, verify configure enables
`accessibility_atspi_bridge`, and build `Gui` and `DBus`. Use a matching 6.11.2
host and verify the libraries actually loaded: PySide's loader uses its bundled
Qt despite `LD_LIBRARY_PATH` alone. The isolated experiment explicitly preloaded
its matching Core/DBus/Gui libraries and used QtQuick/PySide 6.11.2; the strict
checker, rather than library presence, determines native success. This is
isolated toolkit evidence, not support claimed for standard binary SDKs.

Quickshell 0.3.1's separate application-bootstrap accessibility defect remains
on the approved upstream repair route. Full Shell accessibility, calibrated
Toggle travel/interaction motion, physical/touch/pen input, screen-reader
operation, GPU realization and production performance are not qualified here.
The next integrated dependency is Toggle travel and independent track/thumb
interaction motion over specifications 56–57; Slider follows its prerequisites.

Executed local checks for this increment used explicit DejaVu Sans and Qt 6.11.2
software rendering. Complete Toggle light/dark and RTL/LTR window captures
matched the public part rasterizer at DPR 1 and 2; captures were inspected.
Held Space/pointer selection replacement, Enter-down activation, focus/transform/
visibility cancellation, reentrant snapshot revocation and guard recovery were
checked in the native component. Parent XTest traversed Weston into spontaneous
Wayland pointer/key events and exactly-once effects. The isolated Qt build passed
actual AT-SPI Toggle and Press separately; the standard PySide build failed the
strict Switch-role check. Root workspace tests, native Qt tests/clippy, public
Toggle activation/part-paint checks, Python tests and contributor-toolkit checks
passed. The shared native publication/pointer changes also passed Command
snapshot, sampled control and motion/readback regressions. CI is configured for
controller and complete-control checks; no hosted run or standard-SDK native
Switch success is claimed.

## Integrated sampled Toggle frames

The same `ResinaToggleControl` accepts an optional explicit `motionChannels`
string. Empty selects the existing static profile. Nonempty requires exactly
`{track, thumb, travel}`: `track` and `thumb` each contain required `bodyMix` and
`depthScale` channels; `travel` is one channel. Every channel requires its authored
`dynamics` and complete raw `initial` state. No coefficient, state, velocity or
material preset has a default. These descriptors belong to the Qt adapter;
existing portable sampled part IR and typed travel operations retain ownership
of semantics, policy, equations, projection, geometry and contrast.

Each actual frame resolves the current sampled track first, then the sampled
thumb against that track's actual pigment, assembles the complete snapshot and
validates the sampled travel placement against it. Checked roles, semantics and
actual focus become current immediately. Travel changes only placement; part
motion changes only the existing interaction response. Native drawing uses the
checked travel bounds rather than a QML interpolation. Translucent Frost is
rejected by the opaque profile; the runtime never changes transparency
preferences or capabilities to force success. A separately authored scene that
requests reduced transparency uses the existing opaque Frost fallback.

The public monotonic `CommandClock` is shared native clock machinery, used here
for all five channels with one elapsed-time origin. On any target/policy change,
the previous trajectories are sampled at the exact new clock origin and retain
raw position and velocity. Projection never feeds back into state. The same
explicit profile retains current raw state across native context requalification;
a new profile starts from its new explicit initial state. Physical continuity
requires unchanged endpoint allocation; changed layout/direction/density is
reconciled by complete requalification, not a claim of unchanged physical motion.

Two complete decorative views buffer same-state samples while native images
decode. State, source or context replacements withdraw old channels before
preparing current content. The fixed native capture owner and stable target stay
independent of decorative frame replacement. `FrameAnimation` requests another
frame until **every** track/ thumb response channel and travel channel settles;
bounded projection does not stop scheduling. An actual sampled guard failure
retires paint, semantics and native targeting together. The read-only `snapshot`,
`motion` and `currentFrame` aliases refer to the same selected complete frame.

`ToggleSnapshotResolver.resolveMotionSnapshot(request, producer, options,
motion, render)` uses `{channels, time}` with the explicit profile above.
Success returns `snapshot` and `motion`; rendering additionally returns complete
`trackSource` and `thumbSource`. Motion observation contains complete existing
`track` and `thumb` sampled part IR and typed travel data (`policy`, binary
`target`, full `trajectory`, `projection`, checked `thumbBounds`). It preserves
the snapshot's current selected endpoint rather than rewriting logical layout.
The component's native `motion` alias pairs that result with elapsed `time` and
retained `channels`; this is backend observation, not new portable IR.

`validateMotionChannels` validates the explicit descriptor. `retargetMotion`
accepts `{channels, targets, time}`; `targets.track` and `.thumb` contain existing
`target` response and `policy`, and `.travel` contains binary endpoint `target`
and `policy` from the previous resolution. It samples those previous trajectories
through the reference and returns raw retained channels. Motion/retarget sources
have a 64 KiB limit and reject unknown/duplicate/positional/nonfinite input and
invalid dynamics before policy selection, including immediate policies. Native
measurement/context/publication remain on the Qt UI thread.

Acceptance adds independent trajectory equations and exact projection/placement
from returned raw state, complete native pixels matching the public sampled part
rasterizer, raw retargeting/overshoot, all-channel settling, immediate policies,
intermediate guard rejection, scaled/wrapped labels and unchanged native input
and AT behavior during actual unsettled motion. Run:

```sh
cargo build -p resina-raster --bin resina-toggle-part-motion-raster --locked
python tools/check_qt_toggle_motion_runtime.py \
  --raster target/debug/resina-toggle-part-motion-raster \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-toggle-motion-captures
python tools/check_qt_toggle_control_runtime.py --motion \
  --raster target/debug/resina-toggle-part-motion-raster \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
  --output-dir target/qt-toggle-moving-input
python tools/check_qt_toggle_control_accessibility.py --motion \
  --module-path target/qt-runtime-import \
  --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf --action Toggle
# Repeat --action Press in a host with the qualified native Switch mapping.
```

The pixel oracle freezes the actual native frame driver for one selected-frame
readback and compares the public CPU raster result outside separately prepared
complete label bounds, preserving the existing pixel tolerance. Scalar equations
use the existing conformance tolerance; projection and physical placement are
checked exactly from the verified returned raw state. Fixtures use explicit
arithmetic dynamics, not calibrated production defaults. Nested Wayland parent
XTest remains injected input; actual AT-SPI invocation is a separate executed
boundary. Native Switch qualification still requires the toolkit build described
above, and released Quickshell still requires its approved upstream host repair.
Physical devices, screen-reader operation, hardware GPU realization, production
frame-rate performance and material/style qualification are not certified by
these checks.

Executed on Linux with Qt 6.11.2 software rendering and explicitly registered
DejaVu Sans: the complete moving-frame matrix passed at actual device scales
1, 1.25, 1.3 and 2. It covered Light/Dark LTR/RTL, authored Cast/Frost/Elastomer,
explicit opaque Frost fallback and rejection of translucent Frost, doubled
wrapped German/Arabic paragraph labels, actual focused press feedback, raw
overshoot/retargeting, independent settling, reduced motion, intermediate
contrast rejection and malformed-source withdrawal/recovery. Image review
confirmed complete paragraphs, unclipped focus and native track/thumb edges;
these arithmetic scenes are structural evidence, not accepted material styling.
The fractional-scale image mapping repair also passed native Command paint at
those four scales and Command motion/readback plus static Toggle regression.
The backdrop oracle's half-pixel boundaries were independently verified using
native `QPainter.fillRect(QRectF)`; pixel thresholds were unchanged.

Parent XTest traversed nested Weston into spontaneous Wayland key/pointer
delivery on the final build, preserving exactly-once effects and revocation
during sampled motion. The isolated unmodified Qt build described above passed
actual AT-SPI Toggle and Press separately during unsettled motion with stable
Switch identity. Root locked workspace tests, PNG-disabled raster tests,
native Qt tests/clippy, QML/raster clippy, formatting, Python and contributor
toolkit checks passed. New checker usage and missing-plugin failures returned
their expected nonzero statuses. CI is configured for the sampled matrix;
no hosted run or broader platform qualification is claimed.

## Native Slider capability boundary

Slider already has portable value, edit, pointer, key, presentation, snapshot and
track-segment contracts. Native Qt realization must preserve the complete
[intrinsic semantics](../../../spec/60-slider-accessibility.md), including
explicit orientation and any supplied localized value text. No native Slider
component is currently supplied by this adapter.

The toolkit-only probe below uses a real Qt Quick Item and isolated native
AT-SPI delivery. It deliberately has no Resina controller; its observations
are capability evidence, never component qualification. With Qt 6.11.2 on nested
Weston, both authored orientations published Slider role 51, numeric bounds
0..1, current value 0.4 and increment 0.05. Neither published HORIZONTAL or
VERTICAL state. `Value.Text` failed and the host reported unimplemented `GetText`,
despite an explicit `valueText: "40 percent"`. Numeric Value queries succeeded;
the error must not be interpreted as absence of the whole numeric interface.

A real native write changed the value to 0.65 and the fixture then made itself
read-only. A second real write of 2 still changed the value. This is toolkit
default property delivery, not a failure of an implemented Resina Slider.
Resina must route every write through current reference permission, domain and
edit adoption before coherent publication/product notification. A read-only
semantic flag alone does not enforce this path.

```sh
QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_QUICK_BACKEND=software \
  dbus-run-session -- python tools/run_qt_slider_capability_probe.py --orientation horizontal
# Repeat --orientation vertical with an actual Linux display.
```

The exact [Qt Quick source](https://github.com/qt/qtdeclarative/blob/v6.11.2/src/quick/accessible/qaccessiblequickitem.cpp)
reads conventional numeric properties and writes `value` directly. Its public
attribute mapping supplies locale, not orientation, and its Slider text mapping
does not supply localized value text. The
[Qt AT-SPI bridge](https://github.com/qt/qtbase/blob/v6.11.2/src/gui/accessible/linux/atspiadaptor.cpp)
can map public `QAccessibleAttributesInterface` orientation to native state but
omits `Text` from its Value interface. The
[AT-SPI contract](https://github.com/GNOME/at-spi2-core/blob/main/xml/Value.xml)
defines `Value.Text` as the human-readable value alternative.

The Resina adapter now owns a public accessibility factory restricted to
`SliderAccessibilityItem`. It uses no private Qt accessibility API, toolkit
patch, custom D-Bus service or upstream contribution. The portable contracts
remain unchanged. The Qt AT-SPI value-text limitation is documented here and
retained in the native evidence; it is not replaced by a label, description or
custom attribute.

### Continuous Slider accessibility bridge

[`ResinaSliderAccessibility.qml`](../resina-qml/qml/ResinaSliderAccessibility.qml)
is a native semantic leaf over the compiled `SliderAccessibilityItem`. It is a
bridge for a composing presentation owner, not a complete painted Slider.
It supplies the public [value](https://doc.qt.io/qt-6/qaccessiblevalueinterface.html),
[action](https://doc.qt.io/qt-6/qaccessibleactioninterface.html) and
[attributes](https://doc.qt.io/qt-6/qaccessibleattributesinterface.html) interfaces.
Qt supplies the actual native accessibility bus and object cache. Decoration must be ignored
and there must be only one semantic Slider item for the control.

The composing owner calls `publish(JSON.stringify(publication))` with:

```js
{
  accessibility: qualifiedSnapshot.accessibility, // complete Core SliderAccessibilityIR
  revision: presentation.revision,
  valuePolicy: {kind: "continuous"},
  stepSize: authoredPositiveStep
}
```

Publication is read-only. The bridge checks the complete semantic shape,
version, domain and numeric literal precision, action availability, explicit
orientation, optional nonblank description/value text, focusability and actual
native focus. It rejects unknown/duplicate members, incoherent semantics,
non-object records and requests exceeding 64 KiB. Invalid publication withdraws
all semantics. Native focus changes also withdraw the publication, so the
owner must requalify actual focus before accepting further actions. This profile
accepts continuous values only; it explicitly rejects stops rather than silently applying continuous adjustment rules.
`stepSize` is an authored positive finite amount, never a toolkit default.
It maps the native minimum increment and Increase/Decrease amount; it does not
create a quantization grid or restrict continuous SetValue to step multiples.

The owner sets `sourceAvailable` only after qualifying and publishing the
complete current native frame. It must withdraw readiness before replacement,
failed preparation or revoked native context. The QML facade removes the leaf
from native discovery while readiness is withdrawn; cached native actions still
run the same live guards. This bridge also checks actual
window visibility, item visibility, enabled state and positive native extent
on every action. Its metadata checks cannot establish paint, layout, contrast,
text measurement or native frame readiness; those belong to the composing
complete-snapshot owner. No painted Slider, pointer/keyboard edit lifecycle or
product effect is supplied by this bridge.

Actual AT-SPI SetCurrentValue, Increase and Decrease call Rust's existing
[Slider adjustment](../../../spec/59-slider-adjustment.md) resolver against
current published value and live permissions. Native setters never write a Qt
`value` property. Accepted proposals emit `adjustmentRequested` with
`{candidate, candidateBase, revision}`; rejected, read-only, disabled or
unavailable invocations emit no proposal. Keyboard focus is not required.
Invalid out-of-domain values are rejected rather than clamped. Synchronous
callback reentry cannot deliver a second proposal.

The composing owner must adopt each candidate through the existing
[presentation commit](../../../spec/69-slider-presentation.md) contract against
its current visible value, revision and permissions, including cancellation of
any open edit. The bridge does not commit a value or make this adoption decision.
The owner must publish/requalify the resulting complete value and localized text
before any consumer-owned changed-value effect. An accepted no-op is still a
proposal, so the owner can close an edit; it must produce no changed-value
product effect. A delayed proposal is never permission to bypass live adoption.

The documented public [factory/cache API](https://doc.qt.io/qt-6/qaccessible.html#installFactory)
is used with [QAccessibleObject](https://doc.qt.io/qt-6/qaccessibleobject.html) for
the non-widget Qt Quick item. The public factory installs once per live
QCoreApplication, on the UI thread, and removes itself when its application-owned registration object is destroyed.
It recognizes the concrete Resina class only. Qt owns and invalidates each
cached `QAccessibleObject` with the item; the factory retains no raw item pointer.
The plugin must remain loaded until application teardown. This matches Qt's
[default PreventUnloadHint](https://github.com/qt/qtbase/blob/v6.11.2/src/corelib/plugin/qpluginloader.cpp)
for native plugins. Manual unloading while factories or native interfaces live
is unsupported. This mechanism does not repair Quickshell's destroyed-initial-
application AT-SPI bootstrap limitation documented above.

The native adapter checker uses an isolated real AT-SPI bus and a standalone Qt
Quick host. Its arithmetic continuous-value fixture publishes complete semantic
value/text before counting a fixture effect; it deliberately has no Resina
paint or pointer edit, so it cannot qualify a complete component. It verifies
both axes, exact native binary64 values, SetValue/Increase/Decrease without
focus, accepted no-op versus changed effects, out-of-domain rejection, live
read-only/disabled/withdrawn source guards, invalid publication withdrawal,
stable identity across publications, destruction/stale actions/recreation, and
preservation of an unrelated default Qt Button factory.

```sh
python tools/check_qt_slider_accessibility_adapter.py --module-path target/qt-runtime-import --orientation horizontal
python tools/check_qt_slider_accessibility_adapter.py --module-path target/qt-runtime-import --orientation vertical
# Requires an actual Linux display; the checker isolates its own AT-SPI bus.
# Fail closed when qualifying complete native intrinsic semantics:
python tools/check_qt_slider_accessibility_adapter.py --module-path target/qt-runtime-import --require-value-text
```

Executed locally with unmodified PySide6 Qt 6.11.2 on nested Weston, both
orientations passed the adapter criterion. The same adapter checks also passed
on local Xvfb/X11. Real AT-SPI delivered the adjustments
through Rust and observed coherent republished numeric values. `Value.Text`
remained unsupported and `completeNativeIntrinsicSemantics` was `false`;
`--require-value-text` therefore fails. The adapter exposes supplied value text
at the public QAccessible Value text boundary, but that is not proof of native
AT-SPI publication. Full native intrinsic accessibility, complete Slider paint,
edit adoption/input integration, released Quickshell, physical input and screen
reader operation remain unqualified. CI runs the same adapter criteria on Xvfb;
configured CI checks are not evidence of an executed hosted run.

## Compiled destructive-observer regressions

`python tools/check_qt_controller_lifetime.py --module-path <prepared-module>`
checks Command notification destruction and the native pointer caller. Add
`--controller ToggleController` for Toggle, including external checked replacement.
Each scenario runs in a separate process and actually processes Qt DeferredDelete.
The checks require no signal after controller destruction, no surviving hold
after pointer removal, and no cancellation of a replacement gesture. They
preserve the shared activation/reentrancy contracts. These synthetic operations
and injected Qt pointer events are lifetime evidence, not physical-device or
AT-SPI evidence.
