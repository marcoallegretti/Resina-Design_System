import contextlib
import copy
import io
import json
import os
import subprocess
import unittest
from pathlib import Path
from unittest import mock

import check_qt_native_availability as checker
from run_qt_availability_probe import ActivationProbe
import run_qt_availability_probe as runner


HEADER = "method return time=1 sender=:1.2 -> destination=:1.3 serial=2 reply_serial=2\n"


def measured_report():
    observed = {name: {"enabled": enabled, "sensitive": enabled,
                      "focusable": enabled}
                for name, (enabled, _) in checker.EXPECTED.items()}
    actions = {name: ["Press"] for name in checker.EXPECTED}
    callbacks = [
        {"name": "Disabled command", "available": False, "activations": 0},
        {"name": "Excluded command", "available": False, "activations": 0},
        {"name": "Enabled command", "available": True, "activations": 1},
        {"name": "Enabled command", "available": False, "activations": 1},
    ]
    for record in callbacks:
        state = {"schemaVersion": "0.1.0", "enabled": record["available"], "focused": False, "hold": None}
        result = {"schemaVersion": "0.1.0", "state": state, "activate": record["available"],
                  "pressed": False, "capture": None}
        disabled = dict(state, enabled=False)
        record.update(invokeResult=result, availabilityResult=(
            dict(result, state=disabled, activate=False) if record["available"] else None
        ), committedState=disabled)
    return checker.report(observed, actions, callbacks,
                          {"enabled": False, "sensitive": False, "focusable": False})


