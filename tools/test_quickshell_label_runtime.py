import json
import subprocess
import unittest

from check_quickshell_label_runtime import check_log, check_substitution
import conformance_font


MASTERS = [{"family": "Inter", "style": master["style"], "weight": master["weight"], "file": master["file"],
            "sha256": master["sha256"]} for master in conformance_font.MANIFEST["masters"]]


def faces(styles):
    return "".join(f"RESINA_LABEL_FACE {json.dumps({'weight': weight, 'face': {'family': 'Inter', 'style': style}})}\n"
                   for weight, style in zip((400, 500, 600), styles))


GOOD_FACES = faces(("Regular", "Medium", "SemiBold"))


class QuickshellLabelEvidenceTests(unittest.TestCase):
    def test_complete_runtime_evidence_records_resolved_masters(self):
        resolved = check_log(subprocess.CompletedProcess(
            [], 0, "DEBUG qml: " + GOOD_FACES + "DEBUG qml: RESINA_LABEL_PASS 432 13 1.25\n", ""), 1.25, MASTERS)
        self.assertEqual([(master["style"], master["weight"], master["file"]) for master in resolved],
                         [("Regular", 400, "Inter-Regular.otf"), ("Medium", 500, "Inter-Medium.otf"),
                          ("SemiBold", 600, "Inter-SemiBold.otf")])

    def test_wrong_scale_incomplete_duplicate_failed_and_missing_evidence_fail(self):
        good = GOOD_FACES + "RESINA_LABEL_PASS 432 13 1.25\n"
        for output, error, code in (
            (GOOD_FACES + "RESINA_LABEL_PASS 432 13 1\n", "", 0),
            (GOOD_FACES + "RESINA_LABEL_PASS 431 13 1.25\n", "", 0),
            (GOOD_FACES + "RESINA_LABEL_PASS 432 12 1.25\n", "", 0),
            (good + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (good, "RESINA_LABEL_FAIL missing height", 0),
            (good, "", 1),
            ("", "", 0),
            ("RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (GOOD_FACES + good, "", 0),
            (faces(("Regular", "Medium")) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (faces(("Regular", "Regular", "SemiBold")) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (faces(("Regular", "Medium", "Bold")) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
        ):
            with self.subTest(output=output, error=error, code=code):
                with self.assertRaises(ValueError):
                    check_log(subprocess.CompletedProcess([], code, output, error), 1.25, MASTERS)

    def test_regular_only_registration_must_be_reported_as_substitution(self):
        regular = faces(("Regular", "Regular", "Regular")) + "RESINA_LABEL_FACES_PASS 1\n"
        self.assertIn("weight 500 resolved to the Inter Regular master",
                      check_substitution(subprocess.CompletedProcess([], 0, regular, ""), MASTERS))
        for output, code in ((GOOD_FACES + "RESINA_LABEL_FACES_PASS 1\n", 0),
                             (faces(("Regular", "Regular", "Regular")), 0), (regular, 1),
                             (regular + "RESINA_LABEL_FAIL font", 0),
                             (faces(("Regular", "Book", "Book")) + "RESINA_LABEL_FACES_PASS 1\n", 0)):
            with self.subTest(output=output, code=code):
                with self.assertRaises(ValueError):
                    check_substitution(subprocess.CompletedProcess([], code, output, ""), MASTERS)


if __name__ == "__main__":
    unittest.main()
