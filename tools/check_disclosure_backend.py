import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source
from check_color_guard_backend import check_success
from check_headless_backend import run_backend
from check_schemas import ROOT, check_case, load_json, validator_for


def cases():
    return load_json(ROOT / "conformance/interaction/disclosure-cases.json")


def _pointer_identity(hold):
    if hold is not None and hold["kind"] == "pointer":
        return hold["id"]
    return None


def _activation(state, event):
    state = copy.deepcopy(state)
    before_pointer = _pointer_identity(state["hold"])
    activate = False
    kind = event["kind"]
    hold = state["hold"]
    if kind == "availability":
        state["enabled"] = event["enabled"]
        if not state["enabled"]:
            state["hold"] = None
    elif kind == "focus":
        state["focused"] = event["focused"]
        if not state["focused"] and hold is not None and hold["kind"] == "key":
            state["hold"] = None
    elif kind == "cancel":
        state["hold"] = None
    elif kind == "invoke" and state["enabled"]:
        state["hold"] = None
        activate = True
    elif kind == "pointerDown" and state["enabled"] and event["inside"] and hold is None:
        state["hold"] = {"kind": "pointer", "id": event["id"], "inside": True}
    elif kind == "pointerMove" and _pointer_identity(hold) == event["id"]:
        state["hold"] = {
            "kind": "pointer", "id": event["id"], "inside": event["inside"],
        }
    elif kind == "pointerUp" and _pointer_identity(hold) == event["id"]:
        state["hold"] = None
        activate = event["inside"]
    elif kind == "pointerCancel" and _pointer_identity(hold) == event["id"]:
        state["hold"] = None
    elif (kind == "keyDown" and state["enabled"] and state["focused"]
          and not event["repeat"] and hold is None):
        state["hold"] = {"kind": "key", "key": event["key"]}
        activate = event["key"] == "enter"
    elif (kind == "keyUp" and hold is not None and hold["kind"] == "key"
          and hold["key"] == event["key"]):
        state["hold"] = None
        activate = event["key"] == "space"
    after_pointer = _pointer_identity(state["hold"])
    capture = None
    if before_pointer != after_pointer:
        if before_pointer is not None:
            capture = {"kind": "release", "id": before_pointer}
        else:
            capture = {"kind": "acquire", "id": after_pointer}
    hold = state["hold"]
    pressed = hold is not None and (hold["kind"] == "key" or hold["inside"])
    return {
        "schemaVersion": "0.1.0",
        "state": state,
        "activate": activate,
        "pressed": pressed,
        "capture": capture,
    }


def expected_result(request):
    if request["schemaVersion"] != "0.1.0":
        raise ValueError("schemaVersion must be 0.1.0")
    if not request["name"].strip():
        raise ValueError("disclosure name must not be blank")
    if "description" not in request:
        raise ValueError("missing field `description`")
    description = request["description"]
    if description is not None and not description.strip():
        raise ValueError("disclosure description must be nonblank or absent")
    if "controlledContent" not in request:
        raise ValueError("missing field `controlledContent`")
    controlled = request["controlledContent"]
    if not controlled["identity"]:
        raise ValueError("controlled content identity must not be empty")
    if not controlled["present"]:
        raise ValueError("controlled content identity is stale")
    state = request["activation"]
    event = request["event"]
    if event["kind"].startswith("pointer") and not event["id"]:
        raise ValueError("pointer ID must not be empty")
    hold = state["hold"]
    if hold is not None and hold["kind"] == "pointer" and not hold["id"]:
        raise ValueError("pointer ID must not be empty")
    if not state["enabled"] and hold is not None:
        raise ValueError("disabled state must not hold a press")
    if not state["focused"] and hold is not None and hold["kind"] == "key":
        raise ValueError("keyboard hold requires actual focus")
    if not request["focusable"] and state["enabled"]:
        raise ValueError("enabled disclosure must be focusable")
    if not request["focusable"] and state["focused"]:
        raise ValueError("actual disclosure focus requires focusability")
    activation = _activation(state, event)
    next_state = activation["state"]
    if not request["focusable"] and next_state["enabled"]:
        raise ValueError("enabled disclosure must be focusable")
    if not request["focusable"] and next_state["focused"]:
        raise ValueError("actual disclosure focus requires focusability")
    expanded = request["expanded"] ^ activation["activate"]
    return {
        "schemaVersion": "0.1.0",
        "activation": activation,
        "expanded": expanded,
        "accessibility": {
            "schemaVersion": "0.1.0",
            "role": "button",
            "name": request["name"],
            "description": description,
            "value": None,
            "state": {
                "enabled": next_state["enabled"],
                "focused": next_state["focused"],
                "expanded": expanded,
            },
            "actions": [{"kind": "invoke", "available": next_state["enabled"]}],
            "focusable": request["focusable"],
            "relationships": [{"kind": "controls", "target": controlled["identity"]}],
        },
    }


