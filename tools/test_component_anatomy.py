import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for


class ComponentAnatomySourceTests(unittest.TestCase):
    def test_anatomy_discriminators_and_roles_require_strings(self):
        validator = validator_for("schemas/component-anatomy.schema.json")
        for component in ["command", "toggle"]:
            document = load_json(ROOT / f"definitions/components/{component}.json")
            pending = [("", document)]
            while pending:
                prefix, container = pending.pop()
                for name, original in container.items():
                    path = prefix + "/" + name
                    if isinstance(original, dict):
                        pending.append((path, original))
                    elif name != "schemaVersion":
                        for value in [{original: None}, [original], None, True, 1]:
                            with self.subTest(component=component, path=path, value=value):
                                invalid = apply_changes(document, [{"path": path, "value": value}])
                                self.assertFalse(validator.is_valid(invalid))

    def test_anatomy_roots_and_parts_require_objects(self):
        validator = validator_for("schemas/component-anatomy.schema.json")
        for component, paths in [
            ("command", ["", "/variants", "/variants/standard", "/variants/primary",
                         "/variants/standard/body", "/variants/primary/body",
                         "/variants/standard/label", "/variants/primary/label"]),
            ("toggle", ["", "/track", "/thumb", "/label"]),
        ]:
            document = load_json(ROOT / f"definitions/components/{component}.json")
            self.assertTrue(validator.is_valid(document))
            for path in paths:
                original = document
                for part in path.split("/")[1:]:
                    original = original[part]
                for value in [list(original.values()), None, True, 1, "part", []]:
                    with self.subTest(component=component, path=path, value=value):
                        invalid = (apply_changes(document, [{"path": path, "value": value}])
                                   if path else value)
                        self.assertFalse(validator.is_valid(invalid))


if __name__ == "__main__":
    unittest.main()
