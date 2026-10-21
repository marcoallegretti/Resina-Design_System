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


if __name__ == "__main__":
    unittest.main()
