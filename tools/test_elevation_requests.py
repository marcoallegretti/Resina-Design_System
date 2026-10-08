import copy
import unittest

from check_schemas import ROOT, load_json, validator_for


class ElevationRequestBoundaryTests(unittest.TestCase):
    def test_malformed_requests_preserve_otherwise_valid_payloads(self):
        cases = load_json(ROOT / "conformance/elevation/backend-cases.json")
        original = cases[0]["request"]
        expected = {}
        for name, value in [
            ("positional", [original["schemaVersion"], original["tokens"], original["assignments"]]),
            ("unsupported positional version", ["9", original["tokens"], original["assignments"]]),
            ("null", None), ("boolean", True), ("number", 1),
            ("string", "request"), ("empty array", []),
        ]:
            expected["elevation request shape " + name] = value
        for name, value in [("null", None), ("boolean", True), ("number", 1),
                            ("array", []), ("object", {}), ("empty string", "")]:
            expected["elevation request version " + name] = {**original, "schemaVersion": value}
        for field in ["schemaVersion", "tokens", "assignments"]:
            expected["elevation request missing " + field] = {k: v for k, v in original.items() if k != field}
        negatives = [c for c in cases if c["name"].startswith("elevation request ")]
        self.assertEqual(len(negatives), 16)
        self.assertEqual({c["name"] for c in negatives}, expected.keys())
        case_validator = validator_for("schemas/elevation-depth-case.schema.json")
        request_validator = validator_for("schemas/elevation-depth-request.schema.json")
        self.assertTrue(request_validator.is_valid(original))
        for case in negatives:
            with self.subTest(name=case["name"]):
                self.assertEqual(case["request"], expected[case["name"]])
                self.assertTrue(case_validator.is_valid(case))
                self.assertFalse(case["requestSchemaValid"])
                self.assertFalse(request_validator.is_valid(case["request"]))
                self.assertNotIn("expected", case)
                self.assertTrue(case["errorContains"])
                if case["name"].startswith("elevation request version "):
                    restored = {**case["request"], "schemaVersion": original["schemaVersion"]}
                    self.assertEqual(restored, original)
                elif case["name"].startswith("elevation request missing "):
                    field = case["name"].removeprefix("elevation request missing ")
                    self.assertEqual({**case["request"], field: original[field]}, original)

    def test_case_schema_keeps_invalid_forms_in_failure_branch(self):
        case = load_json(ROOT / "conformance/elevation/backend-cases.json")[0]
        validator = validator_for("schemas/elevation-depth-case.schema.json")
        self.assertTrue(validator.is_valid(case))
        for change in [{"requestSchemaValid": False}, {"errorContains": "invalid request"}]:
            self.assertFalse(validator.is_valid({**case, **change}))
        missing = copy.deepcopy(case)
        del missing["requestSchemaValid"]
        self.assertFalse(validator.is_valid(missing))
        negative = {"name": "malformed root", "request": None,
                    "requestSchemaValid": False, "errorContains": "invalid request"}
        self.assertTrue(validator.is_valid(negative))
        del negative["errorContains"]
        self.assertFalse(validator.is_valid(negative))


if __name__ == "__main__":
    unittest.main()
