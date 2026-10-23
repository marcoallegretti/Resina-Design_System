import copy
import unittest

from check_schemas import load_json
from lit_control_visibility import SIZES, STEP, TABLE, check, geometries, inputs, response_at


class LitControlVisibilityTests(unittest.TestCase):
    def table(self):
        return copy.deepcopy(load_json(TABLE))

    def test_published_table_is_reproduced_by_the_independent_arithmetic(self):
        self.assertEqual(check(self.table()), 36)

    def test_every_altered_member_is_rejected(self):
        for path, change in (
                (("parts", "commandBody", "highlight", "response"), 0.01),
                (("parts", "toggleTrack", "shading", "point", "x"), 0.25),
                (("rows", 0, "body", 1), 1e-6),
                (("rows", 5, "highlightDeltaE"), 1e-6),
                (("rows", 17, "shadingDeltaE"), -1e-6),
                (("sampleStep",), 0.5)):
            table = self.table()
            container = table
            for key in path[:-1]:
                container = container[key]
            container[path[-1]] += change
            with self.subTest(path=path), self.assertRaises(ValueError):
                check(table)

    def test_a_removed_row_is_rejected(self):
        table = self.table()
        del table["rows"][-1]
        with self.assertRaises(ValueError):
            check(table)

    def test_witnesses_are_the_peaks_of_a_finer_grid(self):
        """No point within one grid step of a witness exceeds its response."""
        table = self.table()
        bodies = geometries(*inputs())
        for part, geometry in bodies.items():
            for name, sign in (("highlight", 1), ("shading", -1)):
                witness = table["parts"][part][name]
                best = 0.0
                for column in range(-10, 11):
                    for row in range(-10, 11):
                        point = (witness["point"]["x"] + column * STEP / 10, witness["point"]["y"] + row * STEP / 10)
                        if 0 <= point[0] <= SIZES[part]["width"] and 0 <= point[1] <= SIZES[part]["height"]:
                            best = max(best, sign * response_at(geometry, point))
                with self.subTest(part=part, witness=name):
                    self.assertLess(best - sign * witness["response"], 1e-9)


if __name__ == "__main__":
    unittest.main()
