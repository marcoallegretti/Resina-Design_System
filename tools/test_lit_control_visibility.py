import copy
import unittest

from check_schemas import load_json
from lit_body_arithmetic import Body
from lit_control_visibility import SIZES, STEP, TABLE, bodies, check, delta, geometries, inputs, response_at, source


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

    def test_peak_difference_is_the_brute_force_maximum_of_painted_colors(self):
        """The larger of a row's two deltas is the greatest difference painted anywhere on the face."""
        table = self.table()
        appearance, foundation, relief, environment = inputs()
        shapes = {(part, variant): shape for part, variant, _, _, _, _, shape in bodies()}
        chosen = [(0, "light", "elastomer", "commandBody", "standard"), (1, "dark", "frost", "toggleTrack", "track"),
                  (2, "dark", "cast", "toggleThumb", "thumb"), (3, "light", "cast", "commandBody", "primary"),
                  (4, "light", "frost", "toggleThumb", "thumb")]
        for _, theme, family, part, variant in chosen:
            rows = [row for row in table["rows"] if (row["theme"], row["materialFamily"], row["part"], row["variant"]) == (theme, family, part, variant)]
            for row in rows:
                body = Body(source(part, shapes[(part, variant)], family, row["body"], appearance, foundation, relief, environment))
                brute = 0.0
                for column in range(int(SIZES[part]["width"] / 0.5) + 1):
                    for line in range(int(SIZES[part]["height"] / 0.5) + 1):
                        point = (column * 0.5, line * 0.5)
                        if body.contains(point, body.edge_width):
                            brute = max(brute, delta(row["body"], body.face_color(point)["components"]))
                published = max(row["highlightDeltaE"], row["shadingDeltaE"])
                with self.subTest(row=row):
                    self.assertLessEqual(brute, published + 1e-9)
                    self.assertGreater(brute, published * 0.99)


if __name__ == "__main__":
    unittest.main()
