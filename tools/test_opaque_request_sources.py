import json
import unittest

from check_schemas import ROOT, apply_changes, load_json, validator_for
from check_surface_paint_backend import baseline as paint_baseline
from opaque_request_source import opaque_request_failures


class OpaqueRequestSourceTests(unittest.TestCase):
    def test_failures_change_only_the_opaque_record_encoding(self):
        consumers = [("opaque-surface", ""), ("surface-paint", "/body")]
        consumers += [(name, "/surface/body") for name in (
            "command-paint", "command-motion", "toggle-part-paint", "toggle-part-motion",
        )]
        for consumer, pointer in consumers:
            original = paint_baseline() if consumer == "surface-paint" else load_json(ROOT / f"conformance/ir/{consumer}-request.json")
            validator = validator_for(f"schemas/{consumer}-request.schema.json")
            self.assertTrue(validator.is_valid(original))
            cases = [case for case in load_json(ROOT / f"conformance/ir/{consumer}-cases.json")
                     if case["name"] == "opaque request source tagged foreground role"]
            self.assertEqual(len(cases), 1)
            case = cases[0]
            self.assertTrue(validator_for(f"schemas/{consumer}-case.schema.json").is_valid(case))
            self.assertFalse(case["requestSchemaValid"])
            self.assertEqual(case["requestChanges"][0]["path"], pointer + "/foregroundRole")
            self.assertEqual(apply_changes(original, case["requestChanges"]),
                             json.loads(opaque_request_failures(original, pointer)[1][1]))
            failures = opaque_request_failures(original, pointer)
            self.assertEqual(len(failures), 2)
            for name, source in failures:
                with self.subTest(consumer=consumer, name=name):
                    changed = json.loads(source)
                    self.assertFalse(validator.is_valid(changed))
                    body = original
                    parent = changed
                    members = pointer.strip("/").split("/") if pointer else []
                    for member in members:
                        body = body[member]
                    if members:
                        for member in members[:-1]:
                            parent = parent[member]
                        malformed = parent[members[-1]]
                        parent[members[-1]] = body
                        self.assertEqual(changed, original)
                    else:
                        malformed = changed
                    if name == "positional opaque request":
                        self.assertIsInstance(malformed, list)
                        self.assertEqual(len(malformed), len(body))
                        self.assertEqual(malformed, [
                            body["schemaVersion"], body["theme"], body["surface"], body["size"],
                            body["appearance"], body["foregroundRole"], body["postTreatmentBackdrop"],
                            body["adjacentColor"], body["minimumContentContrast"], body["minimumEdgeContrast"],
                        ])
                    else:
                        self.assertEqual(malformed, {**body, "foregroundRole": {body["foregroundRole"]: None}})
            self.assertTrue(validator.is_valid(original))


if __name__ == "__main__":
    unittest.main()
