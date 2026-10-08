import copy
import json
import unittest
from unittest.mock import patch

from check_schemas import ROOT, load_json, validator_for
from check_theme_backend import source_cases


FIELDS = (
    "schemaVersion",
    "tokens",
    "tokenResolver",
    "tokenInput",
    "materialAssignments",
    "frostPigment",
    "colorAssignments",
    "opaqueColorAssignments",
    "spatialAssignments",
    "typographyAssignments",
)


class ThemeSourceShapeTests(unittest.TestCase):
    def test_schema_invalid_sources_use_schema_valid_request_envelopes(self):
        vectors = load_json(ROOT / "conformance/themes/source-vectors.json")
        self.assertEqual(len(vectors), 9)
        source_validator = validator_for("schemas/theme-source.schema.json")
        request_validator = validator_for("schemas/theme-resolution-request.schema.json")
        loaded = {name: (json.loads(source), expected) for name, source, expected in source_cases()}
        self.assertEqual(len(loaded), len(vectors))
        for vector in vectors:
            request, expected = loaded[vector["name"]]
            self.assertIsNone(expected)
            self.assertTrue(request_validator.is_valid(request), vector["name"])
            self.assertEqual(json.loads(request["themeSource"]), vector["document"])
            self.assertFalse(source_validator.is_valid(vector["document"]), vector["name"])

    def test_positional_vectors_preserve_both_valid_authoring_payloads(self):
        baseline = load_json(ROOT / "conformance/themes/valid-source.json")
        resolver = copy.deepcopy(baseline)
        tokens = resolver.pop("tokens")
        resolver["tokenResolver"] = {
            "version": "2025.10",
            "resolutionOrder": [{"type": "set", "name": "theme", "sources": [tokens]}],
        }
        resolver["tokenInput"] = {}
        validator = validator_for("schemas/theme-source.schema.json")
        vectors = load_json(ROOT / "conformance/themes/source-vectors.json")
        for vector, wanted in zip(vectors[:4], (baseline, baseline, resolver, resolver)):
            document = vector["document"]
            self.assertEqual(len(document), len(FIELDS))
            self.assertIn(document[0], ("0.1.0", "0.2.0"))
            source = {key: value for key, value in zip(FIELDS, document) if value is not None}
            source["schemaVersion"] = "0.1.0"
            self.assertEqual(source, wanted)
            self.assertTrue(validator.is_valid(source))
        self.assertEqual(
            [vector["document"][0] for vector in vectors[:4]],
            ["0.1.0", "0.2.0", "0.1.0", "0.2.0"],
        )

    def test_checker_rejects_bad_metadata_duplicate_names_and_valid_negative_sources(self):
        valid = load_json(ROOT / "conformance/themes/valid-source.json")
        duplicate = {"name": "duplicate source", "document": [], "error": "rejected"}
        for vectors in (
            [{"name": "missing error", "document": []}],
            [{"name": "empty error", "document": [], "error": ""}],
            [{"name": "valid negative", "document": valid, "error": "rejected"}],
            [duplicate, duplicate],
        ):
            def load(path):
                if path.name == "source-vectors.json":
                    return vectors
                return load_json(path)

            with self.subTest(vectors=vectors), patch(
                "check_theme_backend.load_json", side_effect=load
            ):
                with self.assertRaises(ValueError):
                    list(source_cases())


if __name__ == "__main__":
    unittest.main()
