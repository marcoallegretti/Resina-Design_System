import argparse
import json
import math
import re
import sys

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, load_json


NUMBER = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
TOKEN = re.compile(rf'{NUMBER}|[A-Za-z_][A-Za-z_0-9.]*|"[^"\n]*"|[{{}}:;,()]')


def expected_qml(ir):
    outer = ir["geometry"]["outer"]
    bounds = outer["contour"]["bounds"]
    origin = {axis: bounds[axis] + outer["offset"][axis] for axis in ("x", "y")}
    red, green, blue = ir["indicator"]["color"]["components"]
    source = f'''import QtQuick
import QtQuick.Shapes
Shape {{
readonly property real paintOriginX: {origin["x"]}
readonly property real paintOriginY: {origin["y"]}
implicitWidth: {bounds["width"]}
implicitHeight: {bounds["height"]}
fillMode: Shape.NoResize
preferredRendererType: Shape.CurveRenderer
containsMode: Shape.FillContains
focus: false
activeFocusOnTab: false
Accessible.ignored: true
ShapePath {{
strokeColor: "transparent"
strokeWidth: 0
fillColor: Qt.rgba({red}, {green}, {blue}, 1)
fillRule: ShapePath.OddEvenFill
'''
    for placed in (outer, ir["geometry"]["inner"]):
        def point(segment, first=False):
            if segment["kind"] == "line":
                endpoint = segment["from" if first else "to"]
            else:
                radial = segment["start" if first else "end"]
                endpoint = {axis: segment["center"][axis] + segment["radii"][axis] * radial[axis] for axis in ("x", "y")}
            return [endpoint[axis] + placed["offset"][axis] - origin[axis] for axis in ("x", "y")]
        segments = placed["contour"]["segments"]
        first_x, first_y = point(segments[0], True)
        source += f"PathMove {{ x: {first_x}; y: {first_y} }}\n"
        for segment in segments:
            x, y = point(segment)
            if segment["kind"] == "line":
                source += f"PathLine {{ x: {x}; y: {y} }}\n"
            else:
                radii = segment["radii"]
                source += f'PathArc {{ x: {x}; y: {y}; radiusX: {radii["x"]}; radiusY: {radii["y"]}; xAxisRotation: 0; direction: PathArc.Clockwise; useLargeArc: false }}\n'
        source += f"PathLine {{ x: {first_x}; y: {first_y} }}\n"
    return source + "}\n}\n"


def check_qml(source, expected):
    if TOKEN.sub("", source).strip():
        raise ValueError("unsupported QML syntax")
    actual_lines = [TOKEN.findall(line) for line in source.splitlines() if line.strip()]
    required_lines = [TOKEN.findall(line) for line in expected_qml(expected).splitlines() if line.strip()]
    if len(actual_lines) != len(required_lines) or any(len(a) != len(b) for a, b in zip(actual_lines, required_lines, strict=True)):
        raise ValueError("QML structure differs from the native focus profile")
    actual = [token for line in actual_lines for token in line]
    required = [token for line in required_lines for token in line]
    for a, b in zip(actual, required, strict=True):
        if re.fullmatch(NUMBER, b):
            if not re.fullmatch(NUMBER, a) or not math.isfinite(float(a)) or not math.isclose(float(a), float(b), rel_tol=1e-12, abs_tol=1e-12):
                raise ValueError("QML bounds, coordinates or pigment differ")
        elif a != b:
            raise ValueError("QML structure differs from the native focus profile")


def main():
    parser = argparse.ArgumentParser(description="Check native focus QML against public Resina IR cases.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    baseline = load_json(ROOT / "conformance/ir/focus-ir-request.json")
    expected = load_json(ROOT / "conformance/ir/focus-ir-expected.json")
    cases = load_json(ROOT / "conformance/ir/focus-ir-cases.json")
    try:
        check_qml((ROOT / "conformance/quickshell/focus-baseline.qml").read_text(encoding="utf-8"), expected)
        for case in cases:
            source = json.dumps(apply_changes(baseline, case["requestChanges"]), ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, arguments.timeout, case["name"])
                continue
            result = case.get("expected", apply_changes(expected, case.get("expectedChanges", [])))
            previous = None
            for attempt in range(2):
                completed = run_backend(command, source, arguments.timeout)
                if completed.returncode or completed.stderr:
                    raise ValueError(f"{case['name']}: backend exit {completed.returncode}, stderr {completed.stderr[:200]!r}")
                check_qml(completed.stdout, result)
                if previous is not None and previous != completed.stdout:
                    raise ValueError(f"{case['name']}: repeated QML differs")
                previous = completed.stdout
        for name, source in (
            ("duplicate root version", duplicate_member_source(baseline, "/schemaVersion")),
            ("nonfinite size", nonfinite_member_source(baseline, "/size/width")),
            ("Qt coordinate precision", json.dumps(apply_changes(baseline, [{"path": "/size/width", "value": 100000000}]))),
        ):
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL focus QML: {error}", file=sys.stderr)
        return 1
    print(f"Focus QML passed {len(cases) + 3} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
