# Qt command lifecycle runtime (0.1.0)

This separately locked native workspace implements the Resina-owned
Rust/QML runtime boundary. CXX-Qt 0.8.1 exposes Rust-owned command activation
through a real Qt `QObject` in the `Resina.Runtime` QML plugin. The portable
reference owns semantics, Qt owns object lifetime and native publication, and
consumers own product effects. Qt and binding dependencies stay outside the
portable workspace and normative artifacts.

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
Slider. Released Quickshell native publication, native keyboard/pointer delivery, screen-reader
operation, continuous text scaling, coherent complete component snapshot/paint
publication and native runtime CI remain unverified. The next dependency is a
Command owner preparing its existing content, paint, target and semantics together.
