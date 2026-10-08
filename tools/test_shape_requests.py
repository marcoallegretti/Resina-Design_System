import copy
import json
import unittest

from check_schemas import ROOT, load_json, validator_for


class ShapeRequestBoundaryTests(unittest.TestCase):
    def test_malformed_requests_preserve_otherwise_valid_payloads(self):
        cases = load_json(ROOT / "conformance/geometry/shape-fallback-source-cases.json")
        original = cases[0]["request"]
        self.assertEqual(original, {
            "schemaVersion": "0.1.0", "tokens": load_json(ROOT / "tokens/foundation.json"),
            "assignments": load_json(ROOT / "definitions/tier0-shapes.json"),
            "shape": "structural", "size": {"width": 200, "height": 80},
        })
        vector = next(v for v in load_json(ROOT / "conformance/geometry/shape-fallback-vectors.json")
                      if v["shape"] == original["shape"] and v["size"] == original["size"])
        self.assertEqual(cases[0]["expected"], {"schemaVersion": "0.1.0", "radii": vector["expected"]})
        expected = {}
        for name, value in [
            ("positional", [original["schemaVersion"], original["tokens"], original["assignments"], original["shape"], original["size"]]),
            ("unsupported positional version", ["9", original["tokens"], original["assignments"], original["shape"], original["size"]]),
            ("null", None), ("boolean", True), ("number", 1),
            ("string", "request"), ("empty array", []),
        ]:
            expected["shape request shape " + name] = value
        for name, value in [("null", None), ("boolean", True), ("number", 1),
                            ("array", []), ("object", {}), ("empty string", "")]:
            expected["shape request version " + name] = {**original, "schemaVersion": value}
        for field in ["schemaVersion", "tokens", "assignments", "shape", "size"]:
            expected["shape request missing " + field] = {k: v for k, v in original.items() if k != field}
        for version in ["9", "", "0.1.0 "]:
            expected["shape request version before tokens " + json.dumps(version)] = {**original, "schemaVersion": version, "tokens": None}
        negatives = [c for c in cases if c["name"].startswith("shape request ") and "expected" not in c]
        self.assertEqual(len(negatives), 21)
        self.assertEqual({c["name"] for c in negatives}, expected.keys())
        case_validator = validator_for("schemas/shape-fallback-source-case.schema.json")
        request_validator = validator_for("schemas/shape-fallback-request.schema.json")
        self.assertTrue(request_validator.is_valid(original))
        for case in negatives:
            with self.subTest(name=case["name"]):
                self.assertEqual(case["request"], expected[case["name"]])
                self.assertTrue(case_validator.is_valid(case))
                self.assertFalse(case["requestSchemaValid"])
                self.assertFalse(request_validator.is_valid(case["request"]))
                self.assertNotIn("expected", case)
                self.assertTrue(case["errorContains"])
                if case["name"].startswith("shape request version ") and "before tokens" not in case["name"]:
                    restored = {**case["request"], "schemaVersion": original["schemaVersion"]}
                    self.assertEqual(restored, original)
                elif case["name"].startswith("shape request missing "):
                    field = case["name"].removeprefix("shape request missing ")
                    self.assertEqual({**case["request"], field: original[field]}, original)

    def test_case_schema_keeps_invalid_forms_in_failure_branch(self):
        case = load_json(ROOT / "conformance/geometry/shape-fallback-source-cases.json")[0]
        validator = validator_for("schemas/shape-fallback-source-case.schema.json")
        self.assertTrue(validator.is_valid(case))
        for change in [{"requestSchemaValid": False}, {"errorContains": "invalid request"}]:
            self.assertFalse(validator.is_valid({**case, **change}))
        for malformed in [None, True, 1, "request", [], ["0.1.0"]]:
            self.assertFalse(validator.is_valid({**case, "request": malformed}))
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
