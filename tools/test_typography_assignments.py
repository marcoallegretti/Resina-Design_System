import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_theme_backend import cases as theme_cases, theme_source


PREFIX = "typography source shape "


class TypographyAssignmentBoundaryTests(unittest.TestCase):
    def test_model_vectors_require_objects_and_preserve_roles(self):
        vectors = load_json(ROOT / "conformance/typography/assignment-vectors.json")
        validator = validator_for("schemas/typography-assignments.schema.json")
        invalid = [v for v in vectors if v["name"].startswith(PREFIX)]
        self.assertEqual(len(invalid), 82)
        for vector in vectors:
            with self.subTest(name=vector["name"]):
                self.assertEqual(validator.is_valid(vector["document"]), "expected" in vector)
                if "expected" in vector:
                    self.assertEqual(vector["document"]["roles"], vector["expected"])
        original = vectors[0]["document"]
        for group in ["root", "roles"]:
            shapes = [v for v in invalid if f"{group} " in v["name"]]
            self.assertEqual(len(shapes), 6)
            positional = next(v for v in shapes if v["name"].endswith("positional"))
            expected = [original["schemaVersion"], original["roles"]] if group == "root" else \
                {**original, "roles": [list(item) for item in original["roles"].items()]}
            self.assertEqual(positional["document"], expected)
        roles = set(original["roles"])
        role_shapes = [v for v in invalid if " shape role " in v["name"]]
        family_shapes = [v for v in invalid if " shape family " in v["name"]]
        self.assertEqual(len(role_shapes), 60)
        self.assertEqual(len(family_shapes), 10)
        self.assertEqual({v["name"].removeprefix(PREFIX).split()[1] for v in role_shapes}, roles)
        self.assertEqual({v["name"].removeprefix(PREFIX).split()[1] for v in family_shapes}, roles)
        self.assertEqual({original["roles"][role]["familyRole"] for role in roles},
                         {"sans", "display", "monospace", "numeric"})
        for vector in invalid:
            pointer = self.assignment_pointer(vector["name"]).removeprefix("/typographyAssignments")
            value = vector["document"]
            for part in pointer.split("/")[1:]:
                value = value[part]
            expected = apply_changes(original, [{"path": pointer, "value": value}]) if pointer else value
            self.assertEqual(vector["document"], expected)
            self.check_shape(vector["name"], value, original)

    def test_public_cases_change_only_typography_shapes(self):
        vectors = {v["name"] for v in load_json(ROOT / "conformance/typography/assignment-vectors.json")
                   if v["name"].startswith(PREFIX)}
        baseline = load_json(ROOT / "conformance/headless/valid-request.json")
        surface = load_json(ROOT / "conformance/surfaces/binding-vectors.json")[0]["document"]
        for path, schema, original, prefix in [
            ("conformance/headless/backend-cases.json", "schemas/headless-resolution.schema.json", baseline, ""),
            ("conformance/surfaces/scenario-cases.json", "schemas/surface-scenario.schema.json",
             {"schemaVersion": "0.4.0", "resolution": baseline, "surface": surface}, "/resolution"),
        ]:
            cases = [c for c in load_json(ROOT / path) if c["name"].startswith(PREFIX)]
            self.assertEqual(len(cases), 82)
            self.assertEqual({c["name"] for c in cases}, vectors)
            validator = validator_for(schema)
            self.assertTrue(validator.is_valid(original))
            for case in cases:
                with self.subTest(path=path, name=case["name"]):
                    self.assertEqual(case["outcome"], "invalid")
                    self.assertEqual(len(case["requestChanges"]), 1)
                    change = case["requestChanges"][0]
                    self.assertEqual(change["path"], prefix + self.assignment_pointer(case["name"]))
                    request = apply_changes(original, case["requestChanges"])
                    self.assertFalse(validator.is_valid(request))
                    restored = apply_changes(request, [{"path": prefix + "/typographyAssignments",
                                                        "value": baseline["typographyAssignments"]}])
                    self.assertEqual(restored, original)
                    self.check_shape(case["name"], change["value"], baseline["typographyAssignments"])
        public = {c["name"]: c for c in load_json(ROOT / "conformance/themes/resolution-cases.json")}
        themes = [(name, source, expected) for name, source, expected in theme_cases() if name.startswith(PREFIX)]
        self.assertEqual(len(themes), 124)
        self.assertEqual({name for name, _, _ in themes if name in vectors}, vectors)
        variants = set()
        base_text = (ROOT / "conformance/themes/valid-source.json").read_text(encoding="utf-8")
        foundation_text = (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")
        for name, source, expected in themes:
            with self.subTest(name=name):
                case = public[name]
                self.assertIsNone(expected)
                self.assertEqual(case["outcome"], "invalid")
                self.assertTrue(case["requestSchemaValid"])
                self.assertEqual(len(case["sourceChanges"]), 1)
                self.assertNotIn("requestChanges", case)
                self.assertNotIn("sourceReplace", case)
                change = case["sourceChanges"][0]
                self.assertEqual(change["path"], self.assignment_pointer(name))
                request = json.loads(source)
                self.assertTrue(validator_for("schemas/theme-resolution-request.schema.json").is_valid(request))
                theme = json.loads(request["themeSource"])
                validator = validator_for("schemas/theme-source.schema.json")
                self.assertFalse(validator.is_valid(theme))
                unchanged, sources = theme_source({k: v for k, v in case.items() if k != "sourceChanges"},
                                                 base_text, foundation_text)
                original = json.loads(unchanged)
                restored = apply_changes(theme, [{"path": "/typographyAssignments",
                                                  "value": original["typographyAssignments"]}])
                self.assertEqual(restored, original)
                self.assertEqual(request["externalSources"], sources)
                self.assertTrue(validator.is_valid(restored))
                self.check_shape(name, change["value"], original["typographyAssignments"])
                variants.add(case.get("sourceVariant", "embedded"))
        self.assertEqual(variants, {"embedded", "resolverBackedInline", "foundationResolver"})

    def assignment_pointer(self, name):
        parts = name.removeprefix(PREFIX).split()
        if parts[0] == "root":
            return "/typographyAssignments"
        if parts[0] == "roles":
            return "/typographyAssignments/roles"
        path = "/typographyAssignments/roles/" + parts[1]
        return path + "/familyRole" if parts[0] == "family" else path

    def check_shape(self, name, value, baseline):
        parts = name.removeprefix(PREFIX).split()
        if parts[0] == "family":
            self.assertEqual(value, {baseline["roles"][parts[1]]["familyRole"]: None})
        elif "positional" in name:
            if parts[0] == "root":
                expected = [baseline["schemaVersion"], baseline["roles"]]
            elif parts[0] == "roles":
                expected = [list(item) for item in baseline["roles"].items()]
            else:
                fields = load_json(ROOT / "schemas/typography-assignments.schema.json")["$defs"]["role"]["required"]
                expected = [baseline["roles"][parts[1]][field] for field in fields]
            self.assertEqual(value, expected)
        if parts[0] != "family":
            self.assertNotIsInstance(value, dict)


if __name__ == "__main__":
    unittest.main()
