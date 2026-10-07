import json
import unittest
from unittest.mock import patch

from check_theme_backend import cases
from check_schemas import ROOT, load_json, validator_for


class ThemeBackendCheckerTests(unittest.TestCase):
    def test_public_cases_cover_all_token_authoring_forms(self):
        loaded = list(cases())
        public = load_json(ROOT / "conformance/themes/resolution-cases.json")
        vectors = load_json(ROOT / "conformance/themes/request-vectors.json")
        self.assertEqual(len(loaded), len(public) + len(vectors))
        self.assertEqual(len({name for name, _, _ in loaded}), len(loaded))
        foundation = next(
            (json.loads(source), expected)
            for name, source, expected in loaded
            if name == "external foundation supplies authored type and space scales"
        )
        request, expected = foundation
        self.assertIn("foundation.json", request["externalSources"])
        self.assertEqual(expected["typography"]["body"]["fontSize"]["value"], 44)
        baseline = load_json(ROOT / "conformance/headless/expected-resolution.json")
        self.assertEqual(expected["space"], baseline["space"])

    def test_duplicate_theme_member_remains_in_raw_source_text(self):
        request = next(
            json.loads(source)
            for name, source, _ in cases()
            if name == "duplicate member in authored theme is rejected"
        )
        self.assertIn(
            '"schemaVersion": "0.1.0",\n  "schemaVersion": "0.1.0"',
            request["themeSource"],
        )

    def test_root_vectors_are_schema_invalid_with_valid_positional_payloads(self):
        vectors = load_json(ROOT / "conformance/themes/request-vectors.json")
        self.assertEqual(len(vectors), 7)
        validator = validator_for("schemas/theme-resolution-request.schema.json")
        loaded = {name: (json.loads(source), expected) for name, source, expected in cases()}
        for vector in vectors:
            self.assertFalse(validator.is_valid(vector["document"]), vector["name"])
            self.assertEqual(loaded[vector["name"]], (vector["document"], None))
        for vector in vectors[:2]:
            version, source, external, environment = vector["document"]
            self.assertIn(version, ("0.1.0", "0.2.0"))
            request = {
                "schemaVersion": "0.1.0",
                "themeSource": source,
                "externalSources": external,
                "environment": environment,
            }
            self.assertTrue(validator.is_valid(request))
            self.assertEqual(
                source,
                (ROOT / "conformance/themes/valid-source.json").read_text(encoding="utf-8"),
            )
            self.assertEqual(
                environment,
                load_json(ROOT / "conformance/headless/valid-request.json")["environment"],
            )

    def test_checker_rejects_invalid_vector_metadata_and_valid_rejection_inputs(self):
        for vector in (
            {"name": "missing error", "document": []},
            {"name": "empty error", "document": [], "error": ""},
            {
                "name": "embedded theme matches headless semantics",
                "document": [],
                "error": "rejected",
            },
            {
                "name": "valid rejection",
                "document": json.loads(next(cases())[1]),
                "error": "rejected",
            },
        ):
            def load(path):
                if path.name == "request-vectors.json":
                    return [vector]
                return load_json(path)

            with self.subTest(vector=vector), patch(
                "check_theme_backend.load_json", side_effect=load
            ):
                with self.assertRaises(ValueError):
                    list(cases())


if __name__ == "__main__":
    unittest.main()
