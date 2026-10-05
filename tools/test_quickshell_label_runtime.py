import subprocess
import unittest

from check_quickshell_label_runtime import check_log


class QuickshellLabelEvidenceTests(unittest.TestCase):
    def test_complete_runtime_evidence_passes(self):
        check_log(subprocess.CompletedProcess([], 0, "DEBUG qml: RESINA_LABEL_PASS 243 13 1.25\n", ""), 1.25)

    def test_wrong_scale_incomplete_duplicate_failed_and_missing_evidence_fail(self):
        good = "RESINA_LABEL_PASS 243 13 1.25\n"
        for output, error, code in (
            ("RESINA_LABEL_PASS 243 13 1\n", "", 0),
            ("RESINA_LABEL_PASS 242 13 1.25\n", "", 0),
            ("RESINA_LABEL_PASS 243 12 1.25\n", "", 0),
            (good + good, "", 0),
            (good, "RESINA_LABEL_FAIL missing height", 0),
            (good, "", 1),
            ("", "", 0),
        ):
            with self.subTest(output=output, error=error, code=code):
                with self.assertRaises(ValueError):
                    check_log(subprocess.CompletedProcess([], code, output, error), 1.25)


if __name__ == "__main__":
    unittest.main()
