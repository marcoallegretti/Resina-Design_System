import copy
import io
import json
import os
import struct
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock
import zipfile

import conformance_font


ROOT = Path(__file__).resolve().parent.parent


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


def script_manifest(data):
    manifest = copy.deepcopy(conformance_font.MANIFEST)
    for script in manifest["scripts"]:
        for master in script["masters"]:
            master["sha256"] = conformance_font.sha256(data[master["file"]])
    return manifest


def sfnt(cmap):
    return (struct.pack(">IHHHH", 0x00010000, 1, 16, 0, 0) + b"cmap" + struct.pack(">III", 0, 28, len(cmap)) + cmap)


def cmap_table(platform, encoding, subtable):
    return struct.pack(">HHHHI", 0, 1, platform, encoding, 12) + subtable


def format_12(groups):
    body = b"".join(struct.pack(">III", *group) for group in groups)
    return struct.pack(">HHIII", 12, 0, 16 + len(body), 0, len(groups)) + body


def format_4():
    """Segments 0x41-0x43 by delta, then 0x627-0x629 by glyph array [5, 0, 7], then the terminator."""
    ends, starts, deltas = (0x43, 0x629, 0xFFFF), (0x41, 0x627, 0xFFFF), (0xFFC0, 0, 1)
    offsets = (0, 2 * (3 - 1) + 0, 0)
    body = (struct.pack(">3H", *ends) + struct.pack(">H", 0) + struct.pack(">3H", *starts)
            + struct.pack(">3H", *deltas) + struct.pack(">3H", *offsets) + struct.pack(">3H", 5, 0, 7))
    return struct.pack(">HHHHHHH", 4, 14 + len(body), 0, 6, 4, 1, 2) + body


