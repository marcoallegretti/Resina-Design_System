import argparse
import json
import math
import re
import sys
import xml.etree.ElementTree as ET

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, load_json


NUMBER = r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?"
TOKEN = re.compile(rf"[MLAZ]|{NUMBER}")
SVG = "{http://www.w3.org/2000/svg}"


def close(actual, expected):
    return math.isfinite(actual) and math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)


def check_svg(source, expected):
    root = ET.fromstring(source)
    outer = expected["geometry"]["outer"]
    bounds = outer["contour"]["bounds"]
    values = [bounds["x"] + outer["offset"]["x"], bounds["y"] + outer["offset"]["y"], bounds["width"], bounds["height"]]
    if root.tag != SVG + "svg" or set(root.attrib) != {"width", "height", "viewBox", "aria-hidden", "focusable"}:
        raise ValueError("SVG root profile differs")
    if root.attrib["aria-hidden"] != "true" or root.attrib["focusable"] != "false":
        raise ValueError("decorative focus SVG must not create a second accessibility/focus target")
    view = root.attrib["viewBox"].split()
    if len(view) != 4 or not all(close(float(a), b) for a, b in zip(view, values, strict=True)):
        raise ValueError("SVG viewBox omits or moves required outer bounds")
    if not close(float(root.attrib["width"]), bounds["width"]) or not close(float(root.attrib["height"]), bounds["height"]):
        raise ValueError("SVG intrinsic size differs from logical paint bounds")
    if len(root) != 1 or root[0].tag != SVG + "path" or len(root[0]) or set(root[0].attrib) != {"fill", "fill-rule", "d"}:
        raise ValueError("SVG must contain only one unstyled compound path")
    path = root[0]
    if path.attrib["fill-rule"] != "evenodd":
        raise ValueError("SVG ring requires evenodd hole")
    color = re.fullmatch(rf"rgb\(({NUMBER})% ({NUMBER})% ({NUMBER})%\)", path.attrib["fill"])
    if not color or not all(close(float(a) / 100, b) for a, b in zip(color.groups(), expected["indicator"]["color"]["components"], strict=True)):
        raise ValueError("SVG pigment differs from resolved indicator")
    data = path.attrib["d"]
    if TOKEN.sub("", data).strip():
        raise ValueError("unsupported SVG path token")
    tokens = TOKEN.findall(data)
    required = []
    for placed in (outer, expected["geometry"]["inner"]):
        def point(segment, first):
            if segment["kind"] == "line":
                endpoint = segment["from" if first else "to"]
            else:
                radial = segment["start" if first else "end"]
                endpoint = {axis: segment["center"][axis] + segment["radii"][axis] * radial[axis] for axis in ("x", "y")}
            return [endpoint[axis] + placed["offset"][axis] for axis in ("x", "y")]
        segments = placed["contour"]["segments"]
        required.extend(["M", *point(segments[0], True)])
        for segment in segments:
            if segment["kind"] == "line":
                required.extend(["L", *point(segment, False)])
            else:
                required.extend(["A", segment["radii"]["x"], segment["radii"]["y"], 0, "0", "1", *point(segment, False)])
        required.append("Z")
    if len(tokens) != len(required) or not all(a == b if isinstance(b, str) else close(float(a), b) for a, b in zip(tokens, required, strict=True)):
        raise ValueError("SVG path differs from both translated canonical boundaries")


def main():
    parser = argparse.ArgumentParser(description="Check the Resina focus SVG realization against public IR cases.")
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
        for case in cases:
            request = apply_changes(baseline, case["requestChanges"])
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            if "errorContains" in case:
                check_failure(command, source, arguments.timeout, case["name"])
                continue
            result = case.get("expected", apply_changes(expected, case.get("expectedChanges", [])))
            previous = None
            for attempt in range(2):
                completed = run_backend(command, source, arguments.timeout)
                if completed.returncode or completed.stderr:
                    raise ValueError(f"{case['name']}: backend exit {completed.returncode}, stderr {completed.stderr[:200]!r}")
                check_svg(completed.stdout, result)
                if previous is not None and previous != completed.stdout:
                    raise ValueError(f"{case['name']}: repeated SVG differs")
                previous = completed.stdout
        for name, source in (
            ("duplicate root version", duplicate_member_source(baseline, "/schemaVersion")),
            ("nonfinite size", nonfinite_member_source(baseline, "/size/width")),
            ("browser coordinate precision", json.dumps(apply_changes(baseline, [{"path": "/size/width", "value": 100000000}]))),
        ):
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError, ET.ParseError) as error:
        print(f"FAIL focus SVG: {error}", file=sys.stderr)
        return 1
    print(f"Focus SVG passed {len(cases) + 3} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
