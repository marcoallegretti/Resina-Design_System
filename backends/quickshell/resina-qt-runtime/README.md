# Qt command runtime (0.0.1)

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
gesture. Pointer activation does not request or require keyboard focus or an
active window: Qt's Wayland backend
[activates a window](https://github.com/qt/qtbase/blob/v6.11.2/src/plugins/platforms/wayland/qwaylanddisplay.cpp#L980)
only after a later display roundtrip, so the press that gives a surface keyboard
focus arrives while its window is still inactive. Rust
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
actual grab state per delivered device, outside/re-entry/release, focus and
window-activation independence, competing keyboard/device gestures, modified termination, native
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
current body, reserves the target from the neutral body's deepest silhouette at
the projected depth bound, then
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
`nativeDeviceScale` is the hosting window's `devicePixelRatio`, a property Qt
Quick provides since 6.11, and 0 without a window. It is not the attached
`Screen.devicePixelRatio`, for two reasons. Under `wp_fractional_scale_v1` the
screen reports the output's integer scale while the window renders at the
fractional one, and the attached screen value does not notify when the ratio of
the same screen changes, so a live change between integer scales would not
reach the view either.
Do not force a different front width/height or add scale/rotation. Native
translation is checked within the existing 1/1024 logical-pixel precision budget;
the compiled `CommandSnapshotResolver.matchesNativeContext(item, x, y,
deviceScale)` checks the complete native scene transform. It requires a live
UI-thread `QQuickItem` in an exposed native window, finite placement, the
window's effective device pixel ratio and an affine translation with an exact
identity linear part. Missing/non-item/foreign-thread contexts and invalid numeric inputs return
false. It retains no native object. The same translation predicate guards native
pointer targeting. Sampling the origin and unit basis alone is insufficient:
a projective matrix can preserve all three samples while distorting the body
elsewhere. Scale, rotation, shear and projective transforms are rejected before
publication and on the GUI frame check. Keep ancestor clipping/occlusion and
actual surrounding/backdrop colors consistent with the supplied paint context.

A window may report a fallback density: Qt's Wayland platform reports the
screen's integer scale until the compositor's preferred fractional scale
arrives, and Quickshell creates a new window each time a hidden surface is
shown again. `presentsNativeWindow(item)` on the same resolvers reports whether
the item's window is exposed, and the context check above rejects an unexposed
window. The Command and Toggle controls therefore prepare nothing, and report no
preparation error, until their view's window is exposed; they prepare on the
window's next `afterAnimating`. The preferred scale is a separate compositor
event, so exposure does not guarantee it has arrived. In headless Sway 1.12 at
output scales 1.25 and 1.5, a layer-shell `PanelWindow` received its scale
before exposure, so no publication in it is made at the fallback density. An
xdg toplevel `FloatingWindow` was exposed and presented its first frame at the
fallback ratio 2 and received its scale only afterwards, so a Command or Toggle
in such a window can publish once at the fallback density; the change of the
window's ratio then withdraws readiness, as a live output-scale change does, and
the control prepares again at the new density. The compositor-delivered
checks (Command and Toggle lifecycle, Command accessibility, motion and motion
frame budget, and the clock shell) host their controls only in layer-shell
`PanelWindow`s. The authored-update, focus, label, label-render, paint and
native-availability probes use a `FloatingWindow`. CI runs the update checks on
Qt's offscreen platform and the native-availability check on xcb under Xvfb;
the native-availability check with `WAYLAND_DISPLAY` set, and the Command and
Toggle update, label, label-render and paint checks with `--platform wayland`,
present that window as an xdg toplevel. No
check asserts the fallback-density first publication described above or the
preparation that follows it.

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

## Live native surface view

[`ResinaSurfaceView.qml`](../resina-qml/qml/ResinaSurfaceView.qml) realizes one
complete [surface paint request](../../../spec/39-surface-paint-ir.md) for a
structural or transient surface that is not a control. Set `request` to the
request JSON and `samplesPerAxis` to the raster sampling density. The view
resolves the request with the reference resolver through the compiled
`SurfacePaintResolver.resolveSurfaceView(request, options)` at its actual scene
origin and window pixel density, and realizes the body and optional focus ring
with the shared CPU raster as one native image. Like the Command view it takes
the hosting window's `devicePixelRatio` as `nativeDeviceScale` and waits, without
an error, until that window is exposed.

`publication` carries the complete surface paint IR only once the native image
is ready; `paintReady` and default visibility become true with it. The view
exposes the IR's guarded `foreground` color and `contentBounds`, the bounding
rectangle of the IR content region in the view's local logical coordinates. The
readability guard applies inside the content contour; for rounded shapes the
rectangle's corners lie outside it, so consumer content must stay within the
contour published in `publication`. Its implicit size is the IR front and is
kept while withdrawn so layouts stay stable; `foreground` and `contentBounds`
are cleared whenever publication is withdrawn. Invalid requests, unsupported
states and missing surrounding colour for focused surfaces withdraw publication
and report `paintError`. Clearing `request` withdraws publication without error.

`SurfacePaintResolver.resolveOpaqueColor(themeRequest, role)` returns the opaque
sRGB fallback that [theme resolution](../../../spec/13-opaque-color-fallback.md)
assigns to one semantic colour role, as `{"role", "color"}` or a diagnostic.
The colour itself guarantees no contrast. The surface guards check the
`adjacentColor` and, when focused, the `surroundingColor` supplied in each
request. A consumer that paints its backdrop in this colour, resolved from the
surface request's own `theme`, must declare the same value as those colours for
the guards to describe what is actually painted.

The view prepares itself. Changes to the request, sampling density, own
position, scale, rotation, parent, window or pixel density withdraw the current
publication and prepare again. A GUI-thread `afterAnimating` check withdraws
and re-prepares when an ancestor moves the scene origin, and a preparation whose
origin moved before its image was ready is discarded and repeated. Work queued
before destruction or `Loader` unloading does not run. A surface whose
ancestor keeps moving, such as one sliding in, stays withdrawn until its origin
settles; animate it only after publication or keep its origin fixed. Scale, rotation, shear
and projective ancestors fail the shared native context guard used by the
Command view. The view stays withdrawn with a diagnostic, and prepares again once
the context is translation-only. It is decorative, has no input target or native
accessible node, and does not own window, layer or surrounding colour policy.

[`check_qt_surface_view_runtime.py`](../../../tools/check_qt_surface_view_runtime.py)
renders all 16 public Light/Dark Cast/Frost/Elastomer/Gel rest and focused paint
scenes. For each scene, the publication must equal the `resina-surface-paint`
output, the exposed foreground, content region and size must match that IR, and
the native capture must match the `resina-paint-raster` image within one byte per
channel. Replacement must withdraw readiness before publishing. The checker also
covers:
- invalid and focused-without-surrounding requests;
- clearing the request;
- own and ancestor translation, with re-captures at the new origin, including an
  ancestor move while a preparation is pending;
- ancestor distortion, withdrawn and recovered;
- every role's opaque colour for both public theme requests, compared with
  `resina-theme-resolve`, and rejection of an invalid request and role;
- a request supplied at creation;
- destruction, and `Loader` unloading, while a queued preparation or an
  incomplete publication is pending, observed past the preparation timeout;
- no Qt or QML warning anywhere in the run;
- the context guard's precision, type, density and foreign-thread rejection.

CI runs it with Qt 6.11.2 software rendering offscreen at DPR 1, 1.25, 2 and 1.3:

```sh
cargo build -p resina-raster --bin resina-paint-raster --locked
cargo build -p resina-resolver --bin resina-theme-resolve --bin resina-surface-paint --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
QT_SCALE_FACTOR=1.25 python tools/check_qt_surface_view_runtime.py \
  --module-path target/qt-runtime-import --raster target/debug/resina-paint-raster \
  --resolver target/debug/resina-surface-paint --theme-resolver target/debug/resina-theme-resolve \
  --scenes target/material-scenes.json \
  --output-dir target/qt-surface-view-1.25
```

The [clock shell](../../../lab/quickshell/clock-shell/README.md) hosts the view
in released Quickshell 0.3.2 and checks its publications against the reference
resolver, and composited pixels at points where a flat body or backdrop colour
is expected, at output scales 1, 1.25, 1.5 and 2,
and after live output-scale changes between them. At revision 4cb60fe it ran at
scales 1 and 2 and with live changes between 1.25 and 2; scale 1.5 and the
other live changes passed at earlier revisions. Hardware renderers and
enhanced Frost or Gel treatments beyond their opaque representation are not
covered.

## Platform appearance preferences

An environment producer must supply every environment field (spec/18), including
the accessibility preferences. `PlatformAppearance` reads the platform's
preferences from the
[XDG Desktop Portal Settings](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html)
`org.freedesktop.appearance` namespace on the session bus. It reads them when
created and again whenever the portal service gains an owner on the bus, and
follows the portal's `SettingChanged` signal after a successful read. When the
portal leaves the bus, it withdraws every preference and reports an error.

- `reducedMotion` and `highContrast` are `true` for the `uint32` portal value 1
  (`reduced-motion`, `contrast`) and `false` for any other value, which the
  portal defines as no preference.
- `colorScheme` is `"dark"`, `"light"` or `"none"`.
- A key the portal does not provide stays `null`, so a consumer can tell an
  absent preference from one that is not requested.
- `ready` becomes true once the first read completes. While the session bus or
  the portal is unavailable, `error` describes the failure and every preference
  is `null`.

Qt 6.11.2 exposes the colour scheme and contrast preference but no reduced-motion
preference, and Quickshell 0.3.2 has no portal settings API. The element neither
substitutes defaults nor sets Resina environment fields itself; the consumer
decides how they enter its environment.

[`check_qt_platform_appearance.py`](../../../tools/check_qt_platform_appearance.py)
starts its own session bus and runs the real `xdg-desktop-portal` frontend with
the GNOME backend and isolated GSettings. The session's `portals.conf` selects the
GNOME backend for Settings; in this isolated session, xdg-desktop-portal 1.18.4
with xdg-desktop-portal-gnome 46.2 exported no Settings portal without it. It
requires:
- the element to equal an independent `gdbus` read of the portal for every key;
- each provided key to reach a real GSettings change and then its reset, each
  observed first in the portal;
- unprovided keys to stay `null`;
- a missing session bus, and a bus without a portal, to report an error with no
  preference;
- preferences to be read once the portal appears on a bus where the element
  started first, withdrawn with an error when the portal leaves, and read again
  when it returns.

`--require` fails when the backend does not provide a listed key.
xdg-desktop-portal 1.22.1 with xdg-desktop-portal-gnome 50.0 provides and passes
all three keys. xdg-desktop-portal-gnome 46 to 49 do not provide
`reduced-motion`; CI runs on Ubuntu 24.04's GNOME 46 backend and requires
`contrast` and `color-scheme`. The checker needs a Wayland session for the GNOME
backend.

In CI, the platform `reduced-motion` value is exercised by the
[clock shell](../../../lab/quickshell/clock-shell/README.md#verification) check
at output scale 1. That check runs this element through the real
xdg-desktop-portal 1.18.4 frontend with the test Settings backend
[`portal_settings_backend.py`](../../../tools/portal_settings_backend.py) in
place of the GNOME backend, and requires all three keys. The value and its
changes therefore reach the element through the standard
`org.freedesktop.portal.Settings` interface and `SettingChanged`, with no
separate code path in the element. A desktop backend providing
`reduced-motion` is verified only locally, with xdg-desktop-portal-gnome 50.0.

## Build and verification

Install the tested Qt 6.11.2 development SDK with its Core/Qml tools and a C++17
compiler. Keep the host's Qt ABI aligned. Native label checks take
`--font-dir`, a directory holding the Inter 4.0 static Regular, Medium and
SemiBold masters pinned by SHA-256 in
[`tools/conformance_font.json`](../../../tools/conformance_font.json). Ubuntu
24.04's `fonts-inter` installs these files, with the other Inter 4.0 static
faces, in `/usr/share/fonts/opentype/inter`; elsewhere,
`python tools/conformance_font.py fetch --output DIR` downloads the pinned
release archive and extracts them. Each check registers all three, so authored
weights 400, 500 and 600 resolve to real masters, and
`check_qt_command_label_runtime.py` prints the master that resolved every
Command label measurement. That check first lists the Inter faces fontconfig
makes visible to the process and fails unless every upright, normal-width one at
weight 400, 500 or 600 is byte-identical to that weight's pinned master, so the
style Qt reports identifies the file it used. Production `measureJson` returns
only the extent: it drops the matched face, so the Command path cannot detect a
substituted weight. Measurements are compared within one run only.
From the repository root on Linux:

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
python tools/check_qt_command_label_runtime.py --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter
python tools/check_qt_command_snapshot_runtime.py --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter
python tools/check_qt_command_keyboard_runtime.py --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter
python tools/check_qt_command_pointer_runtime.py --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter
QT_SCALE_FACTOR=1.25 python tools/check_qt_command_paint_runtime.py \
  --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter \
  --raster target/debug/resina-command-raster --output-dir target/qt-command-paint-1.25
QT_SCALE_FACTOR=1.25 python tools/check_qt_command_view_runtime.py \
  --module-path target/qt-runtime-import --font-dir /usr/share/fonts/opentype/inter \
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
  --font-dir /usr/share/fonts/opentype/inter
```

For compositor-delivered input, set `resina_weston_window` to the actual X11 output
window ID reported in that Weston log. Parent injection also requires the
X11/XTest runtime libraries:

```sh
python tools/check_qt_command_keyboard_runtime.py --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
  --parent-x11-display :0 --parent-x11-window "$resina_weston_window"
python tools/check_qt_command_pointer_runtime.py --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
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
3.3.0, together with the pinned Python host, and builds unmodified Quickshell
0.3.2 against that SDK. It builds, tests and lints this separate workspace and
executes the compiled Command and Toggle controller and runtime checks and the
Command content, snapshot, keyboard, pointer and control checks. Surface view,
Command paint, control, motion and view, and Toggle control and motion checks
run at DPR 1, 1.25, 2 and 1.3; capture artifacts remain available on failure.
This job's AT-SPI step runs under Xvfb: it measures real invocation and
availability using `--known-deviation`, and runs the static and sampled Command
control accessibility checks and the horizontal and vertical Slider adapter.
The released Quickshell host then repeats the availability measurement with
`--known-deviation` and runs the authored-update checker. In headless Sway it runs
the lifecycle, accessibility, motion, platform appearance and
[clock shell](../../../lab/quickshell/clock-shell/README.md) checkers.
In the availability measurement, malformed/missing native observations and incorrect action
delivery still fail; the known disabled-focusability discrepancy is recorded as
`conformant: false` rather than certified as passing accessibility, and any
mismatch list other than that single entry fails the step. The strict command
above, which fails on that entry, remains the gate for discoverable disabled
focus; the native accessibility claim covers the excluded-disabled policy only. A local reproduction does not
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
measurement and drawing. Released-host Command qualification is described in
[authored updates](#released-quickshell-authored-updates),
[lifecycle](#compositor-delivered-command-lifecycle),
[accessibility](#released-quickshell-command-accessibility) and
[motion](#released-quickshell-command-motion). Fractional output scales are
verified on one headless Sway output by the motion and clock shell checks.
Native input in these checks is compositor-delivered virtual pointer and
keyboard input on one seat. Physical devices, touch, stylus, multiple seats and
multiple outputs remain unverified. Command screen-reader presentation, as the
speech and braille text Orca generates, is verified with Orca 46.1 in CI and
Orca 50.3 locally
([#165](https://github.com/marcoallegretti/Resina-Design_System/issues/165)).
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
requires native paint, target and armed input readiness. The decorative view
becomes visible only with that complete readiness, including first publication
and requalification. The accessible node is first exposed with its complete
name and description before `ready` becomes true, so focus requested from
`onReadyChanged` reaches assistive technology: Qt sends no focus event for an
ignored item and none when it stops being ignored. While the control stays
visible in a visible native window, feedback, availability, authored-property,
placement and density updates keep that node exposed with its previous complete
semantics until the next complete semantics replace them in place. An
unavailable control is never exposed as focusable, even before its retained
semantics are replaced. Until a requalified publication arms input again, the
retained node refuses `Press`. Hiding the control or its window and failure
make the node ignored. Unmounting the control or destroying its surface deletes
the object, and its node goes with it. Snapshot existence alone is not action
authorization. `preparationError` reports failure.
`invoke()` routes the semantic action only through an armed available lifecycle;
its Boolean describes accepted delivery, not a product outcome. `activated` is
emitted from the committed Rust activation transition. `reconfigure()` cancels
held input and requalifies the current source after externally repairing native
context. Authored-property, visibility, native-window, placement and density
changes automatically retire the publication and schedule requalification.
Unsupported contexts stay diagnostically withdrawn until repaired.

A feedback update withdraws old paint and the snapshot while preparing the latest
complete snapshot; the accessible node keeps its previous complete semantics.
It retains its already qualified native target and armed input owner: image
decoding is not authorization for a new gesture. This allows rapid hover/down/up
to reach the lifecycle without waiting for an event-loop turn between events.
Multiple feedback events can coalesce before a frame; no stale snapshot is
retained as current. Existing pointer/key releases remain
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
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-control-captures
python tools/check_qt_command_control_accessibility.py \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter
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
  --font-dir /usr/share/fonts/opentype/inter \
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
generation. The run fails when the host log reports a QML type, reference or
invalid-context error. Final natural and fitted native measurements reach the
independent rational layout checker. These remeasurements verify final
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

### Released Quickshell Toggle authored updates

The [Toggle probe](../../../conformance/quickshell/toggle-updates-probe.qml)
mounts `ResinaToggleControl`, replaces its complete authored configuration and
sets the checked owner through the Rust controller. Run it against the same host
and module pair:

```sh
python tools/check_quickshell_toggle_updates.py \
  --host target/quickshell-build/src/quickshell \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/quickshell-toggle-updates --scales 1 1.25 2
```

The 39 scenarios cover off and on Toggles in Light/Dark LTR/RTL, expanded German
and mixed Arabic/Latin labels, text scales 1.5 and 2, focused off and on Toggles in
both themes, authored strong-outline focus fallback, failed focus contrast and
focused recovery, unavailable state, malformed configuration, unsupported focus
policy, rotation, insufficient thumb contrast and recovery after each rejection.
Two cases gain or lose native focus after a replacement has prepared its frame
and before that frame is published; every callback must carry the new focus.
Two cases unload a focused Toggle through a `Loader`, one while its prepared
frame is still pending and one from its own consumer callback, then load a fresh
Toggle with the same configuration.
Light track edges already use the strong-outline fallback, so the failed focus
contrast case assigns the authored strong outline to `outline`; the edge guard
passes and the rejection comes from the focus guard. Final and callback snapshots
are checked against their own observed activation and checked state: track and
thumb states, phase and response, checked color role, literal pigment from the
current inline theme, label colors and contrast, thumb contrast minimum, layout
selection, accessible state and track-only navigation paint. The accessibility
schema owns the portable role. Callbacks must observe the authored checked value,
and their labels and targets must equal the final values. Raw focus observations
and their conjunction follow the Command updates checker. Before publishing a
pending prepared frame, the control synchronizes native focus and any pending
interruption. These cases change focus only; they do not produce a pending
interruption. On destruction the control retires its pending frame and the
keyboard owner ignores synchronization queued before destruction. The run fails
when the host log reports a QML type, reference or invalid-context error.

Pixel checks sample the background, the thumb center, the uncovered track at the
opposite thumb endpoint, label ink on every measured line and, when focused, the
midpoint of each exterior side of the track focus indicator. They do not certify
rounded corners, the complete perimeter, glyph shapes or product styling. These
tests do not deliver pointer or keyboard input, inspect AT-SPI, test reload or
closure, or sample motion.

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
Each held Space or Enter must deliver at least two repeat publications, keep a
published frame and never retract it. The Toggle lifecycle check applies the same
rule. The motion check holds Space through auto-repeat with sampled motion and
requires one clock epoch over held frames spanning at least one second, past the
compositor's repeat delay, and no retraction.

```sh
python tools/check_quickshell_command_lifecycle.py \
  --host /path/to/quickshell --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
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
- a preparation rejection fails the run unless it is logged while the surface
  is hidden or closed, the control is unmounted or has no visible native
  window, as while a closed surface is recreated, or after a reload request in
  the same engine generation; the expected rejections are counted by cause in
  `report.json`;
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

### Released Quickshell Toggle lifecycle

[`check_quickshell_toggle_lifecycle.py`](../../../tools/check_quickshell_toggle_lifecycle.py)
runs [the Toggle lifecycle probe](../../../conformance/quickshell/toggle-lifecycle-probe.qml)
with the session, IPC, virtual input and log reading of the Command lifecycle
checker. It takes the same arguments and compositor session:

```sh
python tools/check_quickshell_toggle_lifecycle.py \
  --host /path/to/quickshell --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/quickshell-toggle-lifecycle
```

Headless Sway 1.12 and nested niri 26.04 pass all 52 cases. Every consumer effect
is counted exactly and must invert the checked value committed before it; the
probe fails when a callback observes a checked value or activation that is not
yet committed. The run fails on QML type, reference and invalid-context errors
anywhere in the host log, and on preparation rejections outside the withdrawn
surfaces and reloads allowed by the Command lifecycle checker; expected
rejections are counted by cause in `report.json`.

- Activation: a pointer click toggles off to on and on to off; a pointer press
  moved off the target and released toggles nothing; Space toggles on release
  and Enter on press, each held through at least two auto-repeat publications;
  an unavailable Toggle rejects a pointer click and `invoke()` and keeps its
  checked value. Withdrawal also removes keyboard focus, so key rejection is not
  observable there.
- Focus: Tab traversal, requested focus, a focus request while another surface
  holds keyboard focus, and availability withdrawal follow the Command cases.
  Two cases move native focus into or out of the Toggle while the frame for a
  checked change is pending: the previous frame is withdrawn and the new one is
  not yet published. Every published frame in the run must carry the focus Qt
  observes at publication and the current checked value in its accessibility
  state and layout.
- Interruption: Space or pointer held across window deactivation by an
  exclusive-keyboard surface, a hidden surface, a parent change, availability
  withdrawal, unmount and a destroyed surface produces no effect. For the four
  cases that keep the Toggle, the hold must be cleared and the checked value
  kept. A fresh Space afterwards toggles exactly once from the current
  generation.
- Reentrancy: consumer callbacks reached by pointer, Space and Enter that set
  checked, invoke the Toggle, withdraw availability, hide, unmount or destroy
  the surface observe committed enabled state. Setting checked returns it to its
  previous value; `invoke()` produces exactly one additional toggle;
  callback-time withdrawal is accepted and leaves the Toggle unavailable.
- Lifetime: hiding and unmounting together from a callback reached by Space,
  Enter and pointer; Space or pointer held across soft and hard reload, which
  must retire the previous generation and deliver nothing; unmounting while the
  frame for a checked or availability change is pending.

The checker observes that a hold is cleared, not which native owner clears it:
the keyboard owner, the control and the native pointer owner each cancel on
parent and window changes. A Space released after window deactivation reaches
the other surface, so the cleared hold, not the effect count, proves that case.
Configuration replacement during a gesture, AT-SPI delivery, motion, physical
devices, multiple seats and outputs, fractional scale and screen-reader
operation are not covered.

### Released Quickshell Command accessibility

[`check_quickshell_command_accessibility.py`](../../../tools/check_quickshell_command_accessibility.py)
runs the same lifecycle probe in a private D-Bus session with the AT-SPI bus and
Qt accessibility enabled. It takes the same arguments and compositor session as
the lifecycle checker and inspects the actual native tree:

- the application exposes exactly one Button, without decorative descendants,
  with the complete name and description, enabled, sensitive, focusable and
  focused state, and exactly one `Press` action, which an available or
  unavailable Command must accept;
- a real AT-SPI `Press` produces exactly one consumer effect, delivered while
  the Command is enabled and holds no gesture, after which the Command is ready,
  available and without a hold again;
- withdrawn availability clears `enabled` and `sensitive` on the same node, each
  observed as an AT-SPI `StateChanged` event through a registered event listener,
  leaves a disabled, unfocusable final state, and the next `Press` produces no
  effect;
- configuration replacement keeps the native node, republishes the current
  non-ASCII name and the replacement's own localized description, and acts
  exactly once under the same conditions as the first `Press`;
- unmounting the control or destroying its surface removes the Button and
  leaves no node in the application tree with a name or an action, and a
  `Press` on the retired node fails with `org.freedesktop.DBus.Error.UnknownObject`
  and produces no effect;
- after a soft and after a hard reload, a `Press` on the Button held from before
  the reload fails the same way without an effect, and the new generation's
  Button produces exactly one effect from that generation;
- a control that its consumer focuses synchronously from `onReadyChanged` after
  being mounted emits a native AT-SPI focus event on its complete Button;
- availability withdrawn from the consumer callback of an AT-SPI `Press` rejects
  the next `Press`;
- the unsupported discoverable-disabled focus policy fails with its diagnostic
  and exposes no Button and no named or actionable node other than the probe's
  own "Focus target" pane;
- starting from a focused Command, hover and press feedback, availability
  withdrawal and restoration and label replacement keep the same Button path in
  the tree. The registered listener records no `ChildrenChanged` removal or
  addition of that node and no empty name or description, and each availability
  change arrives as `enabled` and `sensitive` `StateChanged` events on that node;
- the full AT-SPI `GetState` of that Button, sampled while hovered, during a
  pointer hold inside and outside the target and during a Space hold, has
  `checked`, `pressed`, `selected` and `checkable` clear, and none of these
  updates emits a `StateChanged` event for those states. Qt emits no event for
  `pressed`, so the sampled state is the evidence for it.

Where native focus goes after a focused Command is unmounted or its surface is
destroyed is recorded, without being asserted, in `withdrawal-focus.json`: the
focus events received after the withdrawal and the nodes then reporting
`focused`. In headless Sway 1.12 and nested niri 26.04 the Command was focused
before each withdrawal; afterwards no focus event arrived and no node of the
application reported `focused`, so focus did not move to another accessible
node. Focus recovery is not part of Command qualification.

All thirteen cases pass in headless Sway and nested niri 26.04 with Quickshell 0.3.2
and Qt 6.11.2. `--screen-reader` additionally starts Orca on the same bus with an
English locale and isolated settings and reads its generated presentation
through a pseudo-terminal: Orca block-buffers a regular debug file and never
flushes it. After focus moves to the probe's accessible focus target and Orca's output stops
growing, focusing the control must produce the presentation recorded for the
running Orca release; other releases fail until their presentation is verified.
Orca 50.3 must generate the speech "Reconnect", "button.", "Connect using saved
settings." in sequence and the braille line
"quickshell application frame Reconnect button Connect using saved settings.".
Orca 46.1 must generate the speech "Reconnect push button.", "Connect using
saved settings." in sequence and the braille line "quickshell application frame
Reconnect push button": it does not braille the description. The release is read from `orca --version`
before Orca is started, and the clock shell check applies the same rule. If Orca
exits before "Screen reader on." is read, the failure reports its return code; if
it is still running after 30 s without that announcement, it receives SIGABRT so
that its faulthandler writes every thread's stack to `orca.debug`
([Orca 46.1 `orca.py` line 484](https://gitlab.gnome.org/GNOME/orca/-/blob/ORCA_46_1/src/orca/orca.py#L484)). A shell that
exits during Orca's start fails as a shell exit and Orca is only terminated.
Neither Orca release presents a sensitivity change on its own, so spoken
presentation of availability withdrawal is unverified. Orca's output after the
withdrawal is recorded under `unavailableUnverified` in `screen-reader.json`
without being asserted; it is empty with both releases. The Orca version and
the matched focus output are saved in the same file.

Orca 50.3 with speech-dispatcher passes locally on openSUSE Tumbleweed. CI runs
Ubuntu 24.04's Orca 46.1 with speech-dispatcher, without the espeak-ng output
module or an audio device. The check reads the speech and braille text Orca's
generators produce from its debug output, which records that text whether or not
a speech server renders it; audible speech and braille devices are not
verified.
Orca 46.1 reads its modifier keymap with `xkbcomp` from the X display named by
`DISPLAY` while it loads its settings, and raises `KeyError` when `DISPLAY` is
unset: `refresh_orca_modifiers` calls `subprocess.check_output(['xkbcomp',
os.environ['DISPLAY'], '-'])` ([`orca_modifier_manager.py` line 118](https://gitlab.gnome.org/GNOME/orca/-/blob/ORCA_46_1/src/orca/orca_modifier_manager.py#L118)),
and `loadUserSettings` calls it ([`orca.py` line 178](https://gitlab.gnome.org/GNOME/orca/-/blob/ORCA_46_1/src/orca/orca.py#L178))
from `init()` while the `SIGALRM` start-up watchdog of `settings.timeoutTime`
(10 s) is armed ([lines 284-291](https://gitlab.gnome.org/GNOME/orca/-/blob/ORCA_46_1/src/orca/orca.py#L284-291)).
On the runner, Ubuntu's apport exception hook then collects package checksums
for longer than that, and the watchdog's `timeout()` calls `die(EXIT_CODE_HANG)`,
which sends SIGKILL to Orca before the traceback is printed
([lines 334-351](https://gitlab.gnome.org/GNOME/orca/-/blob/ORCA_46_1/src/orca/orca.py#L334-351)).
CI therefore runs this check under `xvfb-run`; the checker removes `DISPLAY` from
Quickshell's environment, so Quickshell does not receive the X display, while
Orca and other services of the session may. Orca 50.3 skips the keymap step when
`DISPLAY` is unset ([`orca_modifier_manager.py` lines 284-288](https://gitlab.gnome.org/GNOME/orca/-/blob/50.3/src/orca/orca_modifier_manager.py#L284-288)).
Audible output, other screen readers and physical input are not covered.

### Released Quickshell Command motion

[`check_quickshell_command_motion.py`](../../../tools/check_quickshell_command_motion.py)
runs the lifecycle probe with the explicit motion channels of the
[motion request](../../../conformance/ir/command-motion-request.json) and authored
preference variants. The probe logs every published motion sample with its
snapshot and a sequence number; after the run, each frame keeps the authored dynamics and its complete
snapshot and motion sample are checked against the request in effect, the
projected state set and the independent spring equations. Each consumer effect
also logs the snapshot, motion sample, readiness and hover the `activated()`
callback observes, with the sequence number of the last frame logged before it.
The observed frame must be withdrawn (no snapshot, no motion sample, not ready), or
complete, identical to the logged frame with that number and valid for the
committed state the effect reports; a frame published for the superseded pressed
request fails. Input reaches the control through the compositor:

- holding Space publishes several unsettled spring frames. Each sampled epoch's clock
  advances with elapsed native time within 10 % or 50 ms; one channel settles
  before the other, both settle, and no frame follows. Every frame logged between
  the press and the effect holds the press, and the first frame after the effect
  presents the effect's committed state and is the release retarget. A Space press
  held through auto-repeat and the Space activation after a live scale change
  meet the same callback and ordering conditions;
- a press, its release, hover enter and a second press follow while the spring
  is still moving, so each hands over a moving state;
- compositor hover enters and leaves the target through spring frames. Qt's
  Wayland client types the seat pointer as a touchpad when the compositor offers
  pointer gestures, so hover accepts mouse events from mouse and touchpad devices;
- focus loss during motion republishes frames without the focus cue;
- switching to the reduced-motion preference or withdrawing availability during a
  press cancels the held gesture without an effect; the probe logs each request
  before applying it, the first frame logged after it already reflects the change,
  no frame for the earlier request follows, and every later frame is valid for the
  request in effect; reduced motion then publishes
  only immediate settled frames;
- with motion, activation callbacks reached by Space that replace the
  configuration, withdraw availability or invoke the control meet the same
  callback frame condition, including the callback of the reentrant invocation;
  every later frame is valid for the request in effect and keeps no hold, frames
  after a replacement present the replacement, and frames after the withdrawal
  are unavailable;
- moving the control during a press cancels the held gesture without an effect
  and publishes frames at the new position;
- reduced motion and Cast publish immediate settled frames;
- Frost with authored reduced transparency keeps its family and spring policy,
  and every frame publishes the `opaqueDimensional` Frost representation with
  body alpha 1 and no content fallback;
- the same Frost Command without reduced transparency, whose environment
  declares `translucentSurfaces`, selects a translucent body that the opaque
  Command profile rejects: the Command reports the opaque-body preparation
  error, is not ready, has no target and publishes no frame.
  [Command paint](../../../spec/43-command-paint.md) accepts Frost only as
  `opaqueDimensional`, so a released-host Command never publishes
  `translucentPigmented`; this pair shows that the authored preference, and not
  a fixed host value, selects the representation.

Every logged clock epoch boundary is then checked. A boundary is a frame whose
retained `channels` or target differ from those of the frame logged before it
in the same engine generation. It must publish exactly one instant: a
`retargetTime`, or for an epoch resumed by a reconfiguration a `resumeTime`.
Its initial position and velocity must equal, within spec/44's tolerance
`max(1e-9, 1e-9 * abs(expected))`, the state the previous trajectory reaches
at that instant, sampled with that trajectory's own policy; reduced motion and Cast
reach their endpoint with zero velocity. The instant must not precede the
previous frame's sample or exceed the log time elapsed since the frame logged
before that one. That log bound is only as tight as one frame's preparation
time; the precision of the restart instant itself comes from the Qt runtime
clock check, which compares `CommandClock.restart()` with monotonic time. A
`resumeTime` fails unless the probe logged a configuration request between the
two frames or their configuration, device scale or target centre differ. Policy
changes come only from configuration changes, so they are checked this way.

A channel may hand over exactly zero velocity only when the trajectory it
continues is settled at that instant or uses reduced motion or Cast. An
activation callback that replaces the configuration or withdraws availability
supersedes the epoch the runtime prepared for the committed state before it is
published. Of the effects logged between the two frames, take the last whose
committed state has, through the request in effect, a policy or target other
than those of the previous frame: the boundary may instead continue an epoch for
that state that started from the previous trajectory, and both elapsed times
together must fit the same log bound. That start is not published: the closest
of at most 501 evenly spaced instants is refined by ternary search within one
spacing, and the boundary must continue it within the same tolerance at the
published instant. Otherwise a `retargetTime` or `resumeTime` that precedes
the previous sample, or a retarget across a changed configuration, device scale
or position, fails because the epoch it continues was not logged, as does a
published instant on the first frame of a generation or a boundary with only
one earlier frame logged. A rejected configuration fails the control, which
discards its trajectory and clock: the first boundary after a logged rejection
must publish no instant and start from the authored channels. The run must
check at least one boundary each for hover enter, hover exit, press, release, a
policy change and a reconfiguration, and for each of them at least one boundary
that hands over a nonzero velocity. `report.json` counts the checked boundaries
by trigger under `boundaries`, including availability changes and restarts after
a rejection, and those handing over a nonzero velocity under `movingBoundaries`;
one boundary can count for several triggers.

The check runs at the output scale given by `--scale` and first requires the
device pixel ratio of the Command's window to equal it. The probe logs that
ratio with every motion frame and with every publication without motion from
startup on, and each must carry it, so a publication made at a window's
fallback density fails the check. With `--live-scale`, the
check then changes the scale of the single active Sway output through the
only `sway-ipc.*.sock` in `XDG_RUNTIME_DIR` (a set `SWAYSOCK` must name that
socket) while the shell runs, and requires:

- the Command to become ready again at the new device scale with the same
  target centre, and a complete frame at that scale with the same hit region and
  accessibility channel as the last frame before the change;
- logged frames and publications to move once from the old scale to the new
  one;
- Space, with the Command focused again, to publish unsettled spring frames that
  activate it once, track native time and settle at the new scale.

CI runs the check in headless Sway at output scale 1 on a 1024×600 output, and
at 1.25 and 2 on a 1280×720 output with live changes from 1.25 to 2 and from 2
to 1.25. With Quickshell 0.3.2 and Qt 6.11.2, at revision 4cb60fe, it passes
in headless Sway 1.12 at output scale 1 and with live changes from 1.25 to 2 and
2 to 1.25, and in nested niri 26.04 at scale 1. Output scale 1.5 and live
changes from 1 to 2, 2 to 1 and 1.25 to 1.5 passed in headless Sway 1.12 at
earlier revisions and were not repeated at 4cb60fe.

The Command requests keep the authored environment scale of
`conformance/headless/valid-request.json` at every output scale, so a live
change proves the native device scale only; the clock shell check proves that
the environment scale follows the output.

The motion channels are the conformance fixture, not calibrated material
dynamics, and preferences are explicit authored inputs: no platform preference
is discovered. These cases, like every other functional, lifecycle and
accessibility check, run against the debug runtime module; frame timing is
qualified separately against the release module.

### Released Quickshell Command motion frame budget

Motion timing and budgets are qualified only against the release runtime module.
Debug builds remain functional evidence. The release library is built with
`cargo build --release` and packaged as a separate import root. Locally, with
the `target/qt-runtime` target directory used above:

```sh
cargo build --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --release --locked
python tools/prepare_qt_runtime_module.py \
  --library target/qt-runtime/release/libresina_qt_runtime.so \
  --metadata-dir target/qt-runtime-export/qml_modules/Resina/Runtime \
  --output-dir target/qt-runtime-release-import
```

CI sets `CARGO_TARGET_DIR` to `target/qt-runtime-build`, so its library is
`target/qt-runtime-build/release/libresina_qt_runtime.so`.

[`check_quickshell_command_motion_budget.py`](../../../tools/check_quickshell_command_motion_budget.py)
runs the lifecycle probe in headless Sway at the output scale given by `--scale`
and first requires the device pixel ratio of the Command's window, which its
views use as their native device scale, to equal it. Space is
held and released through the compositor ten times, and each phase lasts six
spring frames. With the release module a hold therefore ends before Sway's
default 600 ms key-repeat delay, so repeated key presses add no work to the
measured frames. A spring frame is measured when the frame published before it
was still moving:

- its published interval is the native clock time between the motion samples of
  the two frames. Within one clock epoch that is the difference of their `time`;
  across a retarget it is the previous epoch's `retargetTime` less the previous
  `time`, plus the new `time`. Epochs started without a `retargetTime` are not
  joined, and a clock restart that leaves the channels and target unchanged
  fails the check. Log timestamps are not used;
- its synchronous preparation cost is the published `preparationDuration`. That
  is a lower bound on the frame's main-thread time: the software scene-graph
  render and the publication that follows are not included;
- within one clock epoch, its publication delay is the published
  `publicationDelay`: the time from its predecessor's publication to the start
  of its own refresh, and its predecessor span is the published
  `predecessorSpan`: the time from the start of the predecessor's refresh to
  that publication. The two are published together or not at all.

At least 20 frames are required. The floor is a median published interval of at
most 1/30 s (30 Hz); a longer median fails the check unless `--floor report`
is given, which prints the result without failing. Without enforcing them, the
check also reports for each run the 95th and 99th percentile (nearest rank) and
maximum published interval, the share of intervals longer than 1/30 s (33.3 ms)
and 1/24 s (41.7 ms), and the longest run of consecutive measured intervals
longer than 1/30 s, in a `TAIL` line and under `tail` in `report.json`. A run
whose median meets the floor can still have nearly half of its intervals longer
than 1/30 s. The target is a median
`preparationDuration` of at most 8 ms and a median published interval of at most
17 ms. The target is reported but not enforced. CI runs the check at output
scales 1 and 2 with `--floor report --require-gap` and keeps each `report.json`
with every measured frame. Shared runner timing varies between runs; at
revision 4cb60fe the median interval was 67.9 ms at scale 1 and 84.8 ms at
scale 2, with median preparation of 13.0 ms and 28.6 ms.
Most of each interval there is asynchronous decoding and software rendering, so
runner timing does not qualify the floor. The floor is qualified by running the
check without `--floor report` on the qualifying machine.

The check also bounds the publication gap, on every host. The headless output
refreshes at 60 Hz, so a predecessor whose `predecessorSpan` is at least 1/60 s
saw an animation tick while it was prepared. For those frames the 75th
percentile publication delay must be at most 4 ms. A predecessor published
within one tick may correctly wait for the next tick and is not counted. With
fewer than 20 such frames the gap is reported as not evaluated: the host rarely
used the path the bound protects. `--require-gap` makes that a failure. CI
requires it, so a runner that stops exercising the path fails rather than
leaving the bound unchecked.

Measured locally at revision 4cb60fe in headless Sway 1.12 with the pixman
renderer on a 1024×600 output, Quickshell 0.3.2 and the Qt 6.11.2 software
backend, with the floor enforced:

| Release module, scale | Median interval | Median preparation | Publication gap (75th percentile) |
| --- | ---: | ---: | ---: |
| 1 | 15.8 ms | 9.3–9.6 ms | not evaluated |
| 2 | 19.6–22.0 ms | 15.5–17.6 ms | 0.58–0.68 ms |

These are three runs at each scale, and the floor is met in each. At scale 1,
only 11, 18 and 12 frames had a predecessor that spanned a tick, fewer than the
20 the gap requires. At an earlier revision, with the call that resumes a due
frame after publication removed, the publication gap was 12.27 ms at scale 1
and 11.49 ms at scale 2, failing the check, and the scale 2 median interval was
31.9 ms.

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
stops only after both channels settle. A frame request that arrives while the
previous frame is still being prepared starts the next preparation as soon as that
frame is published, rather than one animation tick later. The driver therefore
keeps the window's render loop ticking while a frame is prepared and decoded,
until both channels settle or the motion fails. Reduced motion and Cast resolve
immediately through the existing reference policy, without toolkit-specific
semantics.

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
elapsed `time`, retained `channels`, `retargetTime`, `resumeTime`,
`preparationDuration`, `publicationDelay`, `predecessorSpan` and the
complete existing motion IR; it is not a new portable IR or action authorization. `retargetTime`
is the previous trajectory's elapsed clock time at which the native clock
restarted and the retained `channels` were sampled from it. Every frame of that
clock epoch repeats it. It is `null` for the first epoch and after a failure or
reconfiguration. A reconfiguration with an unchanged motion profile keeps the
running trajectory and its clock until the next frame is prepared; the clock
then restarts and the resumed `channels` are that trajectory's state at the
restart, not at the change. `resumeTime` is the previous trajectory's elapsed
clock time at that restart, repeated by every frame of the resumed epoch, and
`null` otherwise.
`preparationDuration` is the main-thread time, in seconds from a
separate `CommandClock`, from the start of the refresh that prepared the frame to
the end of its synchronous snapshot, paint and label preparation. It excludes
asynchronous image decoding and the later publication. `publicationDelay` is the
time on that clock from the publication of the previous frame to the start of
this frame's refresh, and `predecessorSpan` is the time on that clock from the
start of the previous frame's refresh to its publication. Both are `null` when
no frame was published since the previous refresh started, and after a failure
or reconfiguration. `CommandSnapshotResolver` commits
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
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-motion-captures
python tools/check_qt_command_control_runtime.py --motion \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-moving-control-captures
python tools/check_qt_command_control_accessibility.py --motion \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter
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
requires its own runtime verification. Released Quickshell Command accessibility
is verified by the thirteen AT-SPI cases of
[Released Quickshell Command accessibility](#released-quickshell-command-accessibility),
which also covers Command screen-reader presentation. Material calibration
remains unverified here.

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
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-toggle-captures
python tools/check_qt_toggle_control_accessibility.py \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter --action Toggle
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

Unmodified Quickshell 0.3.2 removes Quickshell 0.3.1's separate
application-bootstrap accessibility defect; see the
[Quickshell hosting boundary](#quickshell-hosting-boundary). The Switch-role
limitation above comes from the stock Qt 6.11.2 AT-SPI build, not Quickshell.
[Released-Quickshell Toggle qualification](https://github.com/marcoallegretti/Resina-Design_System/issues/186) accepts the CheckBox role on
stock Qt as a Qt-owned limitation. Full Shell accessibility, calibrated
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
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-toggle-motion-captures
python tools/check_qt_toggle_control_runtime.py --motion \
  --raster target/debug/resina-toggle-part-motion-raster \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter \
  --output-dir target/qt-toggle-moving-input
python tools/check_qt_toggle_control_accessibility.py --motion \
  --module-path target/qt-runtime-import \
  --font-dir /usr/share/fonts/opentype/inter --action Toggle
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
above; Quickshell 0.3.2 removes the 0.3.1 host bootstrap defect.
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
