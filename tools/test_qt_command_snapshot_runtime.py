import copy
import json
import unittest

from jsonschema import ValidationError
from check_qt_command_snapshot_runtime import ROOT, request_for, verify_snapshot


class NativeSnapshotEvidenceTests(unittest.TestCase):
    def fixture(self):
        load = lambda path: json.loads((ROOT / path).read_text())
        # Supplied public pieces exercise checker invariants; no native font claim.
        body = load("conformance/ir/opaque-surface-expected.json")
        body["materialRole"] = "control.interactive"
        label = load("conformance/ir/command-label-ir.json")
        label.update(size={"width": 20, "height": 12},
                     labelBounds={"x": 2, "y": 2, "width": 16, "height": 8})
        semantic = load("conformance/accessibility/command-cases.json")[0]["expected"]
        target = load("conformance/interaction/hit-region-cases.json")[0]["expected"]
        snapshot = {"label": label, "paint": {"schemaVersion": "0.1.0", "phase": "rest",
                    "response": {"bodyMix": 0, "depthScale": 1},
                    "paint": {"schemaVersion": "0.1.0", "body": body}},
                    "hitRegion": target, "accessibility": semantic}
        request = request_for("light", "Reconnect")
        request.update(description=None, componentMinimum={"width": 24, "height": 24})
        request["content"]["environment"]["inputCapabilities"] = ["finePointer"]
        return snapshot, request, body["states"]

    def test_supplied_public_channels(self):
        verify_snapshot(*self.fixture())

    def test_resolved_minimum_matches_authored_environment_and_component(self):
        vectors = json.loads((ROOT / "conformance/interaction/target-minimum-vectors.json").read_text())
        for vector in vectors:
            for component in ({"width": 12, "height": 16}, {"width": 80, "height": 32}):
                snapshot, request, states = copy.deepcopy(self.fixture())
                request["content"]["environment"].update(
                    {key: value for key, value in vector.items() if key not in ("name", "expected")})
                request["componentMinimum"] = component
                minimum = {axis: max(component[axis], vector["expected"][field])
                           for axis, field in (("width", "minimumWidth"), ("height", "minimumHeight"))}
                snapshot["hitRegion"]["minimumSize"] = minimum
                snapshot["hitRegion"]["bounds"].update(minimum)
                with self.subTest(vector=vector["name"], component=component):
                    verify_snapshot(snapshot, request, states)
                    for axis in ("width", "height"):
                        for value in (1, minimum[axis] + 1):
                            bad = copy.deepcopy(snapshot)
                            bad["hitRegion"]["minimumSize"][axis] = value
                            with self.subTest(axis=axis, value=value), self.assertRaisesRegex(
                                    ValueError, "resolved target minimum"):
                                verify_snapshot(bad, request, states)

    def test_target_bounds_preserve_environment_floor(self):
        for capability in ("coarsePointer", "directTouch"):
            for axis in ("width", "height"):
                snapshot, request, states = copy.deepcopy(self.fixture())
                request["content"]["environment"]["inputCapabilities"] = [capability]
                snapshot["hitRegion"]["minimumSize"] = {"width": 48, "height": 48}
                snapshot["hitRegion"]["bounds"].update(width=48, height=48)
                snapshot["hitRegion"]["bounds"][axis] = 24
                with self.subTest(capability=capability, axis=axis), self.assertRaisesRegex(
                        ValueError, "resolved target minimum"):
                    verify_snapshot(snapshot, request, states)

    def test_partial_stale_or_inconsistent_channels_fail(self):
        for change in ("missing", "extra", "semantic", "body", "direction", "focus", "target", "available", "occupied"):
            snapshot, request, states = copy.deepcopy(self.fixture())
            if change == "missing":
                del snapshot["hitRegion"]
            elif change == "extra":
                snapshot["renderer"] = "native"
            elif change == "semantic":
                snapshot["accessibility"]["name"] = "Stale name"
            elif change == "body":
                snapshot["label"]["size"]["width"] = 21
            elif change == "direction":
                snapshot["label"]["layoutDirection"] = "rtl"
            elif change == "focus":
                request["activation"]["focused"] = True
            elif change == "target":
                snapshot["hitRegion"]["bounds"]["height"] = 10
            elif change == "available":
                request["availableBounds"]["width"] = 1
            else:
                request["occupiedRegions"] = [{"x": 0, "y": 0, "width": 1, "height": 1}]
            with self.subTest(change=change), self.assertRaises((ValueError, ValidationError)):
                verify_snapshot(snapshot, request, states)


if __name__ == "__main__":
    unittest.main()
