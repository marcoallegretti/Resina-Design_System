import copy
import json
import unittest
from unittest import mock
import contextlib
import io
import subprocess
import signal

import check_slint_native_availability as checker

from check_slint_native_availability import assess, integer_values, parse_report, references, state_flags, string_value


HEADER = "method return time=1 sender=:1.2 -> destination=:1.3 serial=2 reply_serial=2\n"


class NativeAvailabilityTests(unittest.TestCase):
    def test_protocol_state_words_and_fixture_strings(self):
        self.assertEqual(string_value(HEADER + '   variant string "Enabled command"'), "Enabled command")
        self.assertEqual(integer_values(HEADER + "uint32 43"), [43])
        self.assertEqual(state_flags(HEADER + "array [uint32 1124075776 uint32 0]"),
                         {"enabled": True, "sensitive": True, "focusable": True})
        self.assertEqual(state_flags(HEADER + "array [uint32 2048 uint32 4294967295]"),
                         {"enabled": False, "sensitive": False, "focusable": True})

    def test_protocol_rejects_missing_malformed_or_overflowed_data(self):
        for reply in ("", "error\nuint32 43", HEADER + "uint32 -1", HEADER + "uint32 4294967296",
                      HEADER + "uint32 43 extra", HEADER + "uint32 43 uint32 43"):
            with self.subTest(reply=reply), self.assertRaises(ValueError):
                integer_values(reply)
        for body in ("array []", "array [uint32 1]", "array [uint32 1 uint32 2 uint32 3]",
                     "array [uint32 1 uint32 4294967296]", "array [uint32 1uint32 2]"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                state_flags(HEADER + body)
        for body in ('string "Enabled command" extra', 'string "bad\\name"', 'boolean true'):
            with self.subTest(body=body), self.assertRaises(ValueError):
                string_value(HEADER + body)

    def test_reference_identity_and_tree_bounds(self):
        entry = 'struct {string ":1.42" object path "/org/a11y/atspi/accessible/root"}'
        self.assertEqual(references(HEADER + "array [" + entry + "]"),
                         [(":1.42", "/org/a11y/atspi/accessible/root")])
        self.assertEqual(references(HEADER + "array []"), [])
        for entries in (entry + entry, entry + "unknown", entry.replace(":1.42", "other"),
                        "".join(entry.replace(":1.42", f":1.{index}") for index in range(17))):
            with self.subTest(entries=entries), self.assertRaises(ValueError):
                references(HEADER + "array [" + entries + "]")

    def test_cli_diagnostic_mode_never_hides_execution_errors(self):
        report = assess({name: {"enabled": True, "sensitive": True, "focusable": True}
                         for name in ("Enabled command", "Disabled command", "Excluded command")})
        for diagnostic_mode, outcome, expected in ((False, report, 1), (True, report, 0),
                                                    (True, ValueError("missing native node"), 1)):
            argv = ["checker", "--session"] + (["--report-only"] if diagnostic_mode else []) + ["--", "viewer"]
            with self.subTest(diagnostic_mode=diagnostic_mode, outcome=outcome), \
                 mock.patch.object(checker.sys, "argv", argv), \
                 mock.patch.object(checker.sys, "platform", "linux"), \
                 mock.patch.object(checker.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, "slint-viewer 1.18.1", "")), \
                 mock.patch.object(checker, "session_probe", side_effect=outcome if isinstance(outcome, Exception) else None,
                                   return_value=outcome), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(checker.main(), expected)

    @unittest.skipUnless(checker.sys.platform == "linux", "Linux process-group cleanup")
    def test_session_timeout_kills_the_owned_process_group(self):
        process = mock.Mock(pid=12345)
        process.communicate.side_effect = [subprocess.TimeoutExpired("session", 30), ("", "")]
        process.poll.return_value = None
        with mock.patch.dict(checker.os.environ, {}, clear=True), \
             mock.patch.object(checker.subprocess, "Popen", return_value=process) as spawn, \
             mock.patch.object(checker.os, "killpg") as kill_group, \
             self.assertRaisesRegex(ValueError, "exceeded 30 seconds"):
            checker.run_session(["viewer"])
        self.assertTrue(spawn.call_args.kwargs["start_new_session"])
        kill_group.assert_called_once_with(12345, signal.SIGKILL)
        self.assertEqual(process.communicate.call_args_list[-1], mock.call(timeout=3))

    def test_reports_preserve_unsupported_semantics(self):
        observed = {name: {"enabled": True, "sensitive": True, "focusable": True}
                    for name in ("Enabled command", "Disabled command", "Excluded command")}
        report = assess(observed)
        self.assertEqual(len(report["issues"]), 5)
        encoded = "resina-native-availability " + json.dumps(report)
        self.assertEqual(parse_report("service startup\n" + encoded), report)
        for output in ("", encoded + "\n" + encoded,
                       "resina-native-availability []",
                       encoded.replace('"conformant": false', '"conformant": true'),
                       encoded.replace('"conformant": false', '"conformant": 0'),
                       encoded.replace('"expected": false', '"expected": 0'),
                       "resina-native-availability " + json.dumps(dict(report, issues=[]))):
            with self.subTest(output=output), self.assertRaises(ValueError):
                parse_report(output)

    def test_semantic_mismatches_are_reported_not_silenced(self):
        conforming = {
            "Enabled command": {"enabled": True, "sensitive": True, "focusable": True},
            "Disabled command": {"enabled": False, "sensitive": False, "focusable": True},
            "Excluded command": {"enabled": False, "sensitive": False, "focusable": False},
        }
        self.assertTrue(assess(conforming)["conformant"])
        for name, states in conforming.items():
            for field, value in states.items():
                changed = copy.deepcopy(conforming)
                changed[name][field] = not value
                report = assess(changed)
                self.assertFalse(report["conformant"])
                self.assertEqual(report["issues"], [{"command": name, "state": field,
                                                    "expected": value, "actual": not value}])
        for changed in ({}, dict(conforming, extra=conforming["Enabled command"]),
                        dict(conforming, **{"Enabled command": {"enabled": 1}})):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                assess(changed)


if __name__ == "__main__":
    unittest.main()
