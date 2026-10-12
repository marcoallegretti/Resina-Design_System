import copy
import io
import json
import math
import unittest
from contextlib import redirect_stderr, redirect_stdout
from subprocess import CompletedProcess
from unittest.mock import patch

from check_focus_indicator_backend import contrast
from check_outline_focus_backend import case_request, check_geometry, main
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


class OutlineFocusCheckerTests(unittest.TestCase):
    def test_precise_large_projection_is_not_forced_to_share_reference_rejection(self):
        cases = load_json(ROOT / "conformance/states/outline-focus-cases.json")
        probe = next(case for case in cases if case.get("accuracyProbe"))
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
