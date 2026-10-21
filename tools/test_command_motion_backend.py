import copy
import unittest
from unittest.mock import patch

from check_command_motion_backend import equation_sample, motion_mismatch
from check_schemas import ROOT, load_json, validator_for


class CommandMotionComparisonTests(unittest.TestCase):
    def test_equation_preserves_velocity_at_equilibrium_and_reduced_endpoint(self):
        channel = {"dynamics": {"mass": 1, "stiffness": 4, "damping": 0,
                    "positionThreshold": 1e-6, "velocityThreshold": 1e-6},
                   "initial": {"position": 2, "velocity": 4}}
        initial = equation_sample(channel, 2, 0, False)
        self.assertEqual(initial["state"], channel["initial"])
        self.assertFalse(initial["settled"])
        immediate = equation_sample(channel, -2, 0, True)
        self.assertEqual(immediate["state"], {"position": -2, "velocity": 0})
        self.assertTrue(immediate["settled"])

    def test_metadata_is_exact_before_paint_comparison(self):
        request = load_json(ROOT / "conformance/ir/command-motion-request.json")
        target = request["commandAppearance"]["profiles"]["elastomer"]["pressed"]
        actual = {"schemaVersion": "0.2.0", "policy": "spring", "target": copy.deepcopy(target)}
        actual["target"]["bodyMix"] += 1e-13
        self.assertEqual(motion_mismatch(actual, request), "/target")
        actual["target"] = target
        actual["policy"] = "reducedMotion"
        self.assertEqual(motion_mismatch(actual, request), "/policy")

    def test_static_rest_identity_is_retained_only_for_target(self):
        schema = validator_for("schemas/command-motion-ir.schema.json")
        body = load_json(ROOT / "conformance/ir/lit-control-body-ir.json")
        body["states"]["states"] = ["rest", "focused"]
        paint = {"schemaVersion": "0.1.0", "body": body,
                 "focus": load_json(ROOT / "conformance/ir/focus-ir-expected.json")}
        command = {"schemaVersion": "0.2.0", "phase": "rest", "response": {"bodyMix": 0.1, "depthScale": 0.5},
                   "paint": paint}
        channel = {"schemaVersion": "0.1.0", "representation": "spring", "target": 0,
                   "state": {"position": 0.1, "velocity": 0.2}, "settled": False}
        result = {"schemaVersion": "0.2.0", "policy": "spring", "target": {"bodyMix": 0, "depthScale": 1},
                  "bodyMix": channel, "depthScale": {**channel, "target": 1},
                  "bodyMixProjection": "none", "depthScaleProjection": "none", "command": command}
        self.assertFalse(list(schema.iter_errors(result)))
        result["target"]["bodyMix"] = 0.1
        self.assertTrue(list(schema.iter_errors(result)))
        result["target"]["bodyMix"] = 0
        result["policy"] = "reducedMotion"
        self.assertTrue(list(schema.iter_errors(result)))

    def test_paint_projection_uses_accepted_backend_sample_not_oracle_rounding(self):
        request = load_json(ROOT / "conformance/ir/command-motion-request.json")
        request["time"] = 0
        request["channels"]["depthScale"]["initial"]["position"] = 2
        target = request["commandAppearance"]["profiles"]["elastomer"]["pressed"]
        result = {"schemaVersion": "0.2.0", "policy": "spring", "target": target, "command": {}}
        for name in ("bodyMix", "depthScale"):
            result[name] = equation_sample(request["channels"][name], target[name], 0, False)
            result[name + "Projection"] = "none"
        result["bodyMix"]["state"]["position"] = 5e-10
        result["depthScale"]["state"]["position"] = 2 + 5e-10
        result["depthScaleProjection"] = "upperBound"
        with patch("check_command_motion_backend.command_mismatch", return_value=None) as paint:
            self.assertIsNone(motion_mismatch(result, request))
            self.assertEqual(paint.call_args.args[2], {"bodyMix": 5e-10, "depthScale": 2})
        result["depthScaleProjection"] = "none"
        self.assertEqual(motion_mismatch(result, request), "/depthScaleProjection")
