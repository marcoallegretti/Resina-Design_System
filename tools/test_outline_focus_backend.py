import copy
import io
import json
import math
import sys
import tempfile
import unittest
from decimal import Decimal, localcontext
from pathlib import Path
from contextlib import redirect_stderr, redirect_stdout
from subprocess import CompletedProcess
from unittest.mock import patch

from check_focus_indicator_backend import contrast
from check_outline_focus_backend import case_request, check_geometry, main, parse_result
from check_schemas import ROOT, load_json, validator_for


def fixture():
    case = load_json(ROOT / "conformance/states/outline-focus-cases.json")[0]
    request, binding, outline = case_request(case)
    request["directions"] = request["directions"][:4]
    supports = []
    for direction, point, scalar in zip(
        request["directions"], [(126, 44), (100, 60), (0, 48), (106, -4)], [126, 60, 0, 4]
    ):
        query = {"normal": direction}
        for field, outset in (("silhouette", 0), ("inner", 2), ("outer", 4)):
            query[field] = {
                "point": {"x": point[0] + outset * direction["x"], "y": point[1] + outset * direction["y"]},
                "support": scalar + outset,
            }
        supports.append(query)
    resolution = request["focus"]["scenario"]["resolution"]
    path = resolution["opaqueColorAssignments"]["roles"]["focus"].split(".")[1]
    channels = resolution["tokens"]["palette"][path]["$value"]["components"]
    actual = {
        "schemaVersion": "0.1.0",
        "focus": {
            "schemaVersion": "0.1.0",
            "indicator": {
                "schemaVersion": "0.1.0", "binding": binding, "colorRole": "focus",
                "color": {"colorSpace": "srgb", "components": channels, "alpha": 1},
                "contrastRatio": contrast(channels, [1, 1, 1]), "fallbackApplied": False,
                "strokeWidth": 2, "gap": 2,
            },
            "geometry": {
                "outline": outline, "sideOffset": {"x": 6, "y": -4}, "innerOutset": 2, "outerOutset": 4,
                "silhouetteBounds": {"x": 0, "y": -4, "width": 126, "height": 64},
                "innerBounds": {"x": -2, "y": -6, "width": 130, "height": 68},
                "outerBounds": {"x": -4, "y": -8, "width": 134, "height": 72},
            },
        },
        "supports": supports,
    }
    return request, outline, actual


def precise_wire(value):
    if isinstance(value, Decimal):
        return str(value)
    if isinstance(value, dict):
        return "{" + ",".join(json.dumps(key) + ":" + precise_wire(item) for key, item in value.items()) + "}"
    if isinstance(value, list):
        return "[" + ",".join(precise_wire(item) for item in value) + "]"
    return json.dumps(value, allow_nan=False)


def precise_probe(case):
    request, _, outline = case_request(case)
    _, _, actual = fixture()
    geometry = actual["focus"]["geometry"]
    geometry["outline"] = outline
    geometry["sideOffset"] = copy.deepcopy(request["sideOffset"])
    with localcontext() as context:
        context.prec = 800
        dec = lambda value: Decimal.from_float(value) if isinstance(value, float) else Decimal(value)
        w, h = (dec(outline["size"][key]) for key in ("width", "height"))
        dx, dy = (dec(request["sideOffset"][key]) for key in ("x", "y"))
        for field, extent in (("silhouetteBounds", 0), ("innerBounds", 2), ("outerBounds", 4)):
            geometry[field] = {"x": min(0, dx) - extent, "y": min(0, dy) - extent,
                               "width": w + abs(dx) + 2 * extent, "height": h + abs(dy) + 2 * extent}
        supports = []
        for direction in request["directions"]:
            x, y = (dec(direction[key]) for key in ("x", "y"))
            with localcontext() as powers:
                powers.prec = 100
                magnitude = (x * x + y * y).sqrt()
                nx, ny = x / magnitude, y / magnitude
            corner = ("bottom" if ny >= 0 else "top") + ("Right" if nx >= 0 else "Left")
            a, b = (dec(outline["corners"][corner][key]) for key in ("x", "y"))
            cx, cy = w - a if nx >= 0 else a, h - b if ny >= 0 else b
            # Solve the quartic boundary gradient; scalar witnesses are independent point dot products.
            with localcontext() as powers:
                powers.prec = 100
                denominator = powers.power(powers.power(a * abs(nx), Decimal(4) / 3)
                                           + powers.power(b * abs(ny), Decimal(4) / 3), Decimal(1) / 4)
                u = powers.power(a * abs(nx), Decimal(1) / 3) / denominator if denominator else Decimal(0)
                v = powers.power(b * abs(ny), Decimal(1) / 3) / denominator if denominator else Decimal(0)
            px = cx + (a * u if nx >= 0 else -a * u)
            py = cy + (b * v if ny >= 0 else -b * v)
            if x * dx + y * dy > 0:
                px, py = px + dx, py + dy
            query = {"normal": {"x": nx, "y": ny}}
            for field, extent in (("silhouette", 0), ("inner", 2), ("outer", 4)):
                qx, qy = px + extent * nx, py + extent * ny
                query[field] = {"point": {"x": qx, "y": qy}, "support": qx * nx + qy * ny}
            supports.append(query)
        actual["supports"] = supports
    return request, outline, actual


