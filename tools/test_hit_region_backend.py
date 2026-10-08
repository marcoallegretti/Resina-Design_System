import copy
import unittest

from check_hit_region_backend import baseline, cases, hit_region_mismatch, validate_membership_vectors
from check_schemas import ROOT, load_json, validator_for


class HitRegionConformanceTests(unittest.TestCase):
    def test_subminimum_or_shifted_region_cannot_pass_with_tolerance(self):
        expected = cases()[0]["expected"]
        for field in ("x", "y", "width", "height"):
            actual = copy.deepcopy(expected)
            actual["bounds"][field] -= 1e-10
            self.assertIsNotNone(hit_region_mismatch(actual, expected))
        self.assertIsNone(hit_region_mismatch(copy.deepcopy(expected), expected))

    def test_case_requests_do_not_mutate_the_baseline(self):
        first = cases()
        first[0]["request"]["visualBounds"]["width"] = 100
        self.assertEqual(cases()[0]["request"]["visualBounds"]["width"], 20)

    def test_positional_geometry_records_fail_the_production_schema(self):
        shapes = [case for case in cases()
                  if case["name"].startswith(("positional ", "later positional "))]
        self.assertEqual(len(shapes), 5)
        request_validator = validator_for("schemas/hit-region-request.schema.json")
        for case in shapes:
            with self.subTest(name=case["name"]):
                self.assertFalse(request_validator.is_valid(case["request"]))
                self.assertFalse(case["requestSchemaValid"])
                self.assertTrue(case["failure"])
        base = baseline()
        root = [base[field] for field in (
            "schemaVersion", "environment", "visualBounds", "availableBounds",
            "componentMinimum", "occupiedRegions",
        )]
        self.assertFalse(request_validator.is_valid(root))

    def test_invalid_environment_shapes_reach_schema_and_backend_evidence(self):
        vectors = load_json(ROOT / "conformance/environment/source-shape-vectors.json")
        self.assertEqual(len(vectors), 11)
        shapes = [case for case in cases() if case["name"].startswith("invalid environment shape:")]
        self.assertEqual(len(shapes), len(vectors))
        request_validator = validator_for("schemas/hit-region-request.schema.json")
        case_validator = validator_for("schemas/hit-region-case.schema.json")
        for case in shapes:
            with self.subTest(name=case["name"]):
                self.assertTrue(case_validator.is_valid({key: value for key, value in case.items()
                                                        if key != "request"}))
                self.assertFalse(request_validator.is_valid(case["request"]))
                self.assertFalse(case["requestSchemaValid"])
                self.assertTrue(case["failure"])

    def test_membership_expectations_use_exact_not_rounded_endpoints(self):
        vectors = load_json(ROOT / "conformance/interaction/hit-membership-vectors.json")
        self.assertEqual(validate_membership_vectors(vectors), 36)
        for vector in vectors:
            for point in vector["points"]:
                bounds = vector["bounds"]
                rounded = (bounds["x"] <= point["x"] < bounds["x"] + bounds["width"]
                           and bounds["y"] <= point["y"] < bounds["y"] + bounds["height"])
                if rounded != point["expected"]:
                    changed = copy.deepcopy(vector)
                    changed["points"] = [{**point, "expected": rounded}]
                    with self.assertRaisesRegex(ValueError, "exact arithmetic"):
                        validate_membership_vectors([changed])
                    return
        self.fail("oracle must cover a case rounded endpoint predicates misclassify")


if __name__ == "__main__":
    unittest.main()
