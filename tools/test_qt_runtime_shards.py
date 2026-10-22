import re
import unittest
from collections import Counter
from pathlib import Path

import yaml

WORKFLOW = Path(__file__).resolve().parent.parent / ".github" / "workflows" / "reference.yml"
PAINT_SCALES = ["1", "1.25", "2", "1.3"]
OUTPUT_SCALES = ["1.25:2", "2:1.25"]
PAINT_CONDITION = "matrix.paint_scales != ''"
OUTPUT_CONDITION = "matrix.output_scales != ''"
EXPECTED_CHECKS = [
    "check_qt_command_control_accessibility.py",
    "check_qt_command_control_runtime.py",
    "check_qt_command_keyboard_runtime.py",
    "check_qt_command_label_runtime.py",
    "check_qt_command_motion_runtime.py",
    "check_qt_command_paint_runtime.py",
    "check_qt_command_pointer_runtime.py",
    "check_qt_command_runtime.py",
    "check_qt_command_snapshot_runtime.py",
    "check_qt_command_view_runtime.py",
    "check_qt_controller_lifetime.py",
    "check_qt_native_availability.py",
    "check_qt_platform_appearance.py",
    "check_qt_slider_accessibility_adapter.py",
    "check_qt_surface_view_runtime.py",
    "check_qt_toggle_control_runtime.py",
    "check_qt_toggle_motion_runtime.py",
    "check_qt_toggle_runtime.py",
    "check_quickshell_clock_shell.py",
    "check_quickshell_command_accessibility.py",
    "check_quickshell_command_lifecycle.py",
    "check_quickshell_command_motion.py",
    "check_quickshell_command_motion_budget.py",
    "check_quickshell_command_updates.py",
    "check_quickshell_label_render_runtime.py",
    "check_quickshell_label_runtime.py",
    "check_quickshell_toggle_lifecycle.py",
    "check_quickshell_toggle_updates.py",
]
STEP_TIMEOUTS = {
    "command-lifecycle": 35,
    "toggle-lifecycle": 15,
    "frame-timing": 20,
}
OUTPUT_SCALES_TIMEOUT = 30
FRAME_TIMING_RUNS = "for run in 1:1024x600: 2:1024x600: 6:3072x1800:--require-gap; do"
LINT_COMMANDS = ["cargo fmt", "cargo test", "cargo clippy"]
CHECK_INVOCATION = re.compile(r"tools/(check_\w+\.py)")
GROUP_CONDITION = re.compile(r"^contains\(matrix\.groups, '([a-z-]+)'\)$")


def load_jobs():
    with WORKFLOW.open(encoding="utf-8") as source:
        return yaml.safe_load(source)["jobs"]