def result_mismatch(actual, expected):
    return None if actual == expected else "/"


def activation_projections():
    projections = []
    for case in load_json(ROOT / "conformance/interaction/activation-cases.json"):
        if "expected" not in case:
            continue
        for expanded in (False, True):
            request = {
                "schemaVersion": "0.1.0",
                "name": "Projected disclosure",
                "description": None,
                "activation": copy.deepcopy(case["request"]["state"]),
                "focusable": True,
                "expanded": expanded,
                "controlledContent": {"identity": "projected-content", "present": True},
                "event": copy.deepcopy(case["request"]["event"]),
            }
            expected = expected_result(request)
            if expected["activation"] != case["expected"]:
                raise AssertionError(
                    f"activation projection differs from {case['name']}"
                )
            projections.append((f"{case['name']} expanded={expanded}", request, expected))
    return projections


def _check_failure(command, source, timeout, name, message=None):
    completed = run_backend(command, source, timeout)
    if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
        raise AssertionError(
            f"{name}: failure must exit 1 with empty stdout and a diagnostic; "
            f"got exit {completed.returncode}, stdout {completed.stdout[:200]!r}, "
            f"stderr {completed.stderr[:200]!r}"
        )
    if message is not None and message not in completed.stderr:
        raise AssertionError(f"{name}: diagnostic did not contain {message!r}")


def main():
    parser = argparse.ArgumentParser(description="Check portable Resina disclosure semantics.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")

    case_validator = validator_for("schemas/disclosure-case.schema.json")
    request_validator = validator_for("schemas/disclosure-request.schema.json")
    result_validator = validator_for("schemas/disclosure-result.schema.json")
    public_cases = cases()
    names = set()
    try:
        if not public_cases:
            raise ValueError("disclosure cases must not be empty")
        for case in public_cases:
            name = case["name"]
            check_case(case_validator, name, case, True)
            if name in names:
                raise ValueError(f"duplicate disclosure case: {name}")
            names.add(name)
            check_case(request_validator, name, case["request"], case["requestSchemaValid"])
            source = json.dumps(case["request"], ensure_ascii=False, allow_nan=False)
            if "expected" in case:
                oracle = expected_result(case["request"])
                if oracle != case["expected"]:
                    raise AssertionError(f"{name}: vector differs from independent oracle")
                check_success(
                    command, source, oracle, result_validator,
                    arguments.timeout, name, result_mismatch,
                )
            else:
                try:
                    expected_result(case["request"])
                except ValueError as error:
                    if case["errorContains"] not in str(error):
                        raise AssertionError(f"{name}: oracle failed for the wrong reason: {error}")
                else:
                    raise AssertionError(f"{name}: oracle accepted a negative case")
                _check_failure(command, source, arguments.timeout, name)

        baseline = public_cases[0]["request"]
        failures = (
            ("duplicate version", duplicate_member_source(baseline, "/schemaVersion")),
            ("duplicate target identity", duplicate_member_source(
                baseline, "/controlledContent/identity")),
            ("duplicate expanded state", duplicate_member_source(baseline, "/expanded")),
            ("unknown root member", json.dumps({**baseline, "backend": "native"})),
        )
        for name, source in failures:
            _check_failure(command, source, arguments.timeout, name)
        projections = activation_projections()
        for name, request, expected in projections:
            check_case(request_validator, name, request, True)
            check_success(
                command,
                json.dumps(request, ensure_ascii=False, allow_nan=False),
                expected,
                result_validator,
                arguments.timeout,
                name,
                result_mismatch,
            )
    except (AssertionError, KeyError, OSError, ValueError) as error:
        print(f"FAIL disclosure: {error}", file=sys.stderr)
        return 1
    print(
        "Disclosure backend passed "
        f"{len(public_cases) + len(failures) + len(projections)} conformance cases"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
