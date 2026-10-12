import argparse
import copy
import json
import math
import sys
from decimal import Decimal, localcontext
from fractions import Fraction

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure
from check_focus_indicator_backend import baseline as focus_baseline, check_success as check_indicator
from check_headless_backend import run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for
from check_surface_outline_backend import outline_mismatch


def case_request(case):
    outlines = load_json(ROOT / "conformance/geometry/surface-outline-vectors.json")
    outline = next(item for item in outlines if item["name"] == case["outlineVector"])
    if "expected" not in outline:
        raise ValueError("focus case requires a resolved outline vector")
    focus, binding = focus_baseline()
    focus["scenario"]["surface"]["treatmentStack"]["treatments"] = ["none"]
    binding = copy.deepcopy(binding)
    binding["treatmentStack"]["treatments"] = ["none"]
    request = {
        "schemaVersion": "0.1.0", "focus": focus, "outline": copy.deepcopy(outline["request"]),
        "sideOffset": {"x": 6, "y": -4},
        "directions": [{"x": 1, "y": 0}, {"x": 0, "y": 1}, {"x": -1, "y": 0}, {"x": 0, "y": -1},
                       {"x": 1, "y": 1}, {"x": -1, "y": 1}, {"x": -1, "y": -1}, {"x": 1, "y": -1}],
    }
    return apply_changes(request, case["changes"]), binding, outline["expected"]


def decimal_number(value):
    return Decimal.from_float(value) if isinstance(value, float) else Decimal(value)


def parse_result(source, label):
    actual = parse_json(source, label)
    precise = json.loads(source, parse_float=Decimal)
    if isinstance(actual, dict) and isinstance(actual.get("focus"), dict):
        geometry = actual["focus"].get("geometry")
        if isinstance(geometry, dict):
            for field in ("silhouetteBounds", "innerBounds", "outerBounds"):
                if field in geometry:
                    geometry[field] = precise["focus"]["geometry"][field]
    if isinstance(actual, dict) and "supports" in actual:
        actual["supports"] = precise["supports"]
    return actual


def require_close(actual, expected, label):
    if type(actual) not in (int, float, Decimal):
        raise AssertionError(f"{label}: expected {expected}, got {actual!r}")
    value, target = decimal_number(actual), decimal_number(expected)
    if not value.is_finite() or abs(value - target) > Decimal("1e-12") * max(1, abs(value), abs(target)):
        raise AssertionError(f"{label}: expected {expected}, got {actual!r}")


def check_geometry(actual, request, outline):
    geometry = actual["focus"]["geometry"]
    if outline_mismatch(geometry["outline"], outline):
        raise AssertionError("resolved outline differs from its independent conformance vector")
    if geometry["sideOffset"] != request["sideOffset"]:
        raise AssertionError("physical side offset changed")
    if geometry["innerOutset"] != 2 or geometry["outerOutset"] != 4:
        raise AssertionError("focus disk extents changed")
    if len(actual["supports"]) != len(request["directions"]):
        raise AssertionError("support count differs from ordered directions")
    with localcontext() as context:
        context.prec = 100
        dec = decimal_number
        w, h = (dec(outline["size"][key]) for key in ("width", "height"))
        dx, dy = (dec(request["sideOffset"][key]) for key in ("x", "y"))
        for field, outset in (("silhouetteBounds", 0), ("innerBounds", 2), ("outerBounds", 4)):
            expected = {"x": min(0, dx) - outset, "y": min(0, dy) - outset,
                        "width": w + abs(dx) + 2 * outset, "height": h + abs(dy) + 2 * outset}
            for key, value in expected.items():
                require_close(geometry[field][key], value, f"{field}/{key}")
        for first, second in (("silhouetteBounds", "innerBounds"), ("innerBounds", "outerBounds")):
            for key, extent in (("x", -2), ("y", -2), ("width", 4), ("height", 4)):
                with localcontext() as preservation:
                    preservation.prec = 800
                    difference = dec(geometry[second][key]) - dec(geometry[first][key])
                if difference * extent <= 0:
                    raise AssertionError(f"{first}/{second}/{key}: disk expansion lost")
                require_close(difference, extent, f"{first}/{second}/{key} expansion")
        for index, (query, direction) in enumerate(zip(actual["supports"], request["directions"])):
            nx, ny = (dec(direction[key]) for key in ("x", "y"))
            length = (nx * nx + ny * ny).sqrt()
            nx, ny = nx / length, ny / length
            require_close(query["normal"]["x"], nx, f"normal {index}/x")
            require_close(query["normal"]["y"], ny, f"normal {index}/y")
            for key in ("x", "y"):
                component = dec(query["normal"][key])
                if direction[key] != 0 and component * dec(direction[key]) <= 0:
                    raise AssertionError(f"normal {index}/{key}: nonzero component lost or reversed")
            corner = ("bottom" if ny >= 0 else "top") + ("Right" if nx >= 0 else "Left")
            a, b = (dec(outline["corners"][corner][key]) for key in ("x", "y"))
            cx, cy = (w - a if nx >= 0 else a), (h - b if ny >= 0 else b)
            power = Decimal(2 if outline["curvature"] == "circular" else 4)
            dual = power / (power - 1)
            wx, wy = a * abs(nx), b * abs(ny)
            support_length = (context.power(wx, dual) + context.power(wy, dual)) ** (1 / dual)
            u, v = ((context.power(wx / support_length, 1 / (power - 1)),
                     context.power(wy / support_length, 1 / (power - 1)))
                    if support_length else (Decimal(0), Decimal(0)))
            px, py = cx + (a * u if nx >= 0 else -a * u), cy + (b * v if ny >= 0 else -b * v)
            sweep = dx * nx + dy * ny
            exact_sweep = (Fraction(float(direction["x"])) * Fraction(float(request["sideOffset"]["x"]))
                           + Fraction(float(direction["y"])) * Fraction(float(request["sideOffset"]["y"])))
            if exact_sweep > 0:
                px, py = px + dx, py + dy
            support = cx * nx + cy * ny + support_length + max(0, sweep)
            for field, outset in (("silhouette", 0), ("inner", 2), ("outer", 4)):
                require_close(query[field]["point"]["x"], px + outset * nx, f"{field} {index}/x")
                require_close(query[field]["point"]["y"], py + outset * ny, f"{field} {index}/y")
                require_close(query[field]["support"], support + outset, f"{field} {index}/support")
                dot = dec(query[field]["point"]["x"]) * nx + dec(query[field]["point"]["y"]) * ny
                require_close(query[field]["support"], dot, f"{field} {index}/witness")
            with localcontext() as preservation:
                preservation.prec = 800
                gap = dec(query["inner"]["support"]) - dec(query["silhouette"]["support"])
                stroke = dec(query["outer"]["support"]) - dec(query["inner"]["support"])
            if gap <= 0 or stroke <= 0:
                raise AssertionError("focus loses a positive gap or stroke")
            require_close(gap, 2, f"gap {index}")
            require_close(stroke, 2, f"stroke {index}")


