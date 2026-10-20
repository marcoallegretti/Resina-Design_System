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

    def test_visible_unpinned_family_face_is_rejected(self):
        data = {master["file"]: master["style"].encode() for master in conformance_font.MANIFEST["masters"]}
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font, "MANIFEST", self.manifest(data)):
            for name, content in data.items():
                (Path(directory) / name).write_bytes(content)
            records = conformance_font.masters(directory)
            other = Path(directory) / "Inter.ttc"
            other.write_bytes(b"Inter 4.1 collection")

            def faces(file, weight, slant=0, width=100, index=0):
                return conformance_font.parse_fontconfig_faces(f"{file}\t{index}\t{weight}\t{slant}\t{width}\n")

            pinned = [face for name, weight in zip(data, (80, 100, 180))
                      for face in faces(Path(directory) / name, weight)]
            self.assertEqual([face["weight"] for face in conformance_font.check_visible_faces(records, pinned)],
                             [400, 500, 600])
            ignored = faces(other, 200) + faces(other, 100, slant=100) + faces(other, 100, width=87.5)
            self.assertEqual(len(conformance_font.check_visible_faces(records, pinned + ignored)), 3)
            for unpinned, weight in ((faces(other, 100, index=3), 500),
                                     (faces(Path(directory) / "Inter-Regular.otf", 100), 500),
                                     (faces(Path(directory) / "Inter-Medium.otf", 100, index=1), 500),
                                     (faces(other, "[0 210]"), 400)):
                with self.assertRaisesRegex(ValueError, f"is visible as Inter weight {weight} but is not the pinned"):
                    conformance_font.check_visible_faces(records, pinned + unpinned)

    def test_fontconfig_listing_is_parsed_strictly(self):
        listing = "/fonts/Inter.ttc\t3\t100\t0\t100\n/fonts/InterVariable.ttf\t0\t[100 900]\t0\t[75 100]\n"
        faces = conformance_font.parse_fontconfig_faces(listing)
        self.assertEqual([(face["file"], face["index"], face["weight"], face["width"]) for face in faces],
                         [("/fonts/Inter.ttc", 3, (100.0, 100.0), (100.0, 100.0)),
                          ("/fonts/InterVariable.ttf", 0, (100.0, 900.0), (75.0, 100.0))])
        for listing in ("/fonts/Inter.ttc\t3\t100\n", "/fonts/Inter.ttc\t3\tbold\t0\t100\n"):
            with self.assertRaisesRegex(ValueError, "unreadable"):
                conformance_font.parse_fontconfig_faces(listing)
        failed = mock.Mock(returncode=1, stdout="", stderr="no fc")
        with mock.patch.object(conformance_font.subprocess, "run", return_value=failed):
            with self.assertRaisesRegex(ValueError, "fc-list failed"):
                conformance_font.fontconfig_faces({})

    def test_fetch_rejects_an_oversized_archive(self):
        response = mock.MagicMock()
        response.__enter__.return_value.read.return_value = b"x" * (conformance_font.ARCHIVE_LIMIT + 1)
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font.urllib.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(ValueError, "exceeds"):
                conformance_font.fetch(directory)
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_isolated_fontconfig_lists_no_font_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            environment = conformance_font.isolated_fontconfig(directory)
            text = Path(environment["FONTCONFIG_FILE"]).read_text(encoding="utf-8")
            self.assertNotIn("<dir", text)
            self.assertIn("<cachedir>", text)


if __name__ == "__main__":
    unittest.main()
