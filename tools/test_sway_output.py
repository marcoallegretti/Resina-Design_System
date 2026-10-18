import os
import unittest

from sway_output import changed_scale, single_output, sway_socket


class SwayOutputTests(unittest.TestCase):
    def test_an_explicit_socket_must_be_the_session_socket(self):
        session = os.path.join("/run/user", "sway-ipc.1000.7.sock")
        self.assertEqual(sway_socket(session, "/run/user", ["sway-ipc.1000.7.sock"]), session)
        for explicit in ("/run/user/1000/sway-ipc.1000.7.sock", os.path.join("/run/user", "sway-ipc.1000.8.sock")):
            with self.subTest(explicit=explicit), self.assertRaisesRegex(ValueError, "is not the Sway IPC socket"):
                sway_socket(explicit, "/run/user", ["sway-ipc.1000.7.sock"])
        with self.assertRaisesRegex(ValueError, "exactly one Sway IPC socket"):
            sway_socket(session, "/run/user", ["sway-ipc.1000.7.sock", "sway-ipc.1000.8.sock"])

    def test_the_session_socket_is_derived_from_the_runtime_directory(self):
        found = sway_socket(None, "/run/user", ["wayland-1", "wayland-1.lock", "sway-ipc.1000.4242.sock"])
        self.assertEqual(found, os.path.join("/run/user", "sway-ipc.1000.4242.sock"))

    def test_the_derived_socket_must_be_unique(self):
        for entries in ([], ["wayland-1"], ["sway-ipc.1000.1.sock", "sway-ipc.1000.2.sock"],
                        ["sway-ipc.1000.sock"], ["sway-ipc.1000.1.sock.lock"]):
            with self.subTest(entries=entries), self.assertRaisesRegex(ValueError, "exactly one Sway IPC socket"):
                sway_socket("", "/run/user", entries)

    def test_exactly_one_active_output_is_changed(self):
        output = {"name": "HEADLESS-1", "active": True, "scale": 1.25}
        self.assertIs(single_output([{"name": "HEADLESS-2", "active": False}, output]), output)
        for outputs in ([], [{"name": "HEADLESS-1", "active": False}], [output, dict(output, name="HEADLESS-2")]):
            with self.subTest(outputs=outputs), self.assertRaisesRegex(ValueError, "exactly one active"):
                single_output(outputs)

    def test_a_live_scale_must_change_the_output(self):
        self.assertIsNone(changed_scale(1.25, None))
        self.assertEqual(changed_scale(1.25, 2), 2)
        with self.assertRaisesRegex(ValueError, "does not differ"):
            changed_scale(2, 2.0)
        for scale in (0, -1, 4.5, float("nan")):
            with self.subTest(scale=scale), self.assertRaisesRegex(ValueError, "positive output scale"):
                changed_scale(1, scale)


if __name__ == "__main__":
    unittest.main()
