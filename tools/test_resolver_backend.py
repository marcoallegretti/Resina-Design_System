import json
import unittest

from check_resolver_backend import cases
from check_schemas import ROOT, load_json


class ResolverBackendCheckerTests(unittest.TestCase):
    def test_public_vectors_foundation_and_request_failures_are_covered(self):
        loaded = list(cases())
        public = load_json(ROOT / "conformance/tokens/resolver-module-vectors.json")
        self.assertEqual(len(loaded), len(public) + 5)
        self.assertEqual(len({name for name, _, _ in loaded}), len(loaded))
        self.assertTrue(
            any(
                name == "foundation: authored spatial, type, and radius scales" and len(expected) == 24
                for name, _, expected in loaded
            )
        )
        self.assertTrue(
            any(
                name == "request: duplicate envelope member" and expected is None
                for name, _, expected in loaded
            )
        )

    def test_composition_request_preserves_raw_duplicate_source_text(self):
        source = next(
            source
            for name, source, _ in cases()
            if name == "duplicate resolver member is rejected before composition"
        )
        request = json.loads(source)
        self.assertEqual(request["schemaVersion"], "0.1.0")
        self.assertIn('"version":"2025.10","version":"2025.10"', request["resolver"])


if __name__ == "__main__":
    unittest.main()
