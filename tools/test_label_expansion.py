import copy
import unittest

from check_schemas import ROOT, check_label_expansion, load_json


class LabelExpansionTests(unittest.TestCase):
    def setUp(self):
        self.document = load_json(ROOT / "conformance/content/command-label-expansion.json")

    def test_public_matrix_has_independent_length_coverage(self):
        self.assertEqual(check_label_expansion(self.document), 4)
        integral_number = copy.deepcopy(self.document)
        integral_number["cases"][1]["scalarLength"] = 13.0
        self.assertEqual(check_label_expansion(integral_number), 4)

    def test_missing_duplicated_blank_miscounted_and_outlying_content_fails(self):
        cases = []
        missing = copy.deepcopy(self.document)
        missing["cases"].pop()
        cases.append(missing)
        for field in ["name", "targetExpansion"]:
            duplicate = copy.deepcopy(self.document)
            duplicate["cases"][1][field] = duplicate["cases"][0][field]
            cases.append(duplicate)
        blank = copy.deepcopy(self.document)
        blank["baselineText"] = " "
        cases.append(blank)
        miscounted = copy.deepcopy(self.document)
        miscounted["cases"][1]["scalarLength"] = 14
        cases.append(miscounted)
        outlying = copy.deepcopy(self.document)
        outlying["cases"][1]["text"] = "Reconnect to the network now"
        outlying["cases"][1]["scalarLength"] = len(outlying["cases"][1]["text"])
        cases.append(outlying)
        trailing_newline = copy.deepcopy(self.document)
        trailing_newline["cases"][1]["name"] += "\n"
        cases.append(trailing_newline)
        non_ascii = copy.deepcopy(self.document)
        non_ascii["cases"][1]["text"] = "إعادة الاتصال"
        non_ascii["cases"][1]["scalarLength"] = len(non_ascii["cases"][1]["text"])
        cases.append(non_ascii)
        unsupported = dict(self.document, schemaVersion="0.2.0")
        cases.append(unsupported)
        extra = dict(self.document, renderer="guido")
        cases.append(extra)
        for case in cases:
            with self.subTest(case=case), self.assertRaises((ValueError, AssertionError)):
                check_label_expansion(case)


if __name__ == "__main__":
    unittest.main()
