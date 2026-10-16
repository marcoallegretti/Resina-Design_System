import unittest

from check_quickshell_toggle_lifecycle import committed, outside_target, toggled, verify_pending, verify_publications


def effect(checked, hold=None, enabled=True):
    return {"kind": "effect", "generation": "a", "checked": checked,
            "state": {"schemaVersion": "0.1.0", "enabled": enabled, "focused": True, "hold": hold}}


def publication(snapshot_focused=True, native_focused=True, snapshot_checked=False, layout_checked=False,
                checked=False):
    return {"kind": "publication", "generation": "a", "snapshotFocused": snapshot_focused,
            "nativeFocused": native_focused, "snapshotChecked": snapshot_checked,
            "layoutChecked": layout_checked, "checked": checked}


def pending(**fields):
    return {"kind": "pending", "generation": "a", "change": "checked", "then": "unmount", "accepted": True,
            "published": False, "prepared": True, "ready": False, **fields}


class ToggleLifecycleEvidenceTests(unittest.TestCase):
    def test_each_effect_inverts_the_previous_checked_value(self):
        self.assertTrue(toggled([effect(True)], False))
        self.assertFalse(toggled([effect(True), effect(False)], False))
        self.assertTrue(toggled([], True))
        for added, checked in (([effect(False)], False), ([effect(True), effect(True)], False)):
            with self.subTest(added=added), self.assertRaisesRegex(ValueError, "exactly once"):
                toggled(added, checked)

    def test_frames_carry_synchronized_focus_and_checked_value(self):
        events = [publication(), publication(False, False, True, True, True), effect(True)]
        self.assertEqual(verify_publications(events), {True: 1, False: 1})
        with self.assertRaisesRegex(ValueError, "before native focus was synchronized"):
            verify_publications([publication(native_focused=False)])
        with self.assertRaisesRegex(ValueError, "before native focus was synchronized"):
            verify_publications([publication(snapshot_focused=False)])
        for stale in (publication(snapshot_checked=True), publication(layout_checked=True),
                      publication(checked=True)):
            with self.subTest(stale=stale), self.assertRaisesRegex(ValueError, "stale checked value"):
                verify_publications([stale])

    def test_pending_change_requires_one_prepared_unpublished_frame(self):
        verify_pending([pending(), pending(then="focus"), pending(generation="b")], "a", "checked", "unmount")
        for events in ([], [pending(), pending()], [pending(generation="b")], [pending(change="availability")]):
            with self.subTest(events=events), self.assertRaisesRegex(ValueError, "expected one unmount"):
                verify_pending(events, "a", "checked", "unmount")
        for field, value in (("accepted", False), ("published", True), ("ready", True), ("prepared", False)):
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "did not leave a prepared frame"):
                verify_pending([pending(**{field: value})], "a", "checked", "unmount")

    def test_release_point_lies_outside_the_target_on_either_axis(self):
        target = {"x": 100, "y": 50, "width": 40, "height": 20}
        self.assertFalse(outside_target((100, 50), target))
        self.assertFalse(outside_target((120, 60), target))
        self.assertTrue(outside_target((121, 50), target))
        self.assertTrue(outside_target((100, 61), target))
        self.assertTrue(outside_target((79, 39), target))

    def test_callbacks_observe_committed_enabled_activation(self):
        enter = {"kind": "key", "key": "enter"}
        self.assertTrue(committed(effect(True), "pointer"))
        self.assertTrue(committed(effect(True, enter), "enter"))
        self.assertFalse(committed(effect(True, enabled=False), "space"))
        self.assertFalse(committed(effect(True, enter), "space"))
        self.assertFalse(committed(effect(True, {"kind": "key", "key": "space"}), "enter"))
        self.assertFalse(committed(effect(True, {"kind": "pointer", "id": "1", "inside": True}), "pointer"))


if __name__ == "__main__":
    unittest.main()
