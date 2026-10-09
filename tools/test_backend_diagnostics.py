import contextlib
import copy
import importlib
import io
import json
import subprocess
import unittest
from unittest.mock import patch

from check_schemas import load_json, validator_for


OPERATIONS = (
    ("activation", "interaction/activation", "activation"),
    ("command_paint", "ir/command-paint", "command-paint"),
    ("focus_indicator", "states/focus-indicator", "focus-indicator"),
    ("focus_traversal", "interaction/focus-traversal", "focus-traversal"),
    ("frost_surface_readability", "surfaces/frost-readability", "frost-surface-readability"),
    ("hit_region", "interaction/hit-region", "hit-region"),
    ("surface_paint", "ir/surface-paint", "surface-paint"),
    ("surface_readability", "surfaces/readability", "surface-readability"),
    ("toggle_part_paint", "ir/toggle-part-paint", "toggle-part-paint"),
)


class BackendDiagnosticTests(unittest.TestCase):
    def run_failures(self, operation, fixture, response):
        module = importlib.import_module(f"check_{operation}_backend")
        original_load = module.load_json

        def negative_cases(path):
            cases = original_load(path)
            if path.as_posix().endswith(f"conformance/{fixture}-cases.json"):
                return [case for case in cases if "expected" not in case]
            return cases

        with contextlib.ExitStack() as stack:
            stack.enter_context(patch("sys.argv", ["checker", "--", "backend"]))
            stack.enter_context(contextlib.redirect_stdout(io.StringIO()))
            stack.enter_context(contextlib.redirect_stderr(io.StringIO()))
            if hasattr(module, "run_backend"):
                stack.enter_context(patch.object(module, "run_backend", return_value=response))
            if callable(response):
                stack.enter_context(patch("check_color_guard_backend.run_backend", side_effect=response))
            else:
                stack.enter_context(patch("check_color_guard_backend.run_backend", return_value=response))
            # Successful results have their own operation-specific tests. Here the
            # real case schemas, requests, and every rejection path remain active.
            stack.enter_context(patch.object(module, "check_success"))
            if operation in ("focus_indicator", "frost_surface_readability", "surface_readability"):
                stack.enter_context(patch.object(module, "load_json", side_effect=negative_cases))
            result = module.main()
            return result

    def test_rejection_does_not_require_reference_wording(self):
        for operation, fixture, _ in OPERATIONS:
            with self.subTest(operation=operation):
                response = subprocess.CompletedProcess([], 1, "", "Richiesta non valida: vincolo violato.\n")
                self.assertEqual(self.run_failures(operation, fixture, response), 0)

    def test_rejection_requires_status_no_output_and_nonblank_diagnostic(self):
        for operation, fixture, _ in OPERATIONS:
            for status, stdout, stderr in (
                (0, "", "invalid request"),
                (2, "", "invalid request"),
                (1, "{}", "invalid request"),
                (1, " ", "invalid request"),
                (1, "", ""),
                (1, "", " \t\n"),
            ):
                with self.subTest(operation=operation, response=(status, stdout, stderr)):
                    response = subprocess.CompletedProcess([], status, stdout, stderr)
                    self.assertEqual(self.run_failures(operation, fixture, response), 1)

    def test_focus_and_readability_checkers_reject_positional_success(self):
        for operation, fixture, _ in OPERATIONS:
            if operation not in ("focus_indicator", "frost_surface_readability", "surface_readability"):
                continue
            for version in ("0.1.0", "9.9.9"):
                with self.subTest(operation=operation, version=version):
                    def respond(_command, source, _timeout):
                        request = json.loads(source)
                        if isinstance(request, list) and request and request[0] == version:
                            return subprocess.CompletedProcess([], 0, "{}", "")
                        return subprocess.CompletedProcess([], 1, "", "Invalid request")

                    self.assertEqual(self.run_failures(operation, fixture, respond), 1)

    def test_negative_case_schema_requires_explicit_failure(self):
        from check_schemas import ROOT

        for _, fixture, schema in OPERATIONS:
            with self.subTest(schema=schema):
                cases = load_json(ROOT / f"conformance/{fixture}-cases.json")
                case = next(case for case in cases if case.get("failure") is True)
                validator = validator_for(f"schemas/{schema}-case.schema.json")
                self.assertFalse(list(validator.iter_errors(case)))
                for value in (False, "true", None):
                    invalid = {**case, "failure": value}
                    self.assertTrue(list(validator.iter_errors(invalid)))
                invalid = copy.deepcopy(case)
                del invalid["failure"]
                self.assertTrue(list(validator.iter_errors(invalid)))
                invalid["errorContains"] = "reference wording"
                self.assertTrue(list(validator.iter_errors(invalid)))
                success = next(case for case in cases if "failure" not in case)
                invalid = {**success, "failure": True}
                self.assertTrue(list(validator.iter_errors(invalid)))


if __name__ == "__main__":
    unittest.main()
