import json
import unittest

from check_headless_backend import mismatch, replace_at_pointer
from check_schemas import ROOT, load_json


class HeadlessBackendCheckerTests(unittest.TestCase):
    def test_json_pointer_escapes_and_array_indices(self):
        document = {"a/b": [{"m~n": 1}, 2]}
        replace_at_pointer(document, "/a~1b/0/m~0n", 3)
        replace_at_pointer(document, "/a~1b/1", 4)
        self.assertEqual(document, {"a/b": [{"m~n": 3}, 4]})

    def test_change_cannot_create_or_extend_values(self):
        for pointer in ("/missing", "/items/2", "/items/01", "/items/-", "/bad~2escape"):
            with self.subTest(pointer=pointer):
                with self.assertRaises(ValueError):
                    replace_at_pointer({"items": [1, 2]}, pointer, 3)

    def test_output_comparison_checks_structure_and_numeric_tolerance(self):
        expected = {"role": {"components": [0.5, 1.0]}, "enabled": True}
        self.assertIsNone(mismatch(expected, expected))
        self.assertIsNone(
            mismatch({"role": {"components": [0.5 + 1e-13, 1]}, "enabled": True}, expected)
        )
        self.assertEqual(mismatch({"role": {"components": [0.5]}, "enabled": True}, expected), "/role/components")
        self.assertEqual(mismatch({"role": {"components": [0.5, 1]}, "enabled": 1}, expected), "/enabled")

    def test_duplicate_case_is_otherwise_a_valid_request(self):
        source = (ROOT / "conformance/headless/valid-request.json").read_text(encoding="utf-8")
        cases = load_json(ROOT / "conformance/headless/backend-cases.json")
        edit = next(case["sourceReplace"] for case in cases if case["name"] == "duplicate JSON member rejects the source")
        self.assertEqual(source.count(edit["find"]), 1)
        duplicate = source.replace(edit["find"], edit["with"], 1)
        self.assertEqual(json.loads(duplicate), json.loads(source))


if __name__ == "__main__":
    unittest.main()
