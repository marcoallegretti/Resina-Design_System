import json
from pathlib import Path
import unittest

from command_label_arithmetic import check_cases, extent_remainder, extent_sum
from check_schemas import validator_for


ROOT = Path(__file__).resolve().parents[1]


class ConservativeLabelArithmeticTests(unittest.TestCase):
    def fixture(self):
        return json.loads((ROOT / "conformance/content/command-label-arithmetic.json").read_text())

    def test_public_cases_against_exact_rational_oracle(self):
        document = self.fixture()
        validator_for("schemas/command-label-arithmetic.schema.json").validate(document)
        check_cases(document)
        self.assertEqual(extent_sum(1.1, .2), 1.3000000000000003)
        self.assertEqual(extent_remainder(1.1, .2), .9)
        self.assertEqual(extent_sum(1, 1), 2)

    def test_nearest_rounding_and_oversized_offers_are_rejected(self):
        for index, member, key, value in ((0, "size", "height", 89.54251199999999),
                                          (2, "labelBounds", "width", .9000000000000001)):
            document = self.fixture()
            document["cases"][index]["expected"][member][key] = value
            with self.assertRaises(ValueError):
                check_cases(document)
        for left, right in ((1e100, 1), (1e308, 1e308)):
            with self.assertRaises((ValueError, OverflowError)):
                extent_sum(left, right)
        with self.assertRaises(ValueError):
            extent_remainder(1e100, 1)

    def test_fixture_schema_rejects_ambiguous_or_leaking_records(self):
        validator = validator_for("schemas/command-label-arithmetic.schema.json")
        for change in ("both", "unknown", "negative", "missing"):
            document = self.fixture()
            case = document["cases"][0]
            if change == "both":
                case["error"] = True
            elif change == "unknown":
                case["backend"] = "native"
            elif change == "negative":
                case["padding"]["start"] = -1
            else:
                del case["fitted"]
            self.assertFalse(validator.is_valid(document), change)


if __name__ == "__main__":
    unittest.main()