class QtNativeAvailabilityTests(unittest.TestCase):
    def test_failed_protocol_does_not_commit_state_or_deliver_intent(self):
        probe = ActivationProbe()
        before = copy.deepcopy(probe.states)
        responses = [subprocess.CompletedProcess([], 1, "", "invalid request"),
                     subprocess.CompletedProcess([], 0, "{}", "unexpected diagnostics"),
                     subprocess.CompletedProcess([], 0, "not json", "")]
        for response in responses:
            with self.subTest(response=response), \
                 mock.patch("run_qt_availability_probe.subprocess.run", return_value=response), \
                 self.assertRaises(ValueError):
                probe.invoke("Enabled command", True)
            self.assertEqual(probe.states, before)
            self.assertEqual(probe.activations["Enabled command"], 0)
        with self.assertRaisesRegex(ValueError, "disagree"):
            probe.invoke("Enabled command", False)

    def test_native_actions_require_one_unambiguous_press(self):
        press = 'struct {string "Press" string "Presses the button" string ""}'
        focus = 'struct {string "SetFocus" string "Sets focus" string ""}'
        self.assertEqual(checker.action_names(HEADER + "array [" + press + focus + "]"),
                         ["Press", "SetFocus"])
        for body in ("array []", "array [" + focus + "]", "array [" + press * 2 + "]",
                     "array [" + press + "junk]", 'array [struct {string "Press"}]', "boolean true"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                checker.action_names(HEADER + body)

    def test_report_keeps_discoverable_disabled_failure(self):
        result = measured_report()
        self.assertFalse(result["conformant"])
        self.assertEqual(result["issues"], [{"command": "Disabled command", "state": "focusable",
                                             "expected": True, "actual": False}])
        self.assertEqual(checker.parse_report(checker.PREFIX + json.dumps(result)), result)

    def test_report_rejects_missing_contradictory_and_invalid_evidence(self):
        result = measured_report()
        mutations = [
            lambda r: r.update(conformant=True),
            lambda r: r.update(issues=[]),
            lambda r: r.update(actions=None),
            lambda r: r.update(binding="unknown"),
            lambda r: r.update(host="unknown"),
            lambda r: r.update(host="quickshell-0.3.1-qt-6.11.2"),
            lambda r: r["actions"]["Enabled command"].append("Press"),
            lambda r: r["callbacks"].pop(),
            lambda r: r["callbacks"][0].update(activations=1),
            lambda r: r["callbacks"][3].update(activations=2),
            lambda r: r["callbacks"][2].update(available=1),
            lambda r: r["callbacks"][3]["invokeResult"].update(activate=True),
            lambda r: r["callbacks"][2]["committedState"].update(enabled=True),
            lambda r: r["after"].update(enabled=True),
            lambda r: r["observed"]["Enabled command"].update(extra=False),
        ]
        for mutation in mutations:
            damaged = copy.deepcopy(result)
            mutation(damaged)
            with self.subTest(damaged=damaged), self.assertRaises(ValueError):
                checker.parse_report(checker.PREFIX + json.dumps(damaged))
        for output in ("", checker.PREFIX + "{}", (checker.PREFIX + json.dumps(result) + "\n") * 2):
            with self.subTest(output=output), self.assertRaises(ValueError):
                checker.parse_report(output)

    def test_quickshell_host_requires_rust_and_keeps_the_semantic_mismatch(self):
        result = measured_report()
        result.update(host="quickshell-0.3.1-qt-6.11.2", binding="rust-qt-runtime")
        self.assertEqual(checker.parse_report(checker.PREFIX + json.dumps(result)), result)
        self.assertFalse(result["conformant"])
        with mock.patch.dict(os.environ, {"RESINA_QUICKSHELL_HOST": "/not/a/host"}, clear=True):
            with self.assertRaisesRegex(ValueError, "compiled Rust"):
                runner.main()

    def test_released_quickshell_host_preserves_version_and_semantic_mismatch(self):
        result = measured_report()
        result.update(host="quickshell-0.3.2-qt-6.11.2", binding="rust-qt-runtime")
        self.assertEqual(checker.parse_report(checker.PREFIX + json.dumps(result)), result)
        self.assertFalse(result["conformant"])
        result.update(binding="reference-protocol")
        with self.assertRaisesRegex(ValueError, "host evidence"):
            checker.parse_report(checker.PREFIX + json.dumps(result))

    def test_released_quickshell_version_is_reported_without_environment(self):
        output = "Quickshell: 0.3.2 (revision release, distributed by Test)\nQt: 6.11.2 (built against 6.11.2)\n"
        with mock.patch.dict(os.environ, {"RESINA_QUICKSHELL_HOST": str(Path(__file__).resolve()),
                                         "RESINA_QT_RUNTIME_MODULE": "/module"}, clear=True), \
             mock.patch.object(runner.sys, "argv", ["runner", "--version"]), \
             mock.patch.object(runner.subprocess, "run", return_value=
                               subprocess.CompletedProcess([], 0, output + "private environment\n", "")), \
             contextlib.redirect_stdout(io.StringIO()) as captured:
            self.assertEqual(runner.main(), 0)
            self.assertEqual(captured.getvalue(), "Quickshell 0.3.2 / Qt Quick 6.11.2\n")

    def test_released_quickshell_session_receives_verified_host_identity(self):
        result = measured_report()
        result.update(host="quickshell-0.3.2-qt-6.11.2", binding="rust-qt-runtime")
        with mock.patch.object(checker.sys, "argv", ["checker", "--session", "--report-only"]), \
             mock.patch.object(checker.sys, "platform", "linux"), \
             mock.patch.dict(os.environ, {"DISPLAY": ":42", "RESINA_QUICKSHELL_HOST": "/host"}), \
             mock.patch.object(checker.subprocess, "run", return_value=
                               subprocess.CompletedProcess([], 0, "Quickshell 0.3.2 / Qt Quick 6.11.2\n", "")), \
             mock.patch.object(checker, "session_probe", return_value=result) as probe, \
             contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(checker.main(), 0)
            probe.assert_called_once_with("quickshell-0.3.2-qt-6.11.2")

    def test_quickshell_version_checks_build_and_runtime_without_forwarding_environment(self):
        valid = "Quickshell: 0.3.1 (revision abc, distributed by Test)\nQt: 6.11.2 (built against 6.11.2)\n"
        for output in (valid, valid.replace("0.3.1", "0.3.0"),
                       valid.replace("0.3.1", "0.3.3"),
                       valid.replace("built against 6.11.2", "built against 6.11.1"),
                       valid.replace("Qt: 6.11.2", "Qt: 6.11.1"), valid + valid,
                       valid + "Quickshell: 0.3.2 (revision other)\n",
                       valid + "Qt: 6.11.1 (built against 6.11.1)\n",
                       valid.replace("0.3.1", "0.3.2").replace("built against 6.11.2", "built against 6.11.1")):
            with self.subTest(output=output), \
                 mock.patch.dict(os.environ, {"RESINA_QUICKSHELL_HOST": str(Path(__file__).resolve()),
                                              "RESINA_QT_RUNTIME_MODULE": "/module"}, clear=True), \
                 mock.patch.object(runner.sys, "argv", ["runner", "--version"]), \
                 mock.patch.object(runner.subprocess, "run", return_value=
                     subprocess.CompletedProcess([], 0, output + "secret environment\n", "")), \
                 contextlib.redirect_stdout(io.StringIO()) as captured:
                if output == valid:
                    self.assertEqual(runner.main(), 0)
                    self.assertEqual(captured.getvalue(), "Quickshell 0.3.1 / Qt Quick 6.11.2\n")
                else:
                    with self.assertRaises(ValueError):
                        runner.main()
                    self.assertEqual(captured.getvalue(), "")

    def test_session_cannot_relabel_a_different_verified_host(self):
        result = measured_report()
        for native_session in (False, True):
            arguments = ["checker", "--report-only"] + (["--session"] if native_session else [])
            with self.subTest(native_session=native_session), \
                 mock.patch.object(checker.sys, "argv", arguments), \
                 mock.patch.object(checker.sys, "platform", "linux"), \
                 mock.patch.dict(os.environ, {"DISPLAY": ":42", "RESINA_QUICKSHELL_HOST": "/host"}), \
                 mock.patch.object(checker.subprocess, "run", return_value=
                                   subprocess.CompletedProcess([], 0, "Quickshell 0.3.2 / Qt Quick 6.11.2\n", "")), \
                 mock.patch.object(checker, "session_probe", return_value=result), \
                 mock.patch.object(checker, "run_session", return_value=result), \
                 contextlib.redirect_stdout(io.StringIO()) as output, \
                 contextlib.redirect_stderr(io.StringIO()) as errors:
                self.assertEqual(checker.main(), 1)
                self.assertEqual(output.getvalue(), "")
                self.assertIn("disagrees with the verified runtime", errors.getvalue())

    def test_report_only_never_hides_execution_errors(self):
        result = measured_report()
        for report_only, outcome, expected in ((False, result, 1), (True, result, 0),
                                               (True, ValueError("native host failed"), 1)):
            with self.subTest(report_only=report_only, outcome=outcome), \
                 mock.patch.object(checker.sys, "argv", ["checker", "--session"] +
                                   (["--report-only"] if report_only else [])), \
                 mock.patch.object(checker.sys, "platform", "linux"), \
                 mock.patch.dict(os.environ, {"DISPLAY": ":42"}, clear=True), \
                 mock.patch.object(checker.subprocess, "run", return_value=
                                   subprocess.CompletedProcess([], 0, "Qt Quick 6.11.2", "")), \
                 mock.patch.object(checker, "session_probe", return_value=outcome,
                                   side_effect=outcome if isinstance(outcome, Exception) else None), \
                 contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(checker.main(), expected)

    @unittest.skipUnless(checker.sys.platform == "linux", "Linux process-group cleanup")
    def test_session_timeout_kills_owned_process_group(self):
        process = mock.Mock(pid=123, returncode=None)
        process.poll.return_value = None
        process.communicate.side_effect = [subprocess.TimeoutExpired("probe", 30), ("", "")]
        with mock.patch.dict(os.environ, {}, clear=True), \
             mock.patch.object(checker.subprocess, "Popen", return_value=process), \
             mock.patch.object(checker.os, "killpg") as kill:
            with self.assertRaisesRegex(ValueError, "exceeded 30 seconds"):
                checker.run_session()
            kill.assert_called_once_with(123, checker.signal.SIGKILL)


if __name__ == "__main__":
    unittest.main()