class OutlineFocusCheckerTests(unittest.TestCase):
    def test_all_precision_probes_accept_precise_output_through_the_wire_boundary(self):
        cases = load_json(ROOT / "conformance/states/outline-focus-cases.json")
        probes = [case for case in cases if case.get("accuracyProbe")]
        self.assertEqual(len(probes), 5)
        for probe in probes:
            with self.subTest(probe=probe["name"]), tempfile.TemporaryDirectory() as directory:
                request, _, actual = precise_probe(probe)
                source = json.dumps(request, allow_nan=False)
                response = precise_wire(actual)
                backend = Path(directory) / "backend.py"
                backend.write_text("import sys\nsource=sys.stdin.read()\n"
                                   + "if source == " + repr(source) + ":\n    print(" + repr(response) + ")\n"
                                   + "else:\n    print('invalid request',file=sys.stderr)\n    sys.exit(1)\n", encoding="utf-8")
                def documents(path):
                    return [probe] if path.name == "outline-focus-cases.json" else load_json(path)
                with patch("sys.argv", ["check", "--", sys.executable, str(backend), "-"]), \
                     patch("check_outline_focus_backend.load_json", side_effect=documents), \
                     redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    self.assertEqual(main(), 0)

    def test_precise_output_rejects_lost_components_extents_and_authoritative_projection(self):
        cases = load_json(ROOT / "conformance/states/outline-focus-cases.json")
        for name, corrupt in [
            ("tiny direction component precision probe", lambda result: result["supports"][0]["normal"].update(y=0)),
            ("large projection precision probe", lambda result: result["supports"][0]["inner"].update(
                support=result["supports"][0]["silhouette"]["support"])),
            ("large projection precision probe", lambda result: result["focus"]["geometry"].update(
                innerBounds=copy.deepcopy(result["focus"]["geometry"]["silhouetteBounds"]))),
            ("tiny positive projection precision probe", lambda result: result["focus"]["geometry"]["sideOffset"].update(x=0)),
        ]:
            case = next(case for case in cases if case["name"] == name)
            request, outline, actual = precise_probe(case)
            check_geometry(actual, request, outline)
            corrupt(actual)
            with self.subTest(case=name), self.assertRaises(AssertionError):
                check_geometry(actual, request, outline)

    def test_precise_geometry_parsing_keeps_strict_json_and_schema_checks(self):
        _, _, actual = fixture()
        source = precise_wire(actual)
        for invalid in (source.replace('"schemaVersion":"0.1.0"',
                                       '"schemaVersion":"0.1.0","schemaVersion":"0.1.0"', 1),
                        source.replace('"support":126', '"support":NaN', 1),
                        source.replace('"support":126', '"support":1e999', 1)):
            with self.subTest(source=invalid[:60]), self.assertRaises(ValueError):
                parse_result(invalid, "invalid precise result")
        actual["supports"][0]["normal"]["unknown"] = 1
        parsed = parse_result(precise_wire(actual), "unknown member")
        self.assertTrue(list(validator_for("schemas/outline-focus-result.schema.json").iter_errors(parsed)))

    def test_precise_large_projection_is_not_forced_to_share_reference_rejection(self):
        cases = load_json(ROOT / "conformance/states/outline-focus-cases.json")
        probe = next(case for case in cases if case["name"] == "cancelled projection requires certified accuracy")
        request, _, outline = case_request(probe)
        _, _, actual = fixture()
        geometry = actual["focus"]["geometry"]
        geometry["outline"] = outline
        geometry["sideOffset"] = request["sideOffset"]
        dx, dy = request["sideOffset"]["x"], request["sideOffset"]["y"]
        for field, outset in (("silhouetteBounds", 0), ("innerBounds", 2), ("outerBounds", 4)):
            geometry[field] = {"x": -outset, "y": dy - outset,
                               "width": 120 + dx + 2 * outset, "height": 60 - dy + 2 * outset}
        point = (dx + 120, dy + 60)
        query = {"normal": {"x": 0.6, "y": 0.8}}
        # Binary64 coordinates can compensate their lattice rounding while retaining exact support.
        for field, ix, iy, outset in (("silhouette", 0, 0, 0), ("inner", 2456, 3278, 2),
                                      ("outer", 4916, 6553, 4)):
            query[field] = {
                "point": {"x": point[0] + ix / 2048, "y": point[1] + iy / 2048},
                "support": 120.000390625 + outset,
            }
        actual["supports"] = [query]
        check_geometry(actual, request, outline)
        self.assertEqual(list(validator_for("schemas/outline-focus-result.schema.json").iter_errors(actual)), [])

    def test_accuracy_probe_allows_precise_success_or_atomic_repeatable_rejection(self):
        request, _, actual = fixture()
        probe = copy.deepcopy(load_json(ROOT / "conformance/states/outline-focus-cases.json")[0])
        probe["accuracyProbe"] = True
        probe["changes"] = [{"path": "/directions", "value": request["directions"]}]
        success = CompletedProcess([], 0, json.dumps(actual), "")
        rejection = CompletedProcess([], 1, "", "support precision cannot be certified")
        partial = CompletedProcess([], 1, "{}", "support precision cannot be certified")
        false_result = copy.deepcopy(actual)
        false_result["supports"][0]["inner"]["support"] = 126
        false_success = CompletedProcess([], 0, json.dumps(false_result), "")
        def documents(path):
            return [probe] if path.name == "outline-focus-cases.json" else load_json(path)
        for outputs, expected in [([success, success], 0), ([rejection], 0),
                                  ([partial], 1), ([success, rejection], 1), ([false_success], 1)]:
            with self.subTest(expected=expected, statuses=[item.returncode for item in outputs]):
                with patch("sys.argv", ["check", "--", "backend", "-"]), \
                     patch("check_outline_focus_backend.load_json", side_effect=documents), \
                     patch("check_outline_focus_backend.run_backend", side_effect=outputs), \
                     patch("check_outline_focus_backend.check_failure"), \
                     redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    self.assertEqual(main(), expected)

    def test_authored_cardinal_witnesses_and_schema(self):
        request, outline, actual = fixture()
        check_geometry(actual, request, outline)
        self.assertEqual(list(validator_for("schemas/outline-focus-result.schema.json").iter_errors(actual)), [])
        self.assertEqual(list(validator_for("schemas/outline-focus-ir.schema.json").iter_errors(actual["focus"])), [])

    def test_checker_rejects_offsets_projection_order_and_false_witnesses(self):
        request, outline, actual = fixture()
        corruptions = []
        changed = copy.deepcopy(actual)
        changed["focus"]["geometry"]["sideOffset"]["x"] = 0
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["geometry"]["outline"]["curvature"] = "circular"
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"][0]["inner"]["point"]["x"] += 2 * (2 ** 0.25 - 1)
        changed["supports"][0]["inner"]["support"] += 2 * (2 ** 0.25 - 1)
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"].reverse()
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"].pop()
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"][0]["inner"]["point"]["y"] += 1
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"][0]["inner"]["support"] = 126
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["supports"][0]["inner"]["support"] = math.nan
        corruptions.append(changed)
        for index, changed in enumerate(corruptions):
            with self.subTest(corruption=index), self.assertRaises(AssertionError):
                check_geometry(changed, request, outline)

    def test_result_schema_rejects_incomplete_and_invalid_geometry(self):
        _, _, actual = fixture()
        validator = validator_for("schemas/outline-focus-result.schema.json")
        corruptions = []
        changed = copy.deepcopy(actual)
        del changed["focus"]["geometry"]["outline"]
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["geometry"]["unknown"] = True
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["geometry"]["outerBounds"]["width"] = 0
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["geometry"]["outerOutset"] = 5
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["indicator"]["strokeWidth"] = 1
        corruptions.append(changed)
        changed = copy.deepcopy(actual)
        changed["focus"]["indicator"]["color"]["alpha"] = 0.5
        corruptions.append(changed)
        for index, changed in enumerate(corruptions):
            with self.subTest(corruption=index):
                self.assertTrue(list(validator.iter_errors(changed)))


if __name__ == "__main__":
    unittest.main()
