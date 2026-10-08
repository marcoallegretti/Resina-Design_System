import contextlib
import importlib.util
import io
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from check_contributor_toolkit import check


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / ".agents/scripts/verify.py"
SPEC = importlib.util.spec_from_file_location("contributor_verify", SCRIPT)
verify = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(verify)


class ToolkitLinks(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name in ("AGENTS.md", "CONTRIBUTING.md", "README.md"):
            (self.root / name).write_text("# Contract\n", encoding="utf-8")
        skill = self.root / ".agents/skills/example/SKILL.md"
        skill.parent.mkdir(parents=True)
        skill.write_text("---\nname: example\ndescription: An example skill.\n---\n"
                         "[Contract](../../../AGENTS.md)\n", encoding="utf-8")
        self.skill = skill
        self.catalog = self.root / ".agents/README.md"
        self.catalog.write_text("[Example](skills/example/SKILL.md)\n", encoding="utf-8")

    def test_valid_toolkit(self):
        self.assertEqual(check(self.root), [])

    def test_deleted_contract_is_rejected(self):
        (self.root / "AGENTS.md").unlink()
        self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_path_escape_is_rejected(self):
        with self.skill.open("a", encoding="utf-8") as source:
            source.write("[Outside](../../../../README.md)\n")
        self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_missing_metadata_is_rejected(self):
        self.skill.write_text("# No metadata\n", encoding="utf-8")
        self.assertTrue(any("missing front matter" in error for error in check(self.root)))

    def test_mismatched_skill_name_is_rejected(self):
        source = self.skill.read_text(encoding="utf-8").replace("name: example", "name: another")
        self.skill.write_text(source, encoding="utf-8")
        self.assertTrue(any("name must match path" in error for error in check(self.root)))

    def test_empty_description_is_rejected(self):
        source = self.skill.read_text(encoding="utf-8").replace("description: An example skill.", "description:")
        self.skill.write_text(source, encoding="utf-8")
        self.assertTrue(any("description must be nonempty" in error for error in check(self.root)))

    def test_uncatalogued_skill_is_rejected(self):
        self.catalog.write_text("# Empty catalog\n", encoding="utf-8")
        self.assertTrue(any("absent from toolkit catalog" in error for error in check(self.root)))

    def test_literal_examples_do_not_create_links(self):
        source = self.skill.read_text(encoding="utf-8")
        examples = [
            '`[Example](missing.md)`',
            '``Use `[Example](missing.md)` here``',
            '```md\n[Example](missing.md)\n```',
            '~~~md\n[Example](missing.md)\n~~~',
            '````md\n```\n[Example](missing.md)\n`````',
            '<!-- [Example](missing.md) -->',
            '[Example]`code`(missing.md)',
            r'\[Example](missing.md)',
        ]
        for example in examples:
            with self.subTest(example=example):
                self.skill.write_text(source + example + "\n", encoding="utf-8")
                self.assertEqual(check(self.root), [])

    def test_escaped_ticks_and_invalid_fences_do_not_hide_real_links(self):
        source = self.skill.read_text(encoding="utf-8")
        for example in [
            r'\`[Example](missing.md)\`',
            '```invalid`info\n[Example](missing.md)\n```',
        ]:
            with self.subTest(example=example):
                self.skill.write_text(source + example + "\n", encoding="utf-8")
                self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_titled_and_angle_links_check_actual_targets(self):
        source = self.skill.read_text(encoding="utf-8")
        for template in [
            '[`Contract`]({path})',
            '[Contract]({path} "Title")',
            "[Contract]({path} 'Title')",
            '[Contract]({path} (Title))',
            '[Contract](<{path}> "Title")',
        ]:
            with self.subTest(template=template):
                self.skill.write_text(source + template.format(path="../../../AGENTS.md") + "\n",
                                      encoding="utf-8")
                self.assertEqual(check(self.root), [])
                self.skill.write_text(source + template.format(path="missing.md") + "\n",
                                      encoding="utf-8")
                self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_empty_destinations_do_not_crash_or_catalog_a_document(self):
        source = self.skill.read_text(encoding="utf-8")
        for link in ['[Empty](<>)', '[Empty](<> "Title")', '[Empty]()', '![](<>)']:
            with self.subTest(link=link):
                self.skill.write_text(source + link + "\n", encoding="utf-8")
                self.assertEqual(check(self.root), [])
                self.catalog.write_text(link + "\n", encoding="utf-8")
                self.assertTrue(any("absent from toolkit catalog" in error for error in check(self.root)))
                self.catalog.write_text('[Example](skills/example/SKILL.md)\n', encoding="utf-8")

    def test_empty_text_images_and_links_check_actual_targets(self):
        source = self.skill.read_text(encoding="utf-8")
        for template in ['![]({path})', '![](<{path}> "Title")', '[]({path})']:
            with self.subTest(template=template):
                self.skill.write_text(source + template.format(path="../../../AGENTS.md") + "\n",
                                      encoding="utf-8")
                self.assertEqual(check(self.root), [])
                self.skill.write_text(source + template.format(path="missing.png") + "\n",
                                      encoding="utf-8")
                self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_comment_and_title_delimiters_do_not_hide_real_links(self):
        source = self.skill.read_text(encoding="utf-8")
        for example in [
            '<!-- ` -->\n[Missing](missing.md)\n`',
            '<!--\n```\n-->\n[Missing](missing.md)',
            '[Missing](missing.md "`Title")\n`',
            '`<!--`\n[Missing](missing.md)\n-->',
            '```text\n<!--\n```\n[Missing](missing.md)\n-->',
            '[Missing](missing.md "<!--")\n-->',
        ]:
            with self.subTest(example=example):
                self.skill.write_text(source + example + "\n", encoding="utf-8")
                self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_reference_links_resolve_in_each_document(self):
        source = self.skill.read_text(encoding="utf-8")
        self.catalog.write_text('[Example][skill]\n\n[skill]: skills/example/SKILL.md\n', encoding="utf-8")
        self.skill.write_text(source + '[Contract][doc]\n\n[doc]: ../../../AGENTS.md\n', encoding="utf-8")
        self.assertEqual(check(self.root), [])
        self.skill.write_text(source + '[Missing][doc]\n\n[doc]: missing.md\n', encoding="utf-8")
        self.assertTrue(any("broken local link" in error for error in check(self.root)))

    def test_catalog_requires_a_visible_link(self):
        for entry in [
            'skills/example/SKILL.md',
            '![Example](skills/example/SKILL.md)',
            '<!-- [Example](skills/example/SKILL.md) -->',
            '`[Example](skills/example/SKILL.md)`',
            '```md\n[Example](skills/example/SKILL.md)\n```',
        ]:
            with self.subTest(entry=entry):
                self.catalog.write_text(entry + "\n", encoding="utf-8")
                self.assertTrue(any("absent from toolkit catalog" in error for error in check(self.root)))

    def test_catalog_links_can_have_titles_fragments_and_normalized_paths(self):
        for entry in [
            '[Example](skills/example/SKILL.md "Example")',
            '[![Example](../AGENTS.md)](skills/example/SKILL.md)',
            '[Example](<skills/example/SKILL.md>)',
            '[Example](skills/../skills/example/SKILL.md#example)',
        ]:
            with self.subTest(entry=entry):
                self.catalog.write_text(entry + "\n", encoding="utf-8")
                self.assertEqual(check(self.root), [])

    def test_invalid_metadata_values_and_duplicate_fields_are_rejected(self):
        for fields in [
            'name: example\ndescription:\nlicense: MIT',
            'name: example\ndescription: ""',
            "name: example\ndescription: ''",
            'name: example\ndescription: "  "',
            'name: example\ndescription: # A comment',
            'name: example\ndescription: [unterminated',
            'name: example\ndescription: "unterminated',
            'name: example\ndescription: null',
            'name: example\ndescription: true',
            'name: example\ndescription: 42',
            'name: wrong\nname: example\ndescription: Example skill.',
            'name: example\ndescription: First\ndescription: Second',
            'name: " example "\ndescription: Example',
            'name: example\ndescription: {text: Example}',
            'name: example\ndescription: [Example]',
            'name: example\ndescription: 0x42',
            'name: example\ndescription: !!int 42',
        ]:
            with self.subTest(fields=fields):
                self.skill.write_text("---\n" + fields + "\n---\n", encoding="utf-8")
                self.assertTrue(any("name must match path" in error for error in check(self.root)))

    def test_single_line_quoted_metadata_is_accepted(self):
        for fields in [
            'name: "example"\ndescription: "Example skill."',
            "name: 'example'\ndescription: 'Contributor''s skill.'",
            'name: example\ndescription: "Use \\"quoted\\" names."',
            'name: example\ndescription: Example skill. # A comment',
        ]:
            with self.subTest(fields=fields):
                self.skill.write_text("---\n" + fields + "\n---\n", encoding="utf-8")
                self.assertEqual(check(self.root), [])

    def test_yaml_block_and_tagged_strings_are_accepted(self):
        for description in [
            '|\n  Example skill.\n  Load for documentation.',
            '>\n  Example skill.\n  Load for documentation.',
            '!!str true',
        ]:
            with self.subTest(description=description):
                self.skill.write_text('---\nname: example\ndescription: ' + description + '\n---\n',
                                      encoding="utf-8")
                self.assertEqual(check(self.root), [])


class VerificationRunner(unittest.TestCase):
    def test_dry_run_from_another_directory_never_executes(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "baseline", "schemas", "--dry-run"],
                cwd=directory, capture_output=True, text=True, check=False,
            )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("cargo test --workspace --locked", result.stdout)
        self.assertIn("tools/check_schemas.py", result.stdout)

    def test_failure_stops_remaining_checks_and_preserves_exit_code(self):
        with patch.object(verify.subprocess, "run") as run, contextlib.redirect_stdout(io.StringIO()):
            run.return_value = subprocess.CompletedProcess([], 7)
            self.assertEqual(verify.main(["baseline", "schemas"]), 7)
            self.assertEqual(run.call_count, 1)
            self.assertEqual(run.call_args.kwargs["cwd"], ROOT)

    def test_missing_executable_is_failure(self):
        with patch.object(verify.subprocess, "run", side_effect=FileNotFoundError("missing")), \
                contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(verify.main(["baseline"]), 1)

    def test_unknown_profile_is_rejected_before_execution(self):
        with patch.object(verify.subprocess, "run") as run, contextlib.redirect_stderr(io.StringIO()):
            with self.assertRaises(SystemExit) as error:
                verify.main(["unknown"])
            self.assertEqual(error.exception.code, 2)
            run.assert_not_called()

    def test_dry_run_does_not_run_commands(self):
        with patch.object(verify.subprocess, "run") as run, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(verify.main(["toolkit", "--dry-run"]), 0)
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
