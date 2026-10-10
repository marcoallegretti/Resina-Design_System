# Qt command runtime (0.1.0)

This separately locked native workspace implements the Resina-owned
Rust/QML runtime boundary. CXX-Qt 0.8.1 exposes Rust-owned command activation
through a real Qt `QObject` in the `Resina.Runtime` QML plugin. The portable
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
`ActivationResult` JSON. Rust commits next state and updates the property before
`publicationChanged`, capture effects or product intents. Every accepted event
publishes once, including unchanged state. Recursive dispatch during publication
notification fails explicitly. Once publication completes, product and capture
callbacks can synchronously dispatch another event.

`captureRequested(id, acquire)` supplies the exact capture effect after commit.
The native gesture owner must maintain continued pointer delivery, cancel if
capture cannot be maintained, and route capture loss, deactivation and removal
through the lifecycle. `activated()` delivers one product intent after state and
capture publication. Execute effects from these signals once; retained
`publication.activate` and `publication.capture` do not authorize replay.

Supply actual focus observations and current availability through their events.
Mutation must stay on the QObject's owning thread. Use deferred Qt destruction
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
and checked pressed body/glyph drawing. The Rust bridge exercises all shared
exact membership vectors. Qt fixtures independently check mapped membership,
actual grab state per delivered device, outside/re-entry/release, focus
independence, competing keyboard/device gestures, modified termination, native
ancestor clipping, mapping invalidation, hide/disable, product revocation,
semantic supersession, capture loss and actual armed QObject destruction.
Direct `QMouseEvent` and second-device fixtures are injected Qt evidence. The
nested Weston 14.0.2/Pixman run additionally observed spontaneous Qt events
through `wl_pointer`, including motion/release outside the client, invocation
and capture-loss cancellation. Parent XTest injection establishes that tested
compositor/toolkit path, not physical mouse, touch, pen or device-hotplug
qualification. Native hover, motion and complete accessibility publication remain
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
nonidentity basis transforms are rejected. Keep ancestor clipping/occlusion and
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
withdraws visible paint before rendering, and valid preparation recovers.
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
runner validates both Quickshell 0.3.1 and its build/runtime Qt 6.11.2 versions,
adds the packaged module import root, and executes Quickshell directly. It does
not instantiate the Python lifecycle or a second Qt engine. For example:

```sh
RESINA_QUICKSHELL_HOST=/absolute/path/to/quickshell \
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" \
python tools/check_qt_native_availability.py --report-only
```

Reports identify `host: quickshell-0.3.1-qt-6.11.2` separately from the binding.
Standalone reports identify `host: qt-quick-6.11.2`. Host versions do not certify
a distribution's build or patches. Missing native nodes remain execution
failures, including with `--report-only`.

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

The accepted integration route is an upstream host/toolkit repair, while this
adapter continues in the standalone Qt host. The host/toolkit must preserve or
restore its own native publication before Quickshell controls can be qualified.
Resina does not own a process-wide private bootstrap as a semantic fallback.

This verifies a runtime dependency, not a complete Command/Button, Toggle or
Slider. The [native label pair](../resina-qml/README.md#native-label-measurement)
separately verifies fractional sizing and authored tracking through native
measurement and drawing. Released Quickshell native publication,
physical-device/screen-reader operation, coherent complete component
input/accessibility publication and motion remain unverified. The native runtime
CI job is defined; a hosted run remains unverified.
The decorative view now prepares body and complete glyph drawing together. The
input owner connects actual focus/key routing and native mouse capture to current
Rust activation and stable target mapping. The next dependency is coherent
complete control publication, including native hover, guarded motion and AT
publication.
