import copy
import unittest
from unittest.mock import patch

from check_command_motion_backend import equation_sample
from check_toggle_part_motion_backend import toggle_motion_mismatch
from check_schemas import ROOT, load_json, validator_for


class TogglePartMotionComparisonTests(unittest.TestCase):
    def test_metadata_is_exact_before_paint_comparison(self):
        request = load_json(ROOT / "conformance/ir/toggle-part-motion-request.json")
        target = request["interactionAppearance"]["profiles"]["elastomer"]["pressed"]
        actual = {"schemaVersion": "0.2.0", "policy": "spring", "target": copy.deepcopy(target)}
        actual["target"]["bodyMix"] += 1e-13
        self.assertEqual(toggle_motion_mismatch(actual, request), "/target")
        actual["target"] = target
        actual["policy"] = "reducedMotion"
        self.assertEqual(toggle_motion_mismatch(actual, request), "/policy")

    def test_static_rest_identity_is_retained_only_for_target(self):
        schema = validator_for("schemas/toggle-part-motion-ir.schema.json")
        body = load_json(ROOT / "conformance/ir/lit-control-body-ir.json")
        body["states"]["states"] = ["rest", "focused"]
        part_paint = {"schemaVersion": "0.2.0", "part": "track", "checked": False, "phase": "rest",
                      "response": {"bodyMix": 0.1, "depthScale": 0.5},
                      "paint": {"schemaVersion": "0.1.0", "body": body,
                                "focus": load_json(ROOT / "conformance/ir/focus-ir-expected.json")}}
        channel = {"schemaVersion": "0.1.0", "representation": "spring", "target": 0,
                   "state": {"position": 0.1, "velocity": 0.2}, "settled": False}
        result = {"schemaVersion": "0.2.0", "policy": "spring", "target": {"bodyMix": 0, "depthScale": 1},
                  "bodyMix": channel, "depthScale": {**channel, "target": 1},
                  "bodyMixProjection": "none", "depthScaleProjection": "none", "partPaint": part_paint}
        self.assertFalse(list(schema.iter_errors(result)))
        static = validator_for("schemas/toggle-part-paint-ir.schema.json")
        self.assertFalse(static.is_valid(part_paint))
        endpoint = copy.deepcopy(part_paint)
        endpoint["response"] = {"bodyMix": 0, "depthScale": 1}
        self.assertTrue(static.is_valid(endpoint))
        for field, value in [("checked", True), ("part", "thumb"), ("phase", "pressed")]:
            broken = copy.deepcopy(result)
            broken["partPaint"][field] = value
            self.assertFalse(schema.is_valid(broken))
        result["target"]["bodyMix"] = 0.1
        self.assertTrue(list(schema.iter_errors(result)))
        result["target"]["bodyMix"] = 0
        result["policy"] = "reducedMotion"
        self.assertTrue(list(schema.iter_errors(result)))

    def test_paint_projection_uses_accepted_backend_sample_not_oracle_rounding(self):
        request = load_json(ROOT / "conformance/ir/toggle-part-motion-request.json")
        request["time"] = 0
        request["channels"]["depthScale"]["initial"]["position"] = 2
        target = request["interactionAppearance"]["profiles"]["elastomer"]["pressed"]
        result = {"schemaVersion": "0.2.0", "policy": "spring", "target": target, "partPaint": {}}
        for name in ("bodyMix", "depthScale"):
            result[name] = equation_sample(request["channels"][name], target[name], 0, False)
            result[name + "Projection"] = "none"
        result["bodyMix"]["state"]["position"] = 5e-10
        result["depthScale"]["state"]["position"] = 2 + 5e-10
        result["depthScaleProjection"] = "upperBound"
        with patch("check_toggle_part_motion_backend.toggle_part_mismatch", return_value=None) as paint:
            self.assertIsNone(toggle_motion_mismatch(result, request))
            self.assertEqual(paint.call_args.args[2], {"bodyMix": 5e-10, "depthScale": 2})
        result["depthScaleProjection"] = "none"
        self.assertEqual(toggle_motion_mismatch(result, request), "/depthScaleProjection")

    def test_request_requires_complete_explicit_inputs_without_backend_fields(self):
        schema = validator_for("schemas/toggle-part-motion-request.schema.json")
        request = load_json(ROOT / "conformance/ir/toggle-part-motion-request.json")
        self.assertTrue(schema.is_valid(request))
        for field in request:
            broken = copy.deepcopy(request)
            del broken[field]
            self.assertFalse(schema.is_valid(broken))
        for channel in ("bodyMix", "depthScale"):
            for field in ("dynamics", "initial"):
                broken = copy.deepcopy(request)
                del broken["channels"][channel][field]
                self.assertFalse(schema.is_valid(broken))
        request["backend"] = "guido"
        self.assertFalse(schema.is_valid(request))
