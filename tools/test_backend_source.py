import copy
import io
import json
import math
import runpy
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from subprocess import CompletedProcess
from unittest.mock import patch

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_backend
from check_schemas import parse_json, validator_for


class BackendSourceTests(unittest.TestCase):
    def test_only_selected_member_is_duplicated_and_document_is_unchanged(self):
        document = {
            "a/b": [{"~key": {"text": '"λ"', "value": [True, None, 1.5]}}],
            "other": "source-member-value", "": 4,
        }
        original = copy.deepcopy(document)
        for pointer in ("/a~1b/0/~0key/value", "/other", "/"):
            source = duplicate_member_source(document, pointer)
            self.assertEqual(json.loads(source), original)
            self.assertEqual(document, original)
            with self.assertRaisesRegex(ValueError, "duplicate JSON member"):
                parse_json(source, pointer)
            duplicates = []

            def pairs(members):
                keys = [name for name, _ in members]
                duplicates.append(len(keys) - len(set(keys)))
                return dict(members)

            self.assertEqual(json.loads(source, object_pairs_hook=pairs), original)
            self.assertEqual(sum(duplicates), 1)

    def test_missing_invalid_or_array_targets_fail_explicitly(self):
        document = {"items": [{"value": 1}]}
        for pointer in (
            "", "items", "/missing", "/items/01/value", "/items/2/value",
            "/items/0", "/bad~2", "/items/0/value/missing",
        ):
            for operation in (duplicate_member_source, nonfinite_member_source):
                with self.assertRaises(ValueError, msg=pointer):
                    operation(document, pointer)
        with self.assertRaises(ValueError):
            duplicate_member_source({"number": float("nan")}, "/number")

    def test_nonfinite_number_changes_only_selected_numeric_member(self):
        document = {"items": [{"number": 1.5}], "text": "1e400"}
        source = nonfinite_member_source(document, "/items/0/number")
        with self.assertRaisesRegex(ValueError, "exceeds finite range"):
            parse_json(source, "overflow")
        restored = json.loads(source, parse_float=lambda value: 1.5 if value == "1e400" else float(value))
        self.assertEqual(restored, document)
        self.assertEqual(document["items"][0]["number"], 1.5)
        for value in (True, None, "number", {}, []):
            with self.assertRaisesRegex(ValueError, "must be a number"):
                nonfinite_member_source({"value": value}, "/value")

    def test_registered_nested_duplicates_are_otherwise_valid_requests(self):
        modules = {
            "check_key_light_backend": "key-light-request",
            "check_extruded_contour_backend": "extruded-contour-request",
            "check_inset_contour_backend": "inset-contour-request",
            "check_opaque_pigment_backend": "opaque-pigment-request",
            "check_elevation_depth_backend": "elevation-depth-request",
            "check_focus_ir_backend": "focus-ir-request",
            "check_opaque_surface_backend": "opaque-surface-request",
        }
        for module, schema in modules.items():
            captured = []

            def capture(*args, **kwargs):
                captured.extend(kwargs["extra_failures"])
                return 0

            with (
                patch("check_color_guard_backend.check_backend", side_effect=capture),
                patch("check_delta_backend.check_delta_backend", side_effect=capture),
            ):
                with self.assertRaises(SystemExit) as result:
                    runpy.run_module(module, run_name="__main__")
            self.assertEqual(result.exception.code, 0)
            duplicates = [(name, source) for name, source in captured if "duplicate" in name]
            self.assertTrue(duplicates, module)
            for name, source in duplicates:
                with self.assertRaisesRegex(ValueError, "duplicate JSON member"):
                    parse_json(source, name)
                errors = list(validator_for(f"schemas/{schema}.schema.json").iter_errors(json.loads(source)))
                self.assertFalse(errors, f"{module}: {name}: {errors}")
            baseline = json.loads(duplicates[0][1])
            for name, source in captured:
                if "nonfinite" not in name and "overflow" not in name:
                    continue
                with self.assertRaisesRegex(ValueError, "exceeds finite range"):
                    parse_json(source, name)
                overflows = []

                def restore(actual, expected):
                    if isinstance(actual, float) and math.isinf(actual):
                        self.assertIsInstance(expected, (int, float))
                        self.assertNotIsInstance(expected, bool)
                        overflows.append(actual)
                        return expected
                    if isinstance(expected, dict):
                        self.assertEqual(actual.keys(), expected.keys())
                        return {key: restore(actual[key], value) for key, value in expected.items()}
                    if isinstance(expected, list):
                        self.assertEqual(len(actual), len(expected))
                        return [restore(a, e) for a, e in zip(actual, expected)]
                    self.assertEqual(actual, expected)
                    return actual

                self.assertEqual(restore(json.loads(source), baseline), baseline)
                self.assertEqual(len(overflows), 1)

    def test_shared_checker_duplicate_is_not_masked_by_schema_failure(self):
        duplicate_sources = []

        def rejected(command, source, timeout, name):
            if name == "duplicate request member":
                duplicate_sources.append(source)
                validator = validator_for("schemas/key-light-request.schema.json")
                self.assertFalse(list(validator.iter_errors(json.loads(source))))
                with self.assertRaisesRegex(ValueError, "duplicate JSON member"):
                    parse_json(source, name)

        with (
            patch.object(sys, "argv", ["checker", "--", "backend", "-"]),
            patch("check_color_guard_backend.check_success"),
            patch("check_color_guard_backend.check_failure", side_effect=rejected),
            redirect_stdout(io.StringIO()),
        ):
            result = check_backend(
                "key light", "schemas/key-light-case.schema.json",
                "schemas/key-light-request.schema.json", "schemas/key-light-result.schema.json",
                "conformance/lighting/key-light-vectors.json",
            )
        self.assertEqual(result, 0)
        self.assertEqual(len(duplicate_sources), 1)

    def test_shared_checker_catches_parser_that_accepts_valid_duplicates(self):
        validator = validator_for("schemas/key-light-request.schema.json")

        def permissive(command, source, timeout):
            duplicates = []

            def members(pairs):
                names = [key for key, _ in pairs]
                duplicates.append(len(names) != len(set(names)))
                return dict(pairs)

            request = json.loads(source, object_pairs_hook=members)
            if any(duplicates) and not list(validator.iter_errors(request)):
                return CompletedProcess(command, 0, "{}", "")
            return CompletedProcess(command, 1, "", "invalid request")

        diagnostic = io.StringIO()
        with (
            patch.object(sys, "argv", ["checker", "--", "backend", "-"]),
            patch("check_color_guard_backend.check_success"),
            patch("check_color_guard_backend.run_backend", side_effect=permissive),
            redirect_stdout(io.StringIO()), redirect_stderr(diagnostic),
        ):
            result = check_backend(
                "key light", "schemas/key-light-case.schema.json",
                "schemas/key-light-request.schema.json", "schemas/key-light-result.schema.json",
                "conformance/lighting/key-light-vectors.json",
            )
        self.assertEqual(result, 1)
        self.assertIn("duplicate request member: failure must exit 1", diagnostic.getvalue())

    def test_shared_checker_requires_successful_control_fixture(self):
        diagnostic = io.StringIO()
        with (
            patch.object(sys, "argv", ["checker", "--", "backend", "-"]),
            patch("check_color_guard_backend.load_json", return_value=[]),
            redirect_stderr(diagnostic),
        ):
            result = check_backend(
                "key light", "schemas/key-light-case.schema.json",
                "schemas/key-light-request.schema.json", "schemas/key-light-result.schema.json",
                "conformance/lighting/key-light-vectors.json",
            )
        self.assertEqual(result, 1)
        self.assertIn("requires a successful request", diagnostic.getvalue())

    def test_shape_checker_duplicates_are_otherwise_valid_requests(self):
        validator = validator_for("schemas/shape-fallback-request.schema.json")
        duplicates = []

        def rejected(command, source, timeout, name):
            if "duplicate" in name:
                duplicates.append(source)
                self.assertFalse(list(validator.iter_errors(json.loads(source))))
                with self.assertRaisesRegex(ValueError, "duplicate JSON member"):
                    parse_json(source, name)

        with (
            patch.object(sys, "argv", ["checker", "--", "backend", "-"]),
            patch("check_color_guard_backend.check_success"),
            patch("check_color_guard_backend.check_failure", side_effect=rejected),
            redirect_stdout(io.StringIO()),
        ):
            with self.assertRaises(SystemExit) as result:
                runpy.run_module("check_shape_fallback_backend", run_name="__main__")
        self.assertEqual(result.exception.code, 0)
        self.assertEqual(len(duplicates), 2)
