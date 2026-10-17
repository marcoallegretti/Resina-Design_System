import json
import os
import socket
import struct
from types import SimpleNamespace
import unittest

from check_quickshell_command_lifecycle import (
    completed, effects, expect_effects, expected_rejections, focused, read_events,
)
from wayland_virtual_input import VirtualInput, encode_string


def line(event):
    return " DEBUG qml: RESINA_LIFECYCLE " + json.dumps(event)


class CommandLifecycleEvidenceTests(unittest.TestCase):
    def test_events_retain_generation_and_kind(self):
        log = "\n".join((line({"kind": "started", "generation": "a"}), "  INFO: Configuration Loaded",
                         line({"kind": "effect", "generation": "a", "effects": 1})))
        self.assertEqual([event["kind"] for event in read_events(log)], ["started", "effect"])
        self.assertEqual(len(effects(read_events(log))), 1)

    def test_a_partially_written_event_is_read_once_complete(self):
        complete = line({"kind": "started", "generation": "a"}) + "\n"
        pending = line({"kind": "effect", "generation": "a", "effects": 1, "snapshot": "x" * 8192})
        self.assertEqual([event["kind"] for event in read_events(completed(complete + pending[:5000]))], ["started"])
        self.assertEqual([event["kind"] for event in read_events(completed(complete + pending + "\n"))],
                         ["started", "effect"])

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

    def test_rejections_are_expected_only_while_withdrawn_or_reloading(self):
        def rejected(generation="a", **surface):
            return {"kind": "rejected", "generation": generation, "error": "lost frame",
                    **{"shown": True, "windowed": True, "mounted": True, "attached": True, **surface}}
        events = [rejected(shown=False), rejected(windowed=False), rejected(mounted=False), rejected(attached=False),
                  rejected(shown=False, mounted=False), {"kind": "reload", "generation": "b", "hard": False},
                  rejected("b"), rejected("c", mounted=False)]
        self.assertEqual(expected_rejections(events), {"hidden surface: lost frame": 2,
                                                       "closed surface: lost frame": 1,
                                                       "unmounted control: lost frame": 2,
                                                       "control without a visible native window: lost frame": 1,
                                                       "reload: lost frame": 1})
        for events in ([rejected()], [rejected("b"), {"kind": "reload", "generation": "b", "hard": True}],
                       [{"kind": "reload", "generation": "b", "hard": True}, rejected("a")]):
            with self.subTest(events=events), self.assertRaisesRegex(ValueError, "while the surface was presented"):
                expected_rejections(events)
        for event in ({"kind": "rejected", "generation": "a", "error": "x"}, rejected(shown=None), rejected(attached=1),
                      {**rejected(shown=False), "error": None}):
            with self.subTest(event=event), self.assertRaisesRegex(ValueError, "malformed rejection event"):
                expected_rejections([event])

    def test_native_focus_requires_every_observed_condition(self):
        observed = {"activeFocus": True, "windowActive": True, "visible": True, "enabled": True}
        self.assertTrue(focused({"nativeFocus": observed}))
        self.assertFalse(focused({"nativeFocus": None}))
        for field in observed:
            with self.subTest(field=field):
                self.assertFalse(focused({"nativeFocus": {**observed, field: False}}))

    def test_effect_counts_are_exact_from_the_marked_position(self):
        log = "\n".join(line({"kind": "effect", "generation": "a", "effects": count}) for count in (1, 2))
        session = SimpleNamespace(events=lambda: read_events(log), quiet=lambda: None,
                                  wait=lambda predicate, description, timeout: predicate())
        self.assertEqual(len(expect_effects(session, 1, 1, "second gesture")), 1)
        for count in (0, 2):
            with self.subTest(count=count), self.assertRaisesRegex(ValueError, "second gesture: expected"):
                expect_effects(session, 1, count, "second gesture")

    def test_wayland_strings_are_terminated_and_padded(self):
        self.assertEqual(encode_string("wl_seat"), b"\x08\x00\x00\x00wl_seat\x00")
        self.assertEqual(encode_string("abc"), b"\x04\x00\x00\x00abc\x00")
        self.assertEqual(encode_string("abcd"), b"\x05\x00\x00\x00abcd\x00\x00\x00\x00")

    @unittest.skipUnless(os.name == "posix", "requires Unix domain socket descriptor passing")
    def test_wayland_requests_carry_size_opcode_and_descriptor(self):
        client, server = socket.socketpair(socket.AF_UNIX, socket.SOCK_STREAM)
        read, write = os.pipe()
        try:
            device = VirtualInput.__new__(VirtualInput)
            device.socket = client
            device.send(7, 2, struct.pack("<I", 9))
            self.assertEqual(server.recv(64), struct.pack("<III", 7, 12 << 16 | 2, 9))
            device.send(8, 0, struct.pack("<II", 1, 5), read)
            message, ancillary, _, _ = server.recvmsg(64, socket.CMSG_SPACE(4))
            self.assertEqual(message, struct.pack("<IIII", 8, 16 << 16, 1, 5))
            self.assertEqual([(level, kind) for level, kind, _ in ancillary], [(socket.SOL_SOCKET, socket.SCM_RIGHTS)])
            os.close(struct.unpack("<i", ancillary[0][2][:4])[0])
        finally:
            client.close()
            server.close()
            os.close(read)
            os.close(write)


if __name__ == "__main__":
    unittest.main()