class ScriptFaceTests(unittest.TestCase):
    def test_script_faces_are_pinned_per_weight_with_licence_and_language(self):
        scripts = conformance_font.MANIFEST["scripts"]
        self.assertEqual([(script["script"], script["lang"], script["family"]) for script in scripts],
                         [("Arabic", "ar", "Noto Sans Arabic"), ("Hebrew", "he", "Noto Sans Hebrew")])
        for script in scripts:
            self.assertEqual([master["weight"] for master in script["masters"]], [400, 500, 600])
            self.assertEqual(script["license"], "SIL Open Font License 1.1")
            self.assertTrue(script["archive"]["url"].startswith("https://github.com/notofonts/"))
            self.assertTrue(all(len(master["sha256"]) == 64 for master in script["masters"]))

    def files(self):
        return {master["file"]: master["file"].encode()
                for script in conformance_font.MANIFEST["scripts"] for master in script["masters"]}

    def test_directory_must_contain_each_pinned_script_master(self):
        data = self.files()
        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font, "MANIFEST", script_manifest(data)):
            for name, content in data.items():
                (Path(directory) / name).write_bytes(content)
            records = conformance_font.script_masters(directory)
            self.assertEqual([(record["script"], record["lang"], record["style"], record["weight"])
                              for record in records],
                             [(script, lang, style, weight) for script, lang in (("Arabic", "ar"), ("Hebrew", "he"))
                              for style, weight in (("Regular", 400), ("Medium", 500), ("SemiBold", 600))])
            (Path(directory) / "NotoSansHebrew-Medium.ttf").write_bytes(b"other")
            with self.assertRaisesRegex(ValueError, "not the pinned Noto Sans Hebrew 3.001 Medium master"):
                conformance_font.script_masters(directory)
            (Path(directory) / "NotoSansHebrew-Medium.ttf").unlink()
            with self.assertRaisesRegex(ValueError, "Noto Sans Hebrew Medium master is unavailable"):
                conformance_font.script_masters(directory)

    def test_fetch_verifies_every_archive_before_extracting_any_master(self):
        manifest = copy.deepcopy(conformance_font.MANIFEST)
        archives = {}
        for source in [manifest, *manifest["scripts"]]:
            payload = io.BytesIO()
            with zipfile.ZipFile(payload, "w") as bundle:
                for master in source["masters"]:
                    bundle.writestr(master["archivePath"], source["family"] + master["style"])
                    master["sha256"] = conformance_font.sha256((source["family"] + master["style"]).encode())
            source["archive"]["sha256"] = conformance_font.sha256(payload.getvalue())
            archives[source["archive"]["url"]] = payload.getvalue()

        def opened(url, timeout):
            response = mock.MagicMock()
            response.__enter__.return_value.read.return_value = archives[url]
            return response

        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font, "MANIFEST", manifest), \
                mock.patch.object(conformance_font.urllib.request, "urlopen", side_effect=opened):
            records = conformance_font.fetch(directory)
            self.assertEqual([record["file"] for record in records],
                             [master["file"] for source in [manifest, *manifest["scripts"]]
                              for master in source["masters"]])
        tampered = dict(archives)
        hebrew = manifest["scripts"][1]["archive"]["url"]
        tampered[hebrew] = tampered[hebrew] + b"x"

        def opened_tampered(url, timeout):
            response = mock.MagicMock()
            response.__enter__.return_value.read.return_value = tampered[url]
            return response

        with tempfile.TemporaryDirectory() as directory, \
                mock.patch.object(conformance_font, "MANIFEST", manifest), \
                mock.patch.object(conformance_font.urllib.request, "urlopen", side_effect=opened_tampered):
            with self.assertRaisesRegex(ValueError, "Noto Sans Hebrew archive does not match the pinned SHA-256"):
                conformance_font.fetch(directory)
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_only_pinned_faces_may_be_visible_for_a_script(self):
        records = [{"script": "Arabic", "family": "Noto Sans Arabic", "sha256": conformance_font.sha256(b"pinned")}]
        with tempfile.TemporaryDirectory() as directory:
            pinned, other = Path(directory) / "pinned.ttf", Path(directory) / "DejaVuSans.ttf"
            pinned.write_bytes(b"pinned")
            other.write_bytes(b"other")

            def faces(file, index=0):
                return conformance_font.parse_fontconfig_faces(f"{file}\t{index}\t80\t0\t100\n")

            self.assertEqual(conformance_font.check_visible_script_faces(records, faces(pinned)), [str(pinned)])
            for unpinned in (faces(other), faces(pinned, 1)):
                with self.assertRaisesRegex(ValueError, "visible for Arabic text but is not a pinned Noto Sans Arabic"):
                    conformance_font.check_visible_script_faces(records, faces(pinned) + unpinned)

    def test_every_script_is_listed_by_its_language(self):
        records = [{"script": script["script"], "family": script["family"], "sha256": "0" * 64}
                   for script in conformance_font.MANIFEST["scripts"]]
        with mock.patch.object(conformance_font, "fontconfig_faces", return_value=[]) as listed:
            self.assertEqual(conformance_font.check_script_fontconfig(records, {"FONTCONFIG_FILE": "x"}),
                             {"Arabic": [], "Hebrew": []})
        self.assertEqual([call.args for call in listed.call_args_list],
                         [({"FONTCONFIG_FILE": "x"}, ":lang=ar"), ({"FONTCONFIG_FILE": "x"}, ":lang=he")])

    def test_isolated_fontconfig_lists_only_the_given_font_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            fonts = Path(directory) / "fonts"
            fonts.mkdir()
            text = Path(conformance_font.isolated_fontconfig(Path(directory) / "config", fonts)[
                "FONTCONFIG_FILE"]).read_text(encoding="utf-8")
            self.assertEqual(text.count("<dir>"), 1)
            self.assertIn(f"<dir>{fonts.resolve()}</dir>", text)
            self.assertIn("/etc/fonts/conf.d", text)

    def test_process_isolation_sets_the_fontconfig_file(self):
        with tempfile.TemporaryDirectory() as directory, mock.patch.dict(os.environ, {}, clear=False):
            conformance_font.isolate_process(directory)
            text = Path(os.environ["FONTCONFIG_FILE"]).read_text(encoding="utf-8")
            self.assertIn(f"<dir>{Path(directory).resolve()}</dir>", text)

    def test_cmap_reader_maps_format_4_and_format_12_glyphs(self):
        self.assertEqual(sorted(conformance_font.cmap_code_points(sfnt(cmap_table(3, 10, format_12(
            [(0x627, 0x629, 5), (0x5D5, 0x5D5, 0), (0x1F600, 0x1F600, 9)]))))),
            [0x627, 0x628, 0x629, 0x1F600])
        self.assertEqual(sorted(conformance_font.cmap_code_points(sfnt(cmap_table(3, 1, format_4())))),
                         [0x41, 0x42, 0x43, 0x627, 0x629])
        for font in (b"\0" * 12 + struct.pack(">H", 0), sfnt(cmap_table(1, 0, format_12([(1, 1, 1)])))):
            with self.assertRaises(ValueError):
                conformance_font.cmap_code_points(font)

    def test_evidence_code_points_are_read_from_tracked_text_and_escapes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            alef, lam, shin = chr(0x627), chr(0x644), chr(0x5E9)
            (root / "vector.json").write_text(
                '{"text": "' + alef + lam + " " + shin + '", "escaped": "' + chr(92) + 'u0663"}', encoding="utf-8")
            (root / "case.rs").write_text(
                'let text = "' + chr(92) + 'u{5de}" + "' + chr(0xFEFF) + chr(0xE9) + '";', encoding="utf-8")
            (root / "image.bin").write_bytes(b"\xff\xfe\x00\xd7")
            (root / "tools").mkdir()
            (root / "tools/conformance_font.json").write_text(chr(0x630), encoding="utf-8")
            subprocess.run(["git", "add", "-A"], cwd=root, check=True)
            found = conformance_font.evidence_code_points(root)
            self.assertEqual(sorted(found["Arabic"]), [0x627, 0x644, 0x663])
            self.assertEqual(sorted(found["Hebrew"]), [0x5DE, 0x5E9])

    def test_script_master_must_cover_the_code_points_of_the_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            font = Path(directory) / "font.ttf"
            font.write_bytes(sfnt(cmap_table(3, 10, format_12([(0x627, 0x629, 5)]))))
            record = {"script": "Arabic", "file": "font.ttf", "path": font}
            conformance_font.check_coverage([record], {"Arabic": {0x627, 0x629}, "Hebrew": set()})
            with self.assertRaisesRegex(ValueError, "font.ttf has no glyph for label evidence code points U\\+0644"):
                conformance_font.check_coverage([record], {"Arabic": {0x627, 0x644}, "Hebrew": set()})

    def test_every_script_face_must_be_reported_once_at_its_weight(self):
        records = [{"script": script["script"], "family": script["family"], "style": master["style"],
                    "weight": master["weight"], "file": master["file"], "sha256": master["sha256"]}
                   for script in conformance_font.MANIFEST["scripts"] for master in script["masters"]]

        def log(skip=None, extra=None):
            lines = [{"script": record["script"], "weight": record["weight"],
                      "face": {"family": record["family"], "style": record["style"]}} for record in records]
            lines = [line for line in lines if (line["script"], line["weight"]) != skip]
            return "".join("RESINA_LABEL_SCRIPT_FACE " + json.dumps(line) + "\n" for line in lines + (extra or []))

        self.assertEqual(len(conformance_font.resolved_script_faces(log(), records, (400, 500, 600))), 6)
        with self.assertRaisesRegex(ValueError, "did not report every script face"):
            conformance_font.resolved_script_faces(log(("Hebrew", 600)), records, (400, 500, 600))
        duplicate = {"script": "Arabic", "weight": 400, "face": {"family": "Noto Sans Arabic", "style": "Regular"}}
        with self.assertRaisesRegex(ValueError, "twice"):
            conformance_font.resolved_script_faces(log(extra=[duplicate]), records, (400, 500, 600))
        substituted = log().replace('"style": "Medium"', '"style": "Regular"', 1)
        with self.assertRaisesRegex(ValueError, "weight 500 resolved to the Noto Sans Arabic Regular master"):
            conformance_font.resolved_script_faces(substituted, records, (400, 500, 600))

    def test_the_qml_oracle_receives_each_family_with_the_evidence_text_of_its_script(self):
        scripts = json.loads(conformance_font.qml_script_faces(ROOT))
        self.assertEqual([script["family"] for script in scripts], ["Noto Sans Arabic", "Noto Sans Hebrew"])
        self.assertTrue(scripts[0]["sample"] and scripts[1]["sample"])
        self.assertTrue(all(conformance_font.script_of(ord(character)) == script["script"]
                            for script in scripts for character in script["sample"]))


if __name__ == "__main__":
    unittest.main()
