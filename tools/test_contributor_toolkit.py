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
