import contextlib
import io
import json
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from check_maintenance_issue import (
    LANE_FORMS,
    ROOT,
    fetch_issue_titles,
    form_labels,
    issue_errors,
    lane_from_workflow_ref,
    line_count,
    main,
    title_prefix,
)


MAINTENANCE_LABELS = form_labels(ROOT / ".github/ISSUE_TEMPLATE/maintenance.yml")
REFERENCE_LINES = line_count(ROOT / ".github/workflows/reference.yml")


def body(sections):
    return "\n\n".join(f"### {label}\n\n{text}" for label, text in sections)


def valid_sections():
    return [
        ("Observed", "AGENTS.md and .agents/agents/continuous-simplicity.md describe the lane."),
        ("Expected", "One workflow definition owns the test matrix."),
        ("Evidence", f"`.github/workflows/reference.yml:1-{REFERENCE_LINES}` and `tools/check_maintenance_issue.py`."),
        ("Affected layer or boundary", "Contributor validation under `tools/`."),
        ("Why it matters now", "Contributors edit two places for one change."),
        ("Acceptance criterion", "The matrix is declared once."),
        ("Expected proof", "python -m unittest discover -s tools -p 'test_*.py'"),
    ]


def errors(title="Declare the test matrix once", sections=None, existing=()):
    return issue_errors(
        {"type": "create_issue", "title": title, "body": body(sections or valid_sections())},
        labels=MAINTENANCE_LABELS,
        prefix="[simplicity] ",
        root=ROOT,
        existing_titles=list(existing),
    )


def replace(label, text):
    return [(name, text if name == label else value) for name, value in valid_sections()]


