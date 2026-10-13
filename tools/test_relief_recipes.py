import copy
import unittest

from check_schemas import ROOT, load_json, validator_for


class ReliefRecipeTests(unittest.TestCase):
    def setUp(self):
        self.recipes = load_json(ROOT / "lab/guido/relief-recipes.json")
        self.validator = validator_for("lab/guido/schemas/relief-recipes.schema.json")

    def test_authored_recipes_cover_every_shape_and_profile_without_role_substitution(self):
        self.assertEqual(list(self.validator.iter_errors(self.recipes)), [])
        names = [recipe["name"] for recipe in self.recipes["recipes"]]
        self.assertEqual(len(names), len(set(names)))
        combinations = {(recipe["profile"]["kind"], recipe["shape"]) for recipe in self.recipes["recipes"]}
        self.assertEqual(combinations, {(kind, shape) for kind in ("flat", "convex", "recessed")
                                      for shape in ("structural", "soft", "rounded", "capsule", "organic")})
        for recipe in self.recipes["recipes"]:
            self.assertEqual(recipe["name"], f"{recipe['profile']['kind']}-{recipe['shape']}")
            self.assertEqual(recipe["expectedMaterialFamily"], "elastomer" if recipe["profile"]["kind"] == "convex" else "cast")
            profile = recipe["profile"]
            self.assertLessEqual(profile["amplitude"], profile["shoulderWidth"] / 2)
            self.assertLess(profile["amplitude"], profile["bodyThickness"])

    def test_schema_rejects_unsupported_and_incomplete_recipe_contracts(self):
        for mutate in (
            lambda d: d.update(schemaVersion="0.2.0"),
            lambda d: d.update(edgeWidth=0),
            lambda d: d["sizes"]["convex"].update(height=0),
            lambda d: d.update(backend="guido"),
            lambda d: d["recipes"][0].update(curvature="elliptical"),
            lambda d: d["recipes"][0].update(materialRole="surface.unknown"),
            lambda d: d["recipes"][0]["profile"].update(kind="negativeExtrusion"),
            lambda d: d["recipes"][0]["profile"].update(amplitude=1),
            lambda d: d["recipes"][0].pop("foregroundRole"),
        ):
            document = copy.deepcopy(self.recipes)
            mutate(document)
            self.assertTrue(list(self.validator.iter_errors(document)))


if __name__ == "__main__":
    unittest.main()
