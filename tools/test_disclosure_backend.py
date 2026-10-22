import copy
import unittest

from check_disclosure_backend import (
    activation_projections,
    cases,
    expected_result,
    result_mismatch,
)
from check_schemas import validator_for


class DisclosureBackendTests(unittest.TestCase):
    def test_vectors_match_the_independent_oracle(self):
        for case in cases():
            with self.subTest(name=case["name"]):
                if "expected" in case:
                    self.assertEqual(expected_result(case["request"]), case["expected"])
                else:
                    with self.assertRaisesRegex(ValueError, case["errorContains"]):
                        expected_result(case["request"])

    def test_only_activation_toggles_expansion(self):
        baseline = copy.deepcopy(cases()[0]["request"])
        for expanded in (False, True):
            baseline["expanded"] = expanded
            baseline["event"] = {"kind": "cancel"}
            self.assertEqual(expected_result(baseline)["expanded"], expanded)
            baseline["event"] = {"kind": "invoke"}
            self.assertEqual(expected_result(baseline)["expanded"], not expanded)

    def test_activation_projection_covers_every_event_kind_and_both_states(self):
        projections = activation_projections()
        kinds = {request["event"]["kind"] for _, request, _ in projections}
        self.assertEqual(kinds, {
            "availability", "cancel", "focus", "invoke", "keyDown", "keyUp",
            "pointerCancel", "pointerDown", "pointerMove", "pointerUp",
        })
        for _, request, expected in projections:
            self.assertEqual(
                expected["expanded"],
                request["expanded"] ^ expected["activation"]["activate"],
            )

    def test_result_mutations_are_detected_or_rejected(self):
        expected = cases()[1]["expected"]
        validator = validator_for("schemas/disclosure-result.schema.json")
        for path in (
            ("expanded",),
            ("accessibility", "state", "expanded"),
            ("accessibility", "relationships", 0, "target"),
            ("accessibility", "actions", 0, "available"),
        ):
            actual = copy.deepcopy(expected)
            parent = actual
            for member in path[:-1]:
                parent = parent[member]
            member = path[-1]
            value = parent[member]
            parent[member] = not value if isinstance(value, bool) else "other-target"
            self.assertTrue(result_mismatch(actual, expected))
            if path != ("accessibility", "relationships", 0, "target"):
                self.assertFalse(validator.is_valid(actual), path)

    def test_request_schema_rejects_stale_missing_and_incoherent_targets(self):
        validator = validator_for("schemas/disclosure-request.schema.json")
        baseline = copy.deepcopy(cases()[0]["request"])
        self.assertTrue(validator.is_valid(baseline))
        mutations = []
        stale = copy.deepcopy(baseline)
        stale["controlledContent"]["present"] = False
        mutations.append(stale)
        empty = copy.deepcopy(baseline)
        empty["controlledContent"]["identity"] = ""
        mutations.append(empty)
        missing = copy.deepcopy(baseline)
        del missing["controlledContent"]
        mutations.append(missing)
        unfocusable = copy.deepcopy(baseline)
        unfocusable["focusable"] = False
        mutations.append(unfocusable)
        for request in mutations:
            self.assertFalse(validator.is_valid(request))


if __name__ == "__main__":
    unittest.main()
