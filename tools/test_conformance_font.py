import copy
import io
import tempfile
import unittest
from pathlib import Path
from unittest import mock
import zipfile

import conformance_font


class ConformanceFontTests(unittest.TestCase):
    def manifest(self, data):
        manifest = copy.deepcopy(conformance_font.MANIFEST)
        for master in manifest["masters"]:
            master["sha256"] = conformance_font.sha256(data[master["file"]])
        return manifest

    def test_pinned_masters_have_distinct_real_weights(self):
        weights = [master["weight"] for master in conformance_font.MANIFEST["masters"]]
        self.assertEqual(weights, [400, 500, 600])
        self.assertEqual(conformance_font.FAMILY, "Inter")
        self.assertTrue(conformance_font.MANIFEST["archive"]["url"].startswith("https://"))

    def test_directory_must_contain_each_pinned_master(self):
        data = {master["file"]: master["style"].encode() for master in conformance_font.MANIFEST["masters"]}
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font, "MANIFEST", self.manifest(data)):
            for name, content in data.items():
                (Path(directory) / name).write_bytes(content)
            records = conformance_font.masters(directory)
            self.assertEqual([(record["style"], record["weight"]) for record in records],
                             [("Regular", 400), ("Medium", 500), ("SemiBold", 600)])
            (Path(directory) / "Inter-Medium.otf").write_bytes(b"Regular")
            with self.assertRaisesRegex(ValueError, "not the pinned Inter 4.0 Medium master"):
                conformance_font.masters(directory)
            (Path(directory) / "Inter-Medium.otf").unlink()
            with self.assertRaisesRegex(ValueError, "Medium master is unavailable"):
                conformance_font.masters(directory)

    def test_archive_is_verified_before_extraction(self):
        payload = io.BytesIO()
        with zipfile.ZipFile(payload, "w") as bundle:
            for master in conformance_font.MANIFEST["masters"]:
                bundle.writestr(master["archivePath"], master["style"])
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "pinned SHA-256"):
                conformance_font.extract(payload.getvalue(), directory)
            self.assertEqual(list(Path(directory).iterdir()), [])
            data = {master["file"]: master["style"].encode() for master in conformance_font.MANIFEST["masters"]}
            manifest = self.manifest(data)
            manifest["archive"]["sha256"] = conformance_font.sha256(payload.getvalue())
            with mock.patch.object(conformance_font, "MANIFEST", manifest):
                records = conformance_font.extract(payload.getvalue(), directory)
            self.assertEqual([record["file"] for record in records],
                             ["Inter-Regular.otf", "Inter-Medium.otf", "Inter-SemiBold.otf"])

    def test_resolved_master_must_have_the_requested_weight(self):
        records = [{"family": "Inter", "style": master["style"], "weight": master["weight"],
                    "file": master["file"], "sha256": master["sha256"]}
                   for master in conformance_font.MANIFEST["masters"]]
        resolved = conformance_font.resolve(records, "Inter", "Medium", 500)
        self.assertEqual(resolved, {"family": "Inter", "style": "Medium", "weight": 500,
                                    "file": "Inter-Medium.otf", "sha256": records[1]["sha256"]})
        with self.assertRaisesRegex(ValueError, "weight 500 resolved to the Inter Regular master .*substituted"):
            conformance_font.resolve(records, "Inter", "Regular", 500)
        with self.assertRaisesRegex(ValueError, "not a registered conformance master"):
            conformance_font.resolve(records, "Inter", "Bold", 700)
        with self.assertRaisesRegex(ValueError, "not a registered conformance master"):
            conformance_font.resolve(records, "DejaVu Sans", "Book", 400)

    def test_isolated_fontconfig_lists_no_font_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            environment = conformance_font.isolated_fontconfig(directory)
            text = Path(environment["FONTCONFIG_FILE"]).read_text(encoding="utf-8")
            self.assertNotIn("<dir", text)
            self.assertIn("<cachedir>", text)


if __name__ == "__main__":
    unittest.main()
