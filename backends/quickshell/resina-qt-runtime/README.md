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
Button. Complete component composition remains the next dependency.

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
cargo test --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --locked
cargo clippy --manifest-path backends/quickshell/resina-qt-runtime/Cargo.toml --all-targets --locked -- -D warnings
python -m pip install -r tools/requirements-qt-native.txt -r tools/requirements-schema.txt
python tools/check_qt_command_runtime.py --module-path target/qt-runtime-import
python tools/check_qt_command_label_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
python tools/check_qt_command_snapshot_runtime.py --module-path target/qt-runtime-import --font /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" python tools/check_qt_native_availability.py --report-only
RESINA_QT_RUNTIME_MODULE="$PWD/target/qt-runtime-import" python tools/check_qt_native_availability.py
```

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
measurement and drawing. Released Quickshell native publication, native
keyboard/pointer delivery, screen-reader operation, coherent complete component
paint/publication, motion and native runtime CI remain unverified. The next
dependency is native rendering/publication of the checked snapshot, followed by
a Command owner connecting its lifecycle and real input.