class MaintenanceIssueGateTests(unittest.TestCase):
    def test_issue_following_the_form_with_real_citations_is_accepted(self):
        self.assertEqual(errors(existing=["[simplicity] Remove the unused slider fixture"]), [])

    def test_fields_beyond_title_and_body_are_rejected(self):
        for field, value in (
            ("labels", ["ai-generated"]),
            ("parent", 42),
            ("blocked_by", [7]),
            ("fields", [{"name": "Status"}]),
            ("repo", "owner/other"),
            ("temporary_id", "aw_issue1"),
        ):
            with self.subTest(field=field):
                issue = {"type": "create_issue", "title": "Declare the test matrix once", "body": body(valid_sections()), field: value}
                result = issue_errors(issue, labels=MAINTENANCE_LABELS, prefix="", root=ROOT, existing_titles=[])
                self.assertIn(f"issue sets {field}; only title and body may be proposed", result)

    def test_forms_supply_the_required_headings(self):
        self.assertEqual(
            MAINTENANCE_LABELS,
            [
                "Observed",
                "Expected",
                "Evidence",
                "Affected layer or boundary",
                "Why it matters now",
                "Acceptance criterion",
                "Expected proof",
            ],
        )
        architecture = form_labels(ROOT / ".github/ISSUE_TEMPLATE/architecture.yml")
        self.assertIn("Alternatives and trade-offs", architecture)
        self.assertIn("Maintainer decision requested", architecture)

    def test_missing_reordered_renamed_or_extra_headings_are_rejected(self):
        sections = valid_sections()
        cases = {
            "missing": sections[:-1],
            "reordered": [sections[1], sections[0]] + sections[2:],
            "renamed": [("Observation", sections[0][1])] + sections[1:],
            "extra": sections + [("Notes", "More text.")],
        }
        for name, case in cases.items():
            with self.subTest(name=name):
                self.assertTrue(any("headings must be exactly" in error for error in errors(sections=case)))

    def test_wrong_heading_level_and_leading_text_are_rejected(self):
        wrong_level = body(valid_sections()).replace("### Evidence", "## Evidence")
        issue = {"title": "Declare the test matrix once", "body": wrong_level}
        result = issue_errors(issue, labels=MAINTENANCE_LABELS, prefix="", root=ROOT, existing_titles=[])
        self.assertTrue(any("headings must be exactly" in error for error in result))
        issue = {"title": "Declare the test matrix once", "body": "Summary first.\n\n" + body(valid_sections())}
        result = issue_errors(issue, labels=MAINTENANCE_LABELS, prefix="", root=ROOT, existing_titles=[])
        self.assertIn("body must start with the first form heading", result)

    def test_empty_section_is_rejected(self):
        self.assertIn("section 'Expected' is empty", errors(sections=replace("Expected", "")))

    def test_heading_inside_code_fence_does_not_count(self):
        text = "Observed text.\n\n```\n### Not a heading\n```"
        self.assertEqual(errors(sections=replace("Observed", text)), [])

    def test_nonexistent_path_is_rejected(self):
        for citation in ("`tools/check_missing_lane.py`", "spec/missing-contract.md", "lib.rs"):
            with self.subTest(citation=citation):
                result = errors(sections=replace("Evidence", f"See {citation}."))
                self.assertTrue(any("does not exist" in error for error in result), result)

    def test_line_range_beyond_the_file_is_rejected(self):
        for citation in (
            f".github/workflows/reference.yml:{REFERENCE_LINES + 1}",
            f".github/workflows/reference.yml#L10-L{REFERENCE_LINES + 5}",
            ".github/workflows/reference.yml:20-10",
        ):
            with self.subTest(citation=citation):
                result = errors(sections=replace("Evidence", f"See {citation}."))
                self.assertTrue(any("outside the file" in error for error in result), result)

    def test_line_range_on_a_directory_is_rejected(self):
        result = errors(sections=replace("Evidence", "See tools/:4."))
        self.assertTrue(any("not a file" in error for error in result), result)

    def test_plain_words_with_slashes_are_not_paths(self):
        self.assertEqual(errors(sections=replace("Expected", "Input and/or output stay unchanged.")), [])

    def test_description_of_the_filing_process_is_rejected(self):
        phrases = [
            "An agent found this.",
            "Found by Copilot.",
            "Proposed by AI.",
            "An LLM summarised it.",
            "A language model wrote it.",
            "Created with gh-aw.",
            "Raised by automation.",
            "Reported by this run.",
            "The maintenance workflow found it.",
            "Claude reviewed it.",
            "Generated automatically.",
            "A bot filed it.",
            "An assistant drafted it.",
            "An A.I. drafted it.",
            "Artificial intelligence drafted it.",
            "DeepSeek agrees.",
            "Grok agrees.",
            "Clаude reviewed it.",
            "Cl​aude reviewed it.",
            "Ｃlaude reviewed it.",
            "Checked by self-review.",
        ]
        for phrase in phrases:
            with self.subTest(phrase=phrase):
                result = errors(sections=replace("Observed", phrase))
                self.assertTrue(any(error.startswith("mentions ") for error in result), result)

    def test_ordinary_uses_of_nearby_words_are_allowed(self):
        text = "Consumers must not upgrade the schema version automatically; generated output is never normative."
        self.assertEqual(errors(sections=replace("Observed", text)), [])

    def test_process_terms_in_the_title_are_rejected(self):
        self.assertTrue(any(error.startswith("mentions ") for error in errors(title="Agent duplicates resolver IO")))

    def test_citing_agent_files_and_ci_workflows_is_allowed(self):
        text = "AGENTS.md, .agents/agents/reviewer.md and .github/workflows/reference.yml run the workflow checks."
        self.assertEqual(errors(sections=replace("Observed", text)), [])

    def test_existing_title_is_rejected_ignoring_prefix_case_and_spacing(self):
        for existing in (
            "[simplicity] Declare the test matrix once",
            "declare  the test matrix once.",
            "[simplicity] DECLARE THE TEST MATRIX ONCE",
        ):
            with self.subTest(existing=existing):
                result = errors(title="[simplicity] Declare the test matrix once", existing=[existing])
                self.assertTrue(any("duplicates existing issue" in error for error in result), result)

    def test_every_maintenance_workflow_has_a_form_and_title_prefix(self):
        workflows = ROOT / ".github/workflows"
        importing = sorted(
            path.stem
            for path in workflows.glob("*.md")
            if "shared/maintenance-protocol.md" in path.read_text(encoding="utf-8")
        )
        self.assertEqual(importing, sorted(LANE_FORMS))
        for lane in importing:
            with self.subTest(lane=lane):
                self.assertTrue((ROOT / LANE_FORMS[lane]).is_file())
                self.assertTrue(title_prefix(workflows / f"{lane}.md").startswith("["))

    def test_lane_is_read_from_the_workflow_ref(self):
        ref = "owner/repo/.github/workflows/continuous-simplicity.lock.yml@refs/heads/master"
        self.assertEqual(lane_from_workflow_ref(ref), "continuous-simplicity")
        with self.assertRaises(ValueError):
            lane_from_workflow_ref("owner/repo/.github/workflows/reference.yml@refs/heads/master")

    def test_output_without_a_proposed_issue_passes_and_two_issues_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "agent_output.json"
            output.write_text(json.dumps({"items": [{"type": "noop", "message": "Nothing qualifies."}]}), encoding="utf-8")
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(main(["--agent-output", str(output), "--workflow-ref", "", "--repository", ""]), 0)
            issue = {"type": "create_issue", "title": "One", "body": body(valid_sections())}
            output.write_text(json.dumps({"items": [issue, issue]}), encoding="utf-8")
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                self.assertEqual(main(["--agent-output", str(output), "--workflow-ref", "", "--repository", ""]), 1)
            self.assertIn("2 issues proposed", printed.getvalue())

    def test_issue_listing_follows_pages_and_skips_pull_requests(self):
        pages = [
            ([{"title": "First"}, {"title": "A change", "pull_request": {}}], '<https://api.github.com/page2>; rel="next"'),
            ([{"title": "Second"}], ""),
        ]

        def respond(request, timeout):
            items, link = pages.pop(0)
            response = mock.MagicMock()
            response.__enter__.return_value = response
            response.read.return_value = json.dumps(items).encode()
            response.headers = {"Link": link}
            return response

        with mock.patch("urllib.request.urlopen", side_effect=respond):
            self.assertEqual(fetch_issue_titles("owner/repo", "token"), ["First", "Second"])

    def test_unavailable_issue_listing_fails_closed(self):
        error = urllib.error.HTTPError("https://api.github.com", 403, "Forbidden", {}, io.BytesIO())
        with mock.patch("urllib.request.urlopen", side_effect=error):
            with self.assertRaises(RuntimeError):
                fetch_issue_titles("owner/repo", "token")

    def test_missing_agent_output_fails(self):
        with self.assertRaises(FileNotFoundError):
            main(["--agent-output", str(ROOT / "missing-agent-output.json")])


if __name__ == "__main__":
    unittest.main()
