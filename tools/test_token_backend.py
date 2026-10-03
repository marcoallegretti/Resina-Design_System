import unittest
from subprocess import CompletedProcess
from unittest.mock import patch

from check_token_backend import cases, check_case, mismatch


class TokenBackendCheckerTests(unittest.TestCase):
    def test_exact_values_and_json_numeric_equivalence(self):
        expected = {"token": {"token_type": "number", "value": 2}}
        self.assertIsNone(mismatch(expected, expected))
        self.assertIsNone(mismatch({"token": {"token_type": "number", "value": 2.0}}, expected))
        self.assertEqual(
            mismatch({"token": {"token_type": "number", "value": 2.000000000001}}, expected),
            "/token/value",
        )
        self.assertEqual(
            mismatch({"token": {"token_type": "number", "value": True}}, expected),
            "/token/value",
        )

    def test_public_source_and_document_vectors_are_included(self):
        loaded = list(cases())
        self.assertTrue(
            any(
                name == "source: duplicate token property fails before resolution"
                and expected is None
                for name, _, expected in loaded
            )
        )
        self.assertTrue(
            any(
                name == "document: group extension and property reference"
                and expected is not None
                for name, _, expected in loaded
            )
        )
        self.assertEqual(len({name for name, _, _ in loaded}), len(loaded))
        self.assertTrue(
            any(
                name == "foundation: authored spatial, type, radius, and depth scales"
                and expected["depth.4"] == {
                    "token_type": "dimension", "value": {"value": 8, "unit": "px"}
                }
                for name, _, expected in loaded
            )
        )

    def test_backend_output_must_be_strict_json(self):
        output = CompletedProcess([], 0, '{"token":1,"token":2}', "")
        with patch("check_token_backend.run_backend", return_value=output):
            with self.assertRaisesRegex(AssertionError, "duplicate JSON member"):
                check_case([], "duplicate output", "{}", {"token": 2}, 1)

    def test_repeated_valid_source_must_be_deterministic(self):
        first = CompletedProcess([], 0, '{"token":2}', "")
        second = CompletedProcess([], 0, '{"token":3}', "")
        with patch("check_token_backend.run_backend", side_effect=[first, second]):
            with self.assertRaisesRegex(AssertionError, "changed the result at /token"):
                check_case([], "changed output", "{}", {"token": 2}, 1)

    def test_invalid_source_cannot_emit_partial_output(self):
        output = CompletedProcess([], 1, '{"token":2}', "invalid source")
        with patch("check_token_backend.run_backend", return_value=output):
            with self.assertRaisesRegex(AssertionError, "empty stdout"):
                check_case([], "partial output", "{}", None, 1)


if __name__ == "__main__":
    unittest.main()
