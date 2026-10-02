import json
import unittest

from check_theme_backend import cases
from check_schemas import ROOT, load_json


class ThemeBackendCheckerTests(unittest.TestCase):
    def test_public_cases_cover_all_token_authoring_forms(self):
        loaded = list(cases())
        public = load_json(ROOT / "conformance/themes/resolution-cases.json")
        self.assertEqual(len(loaded), len(public))
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


if __name__ == "__main__":
    unittest.main()