class QtRuntimeShardTests(unittest.TestCase):
    def setUp(self):
        self.jobs = load_jobs()
        self.job = self.jobs["qt-runtime"]
        self.shards = self.job["strategy"]["matrix"]["include"]

    def gated_groups(self):
        groups = {}
        for step in self.job["steps"]:
            condition = step.get("if")
            match = GROUP_CONDITION.match(condition) if condition else None
            if match:
                groups.setdefault(match.group(1), []).append(step["name"])
        return groups

    def test_shard_names_are_unique(self):
        names = [shard["shard"] for shard in self.shards]
        self.assertEqual(len(names), len(set(names)))

    def test_every_group_runs_in_exactly_one_shard(self):
        placed = Counter(group for shard in self.shards for group in shard["groups"])
        self.assertEqual({group: 1 for group in self.gated_groups()}, dict(placed))

    def test_every_check_step_is_gated(self):
        for step in self.job["steps"]:
            if "tools/check_" not in step.get("run", ""):
                continue
            condition = step.get("if", "")
            self.assertTrue(
                GROUP_CONDITION.match(condition) or condition in (PAINT_CONDITION, OUTPUT_CONDITION),
                step["name"],
            )

    def test_paint_scales_are_partitioned_across_shards(self):
        scales = [scale for shard in self.shards for scale in shard["paint_scales"].split()]
        self.assertEqual(sorted(PAINT_SCALES), sorted(scales))
        paint = [step for step in self.job["steps"] if step.get("if") == PAINT_CONDITION]
        self.assertEqual(1, len(paint))
        self.assertIn("for scale in $PAINT_SCALES;", paint[0]["run"])
        self.assertEqual("${{ matrix.paint_scales }}", paint[0]["env"]["PAINT_SCALES"])

    def test_output_scales_are_partitioned_across_shards(self):
        pairs = [pair for shard in self.shards for pair in shard["output_scales"].split()]
        self.assertEqual(sorted(OUTPUT_SCALES), sorted(pairs))
        step = next(step for step in self.job["steps"] if step.get("if") == OUTPUT_CONDITION)
        self.assertIn("for scales in $OUTPUT_SCALES;", step["run"])
        self.assertEqual("${{ matrix.output_scales }}", step["env"]["OUTPUT_SCALES"])

    def test_evidence_is_saved_per_shard(self):
        upload = next(step for step in self.job["steps"] if step.get("uses", "").startswith("actions/upload-artifact"))
        self.assertEqual("qt-native-command-evidence-${{ matrix.shard }}", upload["with"]["name"])
        self.assertEqual("always()", upload["if"])

    def test_cache_is_written_only_on_master_by_one_shard_per_profile_set(self):
        cache = next(step for step in self.job["steps"] if step.get("uses", "").startswith("Swatinem/rust-cache"))
        self.assertEqual(
            "${{ github.ref == 'refs/heads/master' && matrix.save_rust_cache }}", cache["with"]["save-if"]
        )
        self.assertTrue(cache["with"]["key"].endswith("-debug-release${{ matrix.rust_cache_suffix }}"))
        savers = {shard["shard"]: shard["rust_cache_suffix"] for shard in self.shards if shard["save_rust_cache"]}
        self.assertEqual({"updates": "-lint", "output-scales-up": ""}, savers)
        for shard in self.shards:
            self.assertIsInstance(shard["save_rust_cache"], bool)
            if not shard["save_rust_cache"]:
                self.assertEqual("", shard["rust_cache_suffix"])

    def test_stable_check_requires_every_shard(self):
        result = self.jobs["qt-runtime-result"]
        self.assertEqual("qt-runtime", result["name"])
        self.assertEqual("qt-runtime", result["needs"])
        self.assertEqual("always()", result["if"])
        self.assertEqual(
            'test "$SHARDS_RESULT" = success', result["steps"][0]["run"]
        )
        self.assertEqual("${{ needs.qt-runtime.result }}", result["steps"][0]["env"]["SHARDS_RESULT"])

    def step_for_group(self, group):
        return next(step for step in self.job["steps"] if step.get("if") == f"contains(matrix.groups, '{group}')")

    def test_the_set_of_check_invocations_is_pinned(self):
        invoked = {name for step in self.job["steps"] for name in CHECK_INVOCATION.findall(step.get("run", ""))}
        self.assertEqual(sorted(EXPECTED_CHECKS), sorted(invoked))

    def test_lint_step_runs_format_test_and_clippy(self):
        run = self.step_for_group("lint")["run"]
        for command in LINT_COMMANDS:
            self.assertIn(command, run)
        self.assertIn("-D warnings", run)

    def test_frame_timing_keeps_its_runs_and_lives_in_one_shard(self):
        self.assertIn(FRAME_TIMING_RUNS, self.step_for_group("frame-timing")["run"])
        owners = [shard["shard"] for shard in self.shards if "frame-timing" in shard["groups"]]
        self.assertEqual(["output-scales-up"], owners)

    def test_long_steps_keep_their_timeouts(self):
        for group, minutes in STEP_TIMEOUTS.items():
            self.assertEqual(minutes, self.step_for_group(group)["timeout-minutes"], group)
        step = next(step for step in self.job["steps"] if step.get("if") == OUTPUT_CONDITION)
        self.assertEqual(OUTPUT_SCALES_TIMEOUT, step["timeout-minutes"])


if __name__ == "__main__":
    unittest.main()
