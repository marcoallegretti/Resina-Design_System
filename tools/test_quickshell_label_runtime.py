import json
import subprocess
import unittest

from check_quickshell_label_runtime import check_log, check_substitution
import conformance_font


MASTERS = [{"family": "Inter", "style": master["style"], "weight": master["weight"], "file": master["file"],
            "sha256": master["sha256"]} for master in conformance_font.MANIFEST["masters"]]
TABULAR = [12.953125] * 10
PROPORTIONAL = [12.90625, 8.296875, 12.3125, 12.53125, 13.125, 12.359375, 12.59375, 11.421875, 12.578125, 12.59375]


def faces(styles):
    return "".join(f"RESINA_LABEL_FACE {json.dumps({'weight': weight, 'face': {'family': 'Inter', 'style': style}})}\n"
                   for weight, style in zip((400, 500, 600), styles))


def digits(tabular=TABULAR, default=PROPORTIONAL):
    return "".join(
        "RESINA_LABEL_DIGITS " + json.dumps({"tabular": flag, "record": {
            "schemaVersion": "0.1.0", "role": "numeric", "fontSize": {"value": 20, "unit": "px"},
            "advances": advances}}) + "\n"
        for flag, advances in ((True, tabular), (False, default)))


GOOD_FACES = faces(("Regular", "Medium", "SemiBold")) + digits()


class QuickshellLabelEvidenceTests(unittest.TestCase):
    def test_complete_runtime_evidence_records_resolved_masters_and_digits(self):
        evidence = check_log(subprocess.CompletedProcess(
            [], 0, "DEBUG qml: " + GOOD_FACES + "DEBUG qml: RESINA_LABEL_PASS 432 13 1.25\n", ""), 1.25, MASTERS)
        self.assertEqual([(master["style"], master["weight"], master["file"]) for master in evidence["masters"]],
                         [("Regular", 400, "Inter-Regular.otf"), ("Medium", 500, "Inter-Medium.otf"),
                          ("SemiBold", 600, "Inter-SemiBold.otf")])
        self.assertEqual(evidence["digits"]["tabular"]["outcome"], {"advance": 12.953125})
        self.assertEqual(evidence["digits"]["default"]["outcome"],
                         {"error": {"kind": "unequalAdvances", "narrowest": 1, "widest": 4}})

    def test_wrong_scale_incomplete_duplicate_failed_and_missing_evidence_fail(self):
        good = GOOD_FACES + "RESINA_LABEL_PASS 432 13 1.25\n"
        regular = faces(("Regular", "Medium", "SemiBold"))
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
            (faces(("Regular", "Medium")) + digits() + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (faces(("Regular", "Regular", "SemiBold")) + digits() + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (faces(("Regular", "Medium", "Bold")) + digits() + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (regular + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (regular + digits(tabular=PROPORTIONAL) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (regular + digits(default=TABULAR) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
            (regular + digits(tabular=TABULAR[:9]) + "RESINA_LABEL_PASS 432 13 1.25\n", "", 0),
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
