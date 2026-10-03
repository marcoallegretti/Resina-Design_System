import copy
import unittest

from check_schemas import validator_for
from check_surface_paint_backend import baseline, expected_result, surface_paint_mismatch


class SurfacePaintConformanceTests(unittest.TestCase):
    def focused(self):
        request = baseline()
        request["body"]["surface"]["states"]["states"] = ["focused"]
        return expected_result(request)

    def test_schema_requires_exactly_the_navigation_channel_in_use(self):
        validator = validator_for("schemas/surface-paint-ir.schema.json")
        focused = self.focused()
        self.assertFalse(list(validator.iter_errors(focused)))
        del focused["focus"]
        self.assertTrue(list(validator.iter_errors(focused)))
        rest = expected_result(baseline())
        self.assertFalse(list(validator.iter_errors(rest)))
        rest["focus"] = self.focused()["focus"]
        self.assertTrue(list(validator.iter_errors(rest)))

    def test_matching_reference_cannot_hide_cross_channel_state_or_shape_errors(self):
        for mutate in (
            lambda value: value["focus"]["indicator"]["binding"]["states"].update(states=["rest", "focused"]),
            lambda value: value["focus"]["geometry"]["silhouette"]["bounds"].update(width=21),
        ):
            result = self.focused()
            mutate(result)
            self.assertIsNotNone(surface_paint_mismatch(result, copy.deepcopy(result)))

    def test_incomplete_or_extra_channels_cannot_pass_comparison(self):
        expected = self.focused()
        actual = copy.deepcopy(expected)
        del actual["focus"]
        self.assertIsNotNone(surface_paint_mismatch(actual, expected))
        actual = copy.deepcopy(expected)
        actual["renderer"] = "unknown"
        self.assertIsNotNone(surface_paint_mismatch(actual, expected))


if __name__ == "__main__":
    unittest.main()
