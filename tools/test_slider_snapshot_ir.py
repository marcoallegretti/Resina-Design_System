import copy
import itertools
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from backend_source import duplicate_member_source, nonfinite_member_source
from check_extruded_contour_backend import contour_mismatch
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for
from check_slider_snapshot_ir import check_capture, check_snapshot, fixture_expected


class SliderSnapshotIrTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.records = []
        for family, direction, orientation, pose, scale in itertools.product(
                ("cast", "frost", "elastomer"), ("ltr", "rtl"), ("horizontal", "vertical"),
                ("focused", "rest", "disabled", "disabledFocused", "readOnly", "preview"), (1, 2)):
            config = dict(family=family, direction=direction, orientation=orientation, pose=pose, textScale=scale)
            cls.records.append(dict(config=config, snapshot=fixture_expected(config)))

    def setUp(self):
        self.base = load_json(ROOT / "conformance/ir/slider-snapshot-ir.json")
        self.validator = validator_for("schemas/slider-snapshot-ir.schema.json")

    def test_authored_record_matches_independent_complete_arithmetic(self):
        expected = fixture_expected(dict(family="cast", direction="ltr", orientation="horizontal",
                                         pose="focused", textScale=1))
        self.assertIsNone(contour_mismatch(self.base, expected))
        check_snapshot(self.base)

    def test_public_schema_and_coherence_cases(self):
        names = set()
        for case in load_json(ROOT / "conformance/ir/slider-snapshot-ir-cases.json"):
            with self.subTest(name=case["name"]):
                self.assertNotIn(case["name"], names)
                names.add(case["name"])
                changed = apply_changes(self.base, case["changes"])
                check_case(self.validator, case["name"], changed, case["schemaValid"])
                if case["coherent"]:
                    check_snapshot(changed)
                else:
                    with self.assertRaises((AssertionError, ValueError)):
                        check_snapshot(changed)

    def test_fixture_matrix_preserves_coherence_in_every_pose(self):
        check_capture(self.records)

    def test_capture_requires_every_unique_configuration(self):
        for records in ([], self.records[:-1], self.records + [self.records[0]]):
            with self.subTest(length=len(records)):
                with self.assertRaisesRegex(ValueError, "incomplete or duplicate"):
                    check_capture(records)
        changed = copy.deepcopy(self.records)
        changed[0]["config"]["textScale"] = True
        with self.assertRaisesRegex(ValueError, "invalid member types"):
            check_capture(changed)
        changed = copy.deepcopy(self.records)
        changed[0]["backend"] = "unknown"
        with self.assertRaisesRegex(ValueError, "complete named test records"):
            check_capture(changed)

    def test_keyboard_contact_requires_actual_focus_without_pointer_editing(self):
        record = next(r["snapshot"] for r in self.records if r["config"]["pose"] == "rest")
        changes = []
        for part in ("track", "thumb"):
            changes.extend((dict(path=f"/{part}/paint/body/states/states", value=["pressed"]),
                            dict(path=f"/{part}/phase", value="pressed")))
        changed = apply_changes(record, changes)
        check_case(self.validator, "unfocused keyboard contact", changed, True)
        with self.assertRaisesRegex(ValueError, "keyboard contact feedback requires actual focus"):
            check_snapshot(changed)

    def test_full_capture_comparison_rejects_schema_valid_channel_mutations(self):
        for path, value, pose in (("/track/paint/body/pigment/body/components/0", 0.03, "focused"),
                                  ("/thumb/paint/focus/indicator/color/components/0", 0.4, "focused"),
                                  ("/track/response/depthScale", 0.9, "preview"),
                                  ("/accessibility/valueText", "Wrong value text", "focused"),
                                  ("/reducedMotion", False, "focused")):
            with self.subTest(path=path):
                index = next(i for i, r in enumerate(self.records) if r["config"]["pose"] == pose)
                record = copy.deepcopy(self.records[index])
                record["snapshot"] = apply_changes(record["snapshot"], [dict(path=path, value=value)])
                check_case(self.validator, path, record["snapshot"], True)
                check_snapshot(record["snapshot"])
                with self.assertRaisesRegex(ValueError, "snapshot capture differs"):
                    check_capture([record] + self.records[:index] + self.records[index + 1:])

    def test_cli_checks_real_files_and_strict_decoded_source(self):
        sources = ((json.dumps(self.base), 0),
                   (duplicate_member_source(self.base, "/labelOrigin/x"), 1),
                   (nonfinite_member_source(self.base, "/labelOrigin/x"), 1),
                   (json.dumps(self.base).replace('"x": 190', '"x": 1e999', 1), 1))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "snapshot.json"
            for source, status in sources:
                with self.subTest(status=status, source=source[:50]):
                    path.write_text(source, encoding="utf-8")
                    result = subprocess.run([sys.executable, str(ROOT / "tools/check_slider_snapshot_ir.py"),
                                             str(path)], capture_output=True, text=True, timeout=30)
                    self.assertEqual(result.returncode, status, result.stderr)
                    if status:
                        self.assertIn("FAIL Slider snapshot IR", result.stderr)
                        self.assertNotIn("verification passed", result.stdout)

    def test_missing_members_and_backend_leakage_fail_schema(self):
        for field in self.base:
            with self.subTest(field=field):
                changed = copy.deepcopy(self.base)
                del changed[field]
                check_case(self.validator, field, changed, False)
        for field in ("renderer", "nativeHandle", "product", "pointerCapture"):
            with self.subTest(field=field):
                check_case(self.validator, field, dict(self.base, **{field: "opaque"}), False)


if __name__ == "__main__":
    unittest.main()
