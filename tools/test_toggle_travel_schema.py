import copy
import unittest
from check_schemas import ROOT, load_json, validator_for


class ToggleTravelSchemaTests(unittest.TestCase):
    def test_explicit_dynamics_state_and_binary_target_are_required(self):
        validator = validator_for("schemas/toggle-travel-cases.schema.json")
        baseline = load_json(ROOT / "conformance/motion/toggle-travel-cases.json")
        self.assertTrue(validator.is_valid(baseline))
        for field in baseline:
            document = copy.deepcopy(baseline)
            del document[field]
            self.assertFalse(validator.is_valid(document))
        for field in baseline["cases"][0]:
            document = copy.deepcopy(baseline)
            del document["cases"][0][field]
            self.assertFalse(validator.is_valid(document))
        for path, value in [(("checked",), "mixed"), (("time",), -1),
                            (("projection",), "hidden"), (("initial", "velocity"), None)]:
            document = copy.deepcopy(baseline)
            target = document["cases"][0]
            for part in path[:-1]:
                target = target[part]
            target[path[-1]] = value
            self.assertFalse(validator.is_valid(document))
        document = copy.deepcopy(baseline)
        document["dynamics"]["mass"] = 0
        self.assertFalse(validator.is_valid(document))
        document = copy.deepcopy(baseline)
        document["cases"][0]["backend"] = "guido"
        self.assertFalse(validator.is_valid(document))

    def test_projection_records_the_unbounded_scalar_position_exactly(self):
        validator = validator_for("schemas/toggle-travel-cases.schema.json")
        baseline = load_json(ROOT / "conformance/motion/toggle-travel-cases.json")
        for position, expected in [(-0.5, "offEndpoint"), (0, "none"), (0.5, "none"),
                                   (1, "none"), (1.5, "onEndpoint")]:
            for projection in ("none", "offEndpoint", "onEndpoint"):
                document = copy.deepcopy(baseline)
                document["cases"][0]["position"] = position
                document["cases"][0]["projection"] = projection
                with self.subTest(position=position, projection=projection):
                    self.assertEqual(validator.is_valid(document), projection == expected)


if __name__ == "__main__":
    unittest.main()
