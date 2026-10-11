import argparse
import json
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]


def agrees(actual, expected):
    return json.dumps(actual, sort_keys=True, allow_nan=False) == json.dumps(expected, sort_keys=True, allow_nan=False)


def check_controller(component, case):
    controller = component.create()
    if controller is None:
        raise ValueError("cannot instantiate Rust CommandController: " + repr(component.errors()))
    try:
        state = case["request"]["state"]
        seed = [{"kind": "availability", "enabled": state["enabled"]},
                {"kind": "focus", "focused": state["focused"]}]
        hold = state["hold"]
        if hold is not None:
            if hold["kind"] == "pointer":
                seed.append({"kind": "pointerDown", "id": hold["id"], "inside": True})
                seed.append({"kind": "pointerMove", "id": hold["id"], "inside": hold["inside"]})
            else:
                seed.append({"kind": "keyDown", "key": hold["key"], "repeat": False})
        for event in seed:
            if not controller.dispatch(json.dumps(event)):
                raise ValueError("cannot prepare conformance state: " + controller.property("lastError"))
        if not agrees(json.loads(controller.property("publication"))["state"], state):
            raise ValueError("Qt controller did not prepare the supplied conformance state")
        publications, activations, captures = [], [], []
        controller.publicationChanged.connect(lambda: publications.append(json.loads(controller.property("publication"))))
        controller.activated.connect(lambda: activations.append(json.loads(controller.property("publication"))))
        controller.captureRequested.connect(lambda identity, acquire: captures.append(
            {"kind": "acquire" if acquire else "release", "id": identity}))
        if not controller.dispatch(json.dumps(case["request"]["event"])):
            raise ValueError("valid Qt event rejected: " + controller.property("lastError"))
        expected = case["expected"]
        if not agrees(publications, [expected]) or not agrees(json.loads(controller.property("publication")), expected):
            raise ValueError("Qt publication differs from shared activation case: " + case["name"])
        if not agrees(activations, [expected] if expected["activate"] else []):
            raise ValueError("intent was missing, duplicated or delivered before commit: " + case["name"])
        if not agrees(captures, [expected["capture"]] if expected["capture"] is not None else []):
            raise ValueError("capture delivery differs from shared case: " + case["name"])
        if controller.setProperty("publication", "{}") or controller.setProperty("lastError", ""):
            raise ValueError("native lifecycle properties must be read-only")
    finally:
        controller.deleteLater()


def check_rejection_and_reentrancy(component):
    controller = component.create()
    if controller is None:
        raise ValueError("cannot instantiate Rust CommandController")
    try:
        intents, publications = [], []
        controller.activated.connect(lambda: intents.append(True))
        controller.publicationChanged.connect(lambda: publications.append(True))
        before = controller.property("publication")
        for event in ('{"kind":"invoke","kind":"invoke"}', '[]',
                      '{"kind":"pointerDown","id":"","inside":true}',
                      '{"kind":"invoke","unexpected":true}', " " * (65536 + 1)):
            if controller.dispatch(event) or not controller.property("lastError"):
                raise ValueError("invalid native event was not rejected diagnostically")
            if controller.property("publication") != before or intents or publications:
                raise ValueError("rejection changed native publication or delivered an intent")
        if not controller.dispatch('{"kind":"availability","enabled":true}'):
            raise ValueError("valid delivery did not recover after rejection")
        if controller.property("lastError"):
            raise ValueError("successful delivery did not clear its prior diagnostic")
        reentrant = []
        def during_publication():
            reentrant.append(controller.dispatch('{"kind":"invoke"}'))
        controller.publicationChanged.connect(during_publication)
        if not controller.dispatch('{"kind":"invoke"}') or reentrant != [False] or intents != [True]:
            raise ValueError("publication reentrancy replayed or lost an activation")
        controller.publicationChanged.disconnect(during_publication)
        def disable_after_intent():
            if not controller.dispatch('{"kind":"availability","enabled":false}'):
                raise ValueError("product callback could not update live availability")
        controller.activated.connect(disable_after_intent)
        if not controller.dispatch('{"kind":"invoke"}'):
            raise ValueError("native invocation failed")
        if not controller.dispatch('{"kind":"invoke"}') or intents != [True, True]:
            raise ValueError("stale invocation replayed an intent")
    finally:
        controller.deleteLater()


def main():
    parser = argparse.ArgumentParser(description="Check compiled Qt controller delivery against shared activation cases (synthetic events).")
    parser.add_argument("--module-path", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        from PySide6.QtCore import QCoreApplication, QEvent, QPluginLoader, QUrl, qVersion
        from PySide6.QtQml import QQmlComponent, QQmlEngine
        from PySide6 import QtQuick  # Load the pinned host's native plugin dependency.
        if qVersion() != "6.11.2":
            raise ValueError("requires Qt 6.11.2")
        application = QCoreApplication([])
        loader = QPluginLoader(str(arguments.module_path.resolve(strict=True) / "Resina/Runtime/libResina_Runtime.so"))
        if loader.instance() is None:
            raise ValueError("Qt plugin factory failed: " + loader.errorString())
        engine = QQmlEngine()
        engine.addImportPath(str(arguments.module_path.resolve(strict=True)))
        component = QQmlComponent(engine)
        component.setData(b"import Resina.Runtime\nCommandController {}", QUrl())
        cases = json.loads((ROOT / "conformance/interaction/activation-cases.json").read_text())
        for case in cases:
            if "expected" in case:
                check_controller(component, case)
                application.sendPostedEvents(None, QEvent.DeferredDelete)
        check_rejection_and_reentrancy(component)
        application.sendPostedEvents(None, QEvent.DeferredDelete)
    except (ImportError, OSError, ValueError, RuntimeError) as error:
        print("FAIL compiled Qt command lifecycle: " + str(error), file=sys.stderr)
        return 1
    print("Compiled Qt command publication, capture and intent delivery match shared activation cases; rejection and reentrancy passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
