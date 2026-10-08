import json
import unittest

from check_schemas import ROOT, load_json, validator_for
from check_surface_paint_backend import baseline
from surface_paint_source import positional_paint_request


class SurfacePaintSourceTests(unittest.TestCase):
    def test_positional_record_changes_only_surface_paint_encoding(self):
        for consumer in ("surface-paint", "command-paint", "command-motion", "toggle-part-paint", "toggle-part-motion"):
            with self.subTest(consumer=consumer):
                original = baseline() if consumer == "surface-paint" else load_json(ROOT / f"conformance/ir/{consumer}-request.json")
                saved = json.dumps(original)
                validator = validator_for(f"schemas/{consumer}-request.schema.json")
                self.assertTrue(validator.is_valid(original))
                pointer = "" if consumer == "surface-paint" else "/surface"
                malformed = json.loads(positional_paint_request(original, pointer))
                self.assertFalse(validator.is_valid(malformed))
                paint = original if not pointer else original["surface"]
                positional = malformed if not pointer else malformed["surface"]
                self.assertEqual(positional, [paint["schemaVersion"], paint["body"], paint["surroundingColor"]])
                if pointer:
                    malformed["surface"] = paint
                    self.assertEqual(malformed, original)
                self.assertEqual(json.dumps(original), saved)


if __name__ == "__main__":
    unittest.main()
