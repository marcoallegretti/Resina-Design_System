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
TOKEN = re.compile(rf'{NUMBER}|[A-Za-z_][A-Za-z_0-9.-]*|[{{}}:;,()<>]')


def expected_slint(ir):
    outer = ir["geometry"]["outer"]
    bounds = outer["contour"]["bounds"]
    origin = {axis: bounds[axis] + outer["offset"][axis] for axis in ("x", "y")}
    red, green, blue = [math.floor(c * 255 + 0.5) for c in ir["indicator"]["color"]["components"]]
    source = f'''export component ResinaFocusIndicator inherits Rectangle {{
out property <length> paint-origin-x: {origin["x"]}px;
out property <length> paint-origin-y: {origin["y"]}px;
width: {bounds["width"]}px;
height: {bounds["height"]}px;
background: transparent;
clip: false;
accessible-role: none;
Path {{
width: root.width;
height: root.height;
viewbox-x: 0;
viewbox-y: 0;
viewbox-width: {bounds["width"]};
viewbox-height: {bounds["height"]};
fit: preserve;
fill: rgb({red}, {green}, {blue});
fill-rule: nonzero;
stroke: transparent;
stroke-width: 0px;
accessible-role: none;
'''
    for reverse, placed in enumerate((outer, ir["geometry"]["inner"])):
        def point(segment, first):
            if segment["kind"] == "line":
                endpoint = segment["from" if first else "to"]
            else:
                radial = segment["start" if first else "end"]
                endpoint = {axis: segment["center"][axis] + segment["radii"][axis] * radial[axis] for axis in ("x", "y")}
            return [endpoint[axis] + placed["offset"][axis] - origin[axis] for axis in ("x", "y")]
        segments = placed["contour"]["segments"][::-1] if reverse else placed["contour"]["segments"]
        x, y = point(segments[0], not reverse)
        source += f"MoveTo {{ x: {x}; y: {y}; }}\n"
        for segment in segments:
            x, y = point(segment, bool(reverse))
            if segment["kind"] == "line":
                source += f"LineTo {{ x: {x}; y: {y}; }}\n"
            else:
                radii = segment["radii"]
                sweep = "false" if reverse else "true"
                source += f'ArcTo {{ x: {x}; y: {y}; radius-x: {radii["x"]}; radius-y: {radii["y"]}; x-rotation: 0; sweep: {sweep}; large-arc: false; }}\n'
        source += "Close {}\n"
    return source + "}\n}\n"


def check_slint(source, expected):
    if TOKEN.sub("", source).strip():
        raise ValueError("unsupported Slint syntax")
    actual_lines = [TOKEN.findall(line) for line in source.splitlines() if line.strip()]
    required_lines = [TOKEN.findall(line) for line in expected_slint(expected).splitlines() if line.strip()]
    if len(actual_lines) != len(required_lines) or any(len(a) != len(b) for a, b in zip(actual_lines, required_lines, strict=True)):
        raise ValueError("Slint structure differs from the native focus profile")
    for a, b in zip((t for line in actual_lines for t in line), (t for line in required_lines for t in line), strict=True):
        if re.fullmatch(NUMBER, b):
            if not re.fullmatch(NUMBER, a) or not math.isfinite(float(a)) or not math.isclose(float(a), float(b), rel_tol=1e-12, abs_tol=1e-12):
                raise ValueError("Slint bounds, coordinates or pigment differ")
        elif a != b:
            raise ValueError("Slint structure differs from the native focus profile")


def main():
    parser = argparse.ArgumentParser(description="Check native focus Slint against public Resina IR cases.")
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
        check_slint((ROOT / "conformance/slint/focus-baseline.slint").read_text(encoding="utf-8"), expected)
        for case in cases:
            source = json.dumps(apply_changes(baseline, case["requestChanges"]), ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, arguments.timeout, case["name"])
                continue
            result = case.get("expected", apply_changes(expected, case.get("expectedChanges", [])))
            previous = None
            for _ in range(2):
                completed = run_backend(command, source, arguments.timeout)
                if completed.returncode or completed.stderr:
                    raise ValueError(f"{case['name']}: backend exit {completed.returncode}, stderr {completed.stderr[:200]!r}")
                check_slint(completed.stdout, result)
                if previous is not None and previous != completed.stdout:
                    raise ValueError(f"{case['name']}: repeated Slint differs")
                previous = completed.stdout
        for name, source in (
            ("duplicate root version", duplicate_member_source(baseline, "/schemaVersion")),
            ("nonfinite size", nonfinite_member_source(baseline, "/size/width")),
            ("Slint coordinate precision", json.dumps(apply_changes(baseline, [{"path": "/size/width", "value": 100000000}]))),
        ):
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL focus Slint: {error}", file=sys.stderr)
        return 1
    print(f"Focus Slint passed {len(cases) + 3} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