def main():
    parser = argparse.ArgumentParser(description="Check analytic Resina outline focus through its public protocol.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command and args.command[0] == "--" else args.command
    if not command or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("provide a command and a positive finite timeout")
    cases = load_json(ROOT / "conformance/states/outline-focus-cases.json")
    case_schema = validator_for("schemas/outline-focus-case.schema.json")
    request_schema = validator_for("schemas/outline-focus-request.schema.json")
    result_schema = validator_for("schemas/outline-focus-result.schema.json")
    names = set()
    try:
        for case in cases:
            name = case["name"]
            if name in names or list(case_schema.iter_errors(case)):
                raise ValueError(f"invalid or duplicate case: {name}")
            names.add(name)
            request, binding, outline = case_request(case)
            if (not list(request_schema.iter_errors(request))) != case["requestSchemaValid"]:
                raise ValueError(f"request schema disagrees: {name}")
            source = json.dumps(request, allow_nan=False)
            if "failure" in case:
                check_failure(command, source, args.timeout, name)
                continue
            previous = None
            for attempt in range(2):
                completed = run_backend(command, source, args.timeout)
                if case.get("accuracyProbe") and completed.returncode == 1:
                    if previous is not None or completed.stdout or not completed.stderr.strip():
                        raise AssertionError(f"{name}: precision rejection must be atomic and repeatable")
                    check_failure(command, source, args.timeout, name)
                    break
                if completed.returncode != 0 or completed.stderr:
                    raise AssertionError(f"{name}: expected clean success: {completed.stderr[:300]!r}")
                actual = parse_result(completed.stdout, name)
                errors = list(result_schema.iter_errors(actual))
                if errors:
                    raise AssertionError(f"{name}: invalid result: {errors[0].message}")
                check_indicator(actual["focus"]["indicator"], request["focus"], case["expected"], binding)
                check_geometry(actual, request, outline)
                if previous is not None and actual != previous:
                    raise AssertionError(f"{name}: repeated request changed its result")
                previous = actual
        base, _, _ = case_request(cases[0])
        failures = [
            ("positional request", json.dumps(list(base.values()))),
            ("duplicate direction", duplicate_member_source(base, "/directions/0/x")),
            ("duplicate nested focus", duplicate_member_source(base, "/focus/schemaVersion")),
            ("nonfinite projection", nonfinite_member_source(base, "/sideOffset/x")),
        ]
        for name, source in failures:
            check_failure(command, source, args.timeout, name)
    except (AssertionError, OSError, ValueError, ArithmeticError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Outline focus backend passed {len(cases) + len(failures)} conformance cases with independent 100-digit support and precise extent checks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
