import copy
import unittest

from check_schemas import ROOT, check_command_content, load_json, validator_for


def public_cases():
    return (
        load_json(ROOT / "conformance/content/command-content-vectors.json"),
        load_json(ROOT / "conformance/accessibility/command-content-cases.json"),
    )


def case(cases, name):
    return next(entry for entry in cases if entry["name"] == name)


class CommandContentAccessibilityTests(unittest.TestCase):
    def test_public_cases_pass(self):
        self.assertGreater(check_command_content(*public_cases()), 0)

    def test_name_taken_from_icon_is_rejected(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        entry = case(changed, "icon only")
        entry["expected"]["name"] = entry["input"]["content"]["icon"]["reference"]
        with self.assertRaisesRegex(AssertionError, "icon only: name is not the name the inputs determine"):
            check_command_content(vectors, changed)

    def test_description_taken_from_icon_is_rejected(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        entry = case(changed, "icon only")
        entry["expected"]["description"] = entry["input"]["content"]["icon"]["reference"]
        with self.assertRaisesRegex(
            AssertionError, "icon only: description is not the description the inputs determine"
        ):
            check_command_content(vectors, changed)

    def test_label_equal_to_reference_stays_supplied_text(self):
        vectors, cases = public_cases()
        entry = case(cases, "icon only label equal to icon reference")
        self.assertEqual(entry["expected"]["name"], entry["input"]["content"]["icon"]["reference"])
        self.assertGreater(check_command_content(vectors, cases), 0)

    def test_description_equal_to_reference_stays_supplied_text(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        entry = case(changed, "icon only")
        reference = entry["input"]["content"]["icon"]["reference"]
        entry["input"]["description"] = reference
        entry["expected"]["description"] = reference
        self.assertGreater(check_command_content(vectors, changed), 0)

    def test_label_form_snapshot_is_rejected(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        entry = case(changed, "label form")
        del entry["error"]
        entry["expected"] = copy.deepcopy(case(cases, "icon and label")["expected"])
        entry["expected"]["name"] = entry["input"]["content"]["label"]
        with self.assertRaisesRegex(AssertionError, "label form: expected error LabelForm"):
            check_command_content(vectors, changed)

    def test_icon_and_label_without_layout_is_rejected(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        case(changed, "icon and label")["input"]["laidOutLabel"] = None
        with self.assertRaisesRegex(AssertionError, "icon and label: expected error MissingLabelLayout"):
            check_command_content(vectors, changed)

    def test_layout_of_other_text_is_rejected(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        case(changed, "icon and label")["input"]["laidOutLabel"] = "Forward"
        with self.assertRaisesRegex(AssertionError, "icon and label: expected error LabelLayoutMismatch"):
            check_command_content(vectors, changed)

    def test_layout_error_does_not_replace_snapshot_error(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(cases)
        case(changed, "icon only blank description")["error"] = "UnexpectedLabelLayout"
        with self.assertRaisesRegex(AssertionError, "icon only blank description: expected error BlankDescription"):
            check_command_content(vectors, changed)

    def test_every_form_is_required(self):
        vectors, cases = public_cases()
        changed = [entry for entry in cases if entry["input"]["content"]["form"] != "label"]
        with self.assertRaisesRegex(AssertionError, "cover only"):
            check_command_content(vectors, changed)

    def test_every_error_is_required(self):
        vectors, cases = public_cases()
        changed = [entry for entry in cases if entry.get("error") != "LabelLayoutMismatch"]
        with self.assertRaisesRegex(AssertionError, r"omit \['LabelLayoutMismatch'\]"):
            check_command_content(vectors, changed)

    def test_content_schema_rejects_each_missing_or_foreign_label(self):
        validator = validator_for("schemas/command-content.schema.json")
        vectors, _ = public_cases()
        document = case(vectors, "icon only")["document"]
        self.assertTrue(validator.is_valid(document))
        for member, value in (("accessibleLabel", None), ("accessibleLabel", ""), ("label", "Mute")):
            changed = copy.deepcopy(document)
            changed[member] = value
            with self.subTest(member=member, value=value):
                self.assertFalse(validator.is_valid(changed))
        changed = copy.deepcopy(document)
        del changed["accessibleLabel"]
        self.assertFalse(validator.is_valid(changed))
        changed["accessibleLabel"] = "Mute"
        changed["form"] = "iconAndLabel"
        self.assertFalse(validator.is_valid(changed))

    def test_schema_valid_vector_outcome_is_checked(self):
        vectors, cases = public_cases()
        changed = copy.deepcopy(vectors)
        case(changed, "icon only without accessible label")["schemaValid"] = True
        with self.assertRaisesRegex(AssertionError, "icon only without accessible label"):
            check_command_content(changed, cases)


if __name__ == "__main__":
    unittest.main()
