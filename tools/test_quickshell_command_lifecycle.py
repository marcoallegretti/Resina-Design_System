import json
from types import SimpleNamespace
import unittest

from check_quickshell_command_lifecycle import effects, expect_effects, focused, read_events
from wayland_virtual_input import encode_string


def line(event):
    return " DEBUG qml: RESINA_LIFECYCLE " + json.dumps(event)


class CommandLifecycleEvidenceTests(unittest.TestCase):
    def test_events_retain_generation_and_kind(self):
        log = "\n".join((line({"kind": "started", "generation": "a"}), "  INFO: Configuration Loaded",
                         line({"kind": "effect", "generation": "a", "effects": 1})))
        self.assertEqual([event["kind"] for event in read_events(log)], ["started", "effect"])
        self.assertEqual(len(effects(read_events(log))), 1)

    def test_qml_runtime_errors_fail_the_run(self):
        for message in ("  WARN scene: @controls/ResinaCommandControl.qml[228:-1]: TypeError: Property 'synchronize'",
                        "  WARN scene: x.qml: ReferenceError: command is not defined",
                        "  WARN: QQmlVMEMetaObject: Internal error - attempted to evaluate a function in an invalid context"):
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, "QML runtime error"):
                read_events(line({"kind": "started", "generation": "a"}) + "\n" + message)

    def test_probe_failures_and_malformed_events_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "probe failure: consumer effect preceded"):
            read_events(line({"kind": "failure", "generation": "a",
                              "message": "consumer effect preceded committed activation"}))
        for event in ({"kind": "effect"}, {"generation": "a"}, ["effect"], {"kind": 1, "generation": "a"}):
            with self.subTest(event=event), self.assertRaisesRegex(ValueError, "malformed lifecycle event"):
                read_events(line(event))

    def test_native_focus_requires_every_observed_condition(self):
        observed = {"activeFocus": True, "windowActive": True, "visible": True, "enabled": True}
        self.assertTrue(focused({"nativeFocus": observed}))
        self.assertFalse(focused({"nativeFocus": None}))
        for field in observed:
            with self.subTest(field=field):
                self.assertFalse(focused({"nativeFocus": {**observed, field: False}}))

    def test_effect_counts_are_exact_from_the_marked_position(self):
        log = "\n".join(line({"kind": "effect", "generation": "a", "effects": count}) for count in (1, 2))
        session = SimpleNamespace(events=lambda: read_events(log))
        self.assertEqual(len(expect_effects(session, 1, 1, "second gesture")), 1)
        for count in (0, 2):
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "second gesture: expected"):
                expect_effects(session, 1, count, "second gesture")

    def test_wayland_strings_are_terminated_and_padded(self):
        self.assertEqual(encode_string("wl_seat"), b"\x08\x00\x00\x00wl_seat\x00")
        self.assertEqual(encode_string("abc"), b"\x04\x00\x00\x00abc\x00")
        self.assertEqual(len(encode_string("abcd")) % 4, 0)


if __name__ == "__main__":
    unittest.main()
