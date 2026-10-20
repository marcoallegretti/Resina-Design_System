import copy
import io
import json
import struct
import tempfile
import unittest
import zlib
from pathlib import Path
from unittest.mock import patch

from PIL import Image
from PIL.PngImagePlugin import PngInfo

from check_schemas import ROOT, load_json, schema_registry, tool_schema_registry
from check_surface_paint_backend import baseline, expected_result
from visual_acceptance import compare_native_labels, compare_records, git_blob, image_metrics, record_image, sha256, srgb_to_oklab, validate_capture, validate_native_label, validate_record, validate_viewport


def png(pixels, size):
    image = Image.new("RGBA", size)
    image.putdata(pixels)
    output = io.BytesIO()
    encoding = PngInfo()
    encoding.add(b"sRGB", b"\x00")
    image.save(output, format="PNG", pnginfo=encoding)
    return output.getvalue()


class VisualAcceptanceTests(unittest.TestCase):
    def test_image_file_limit_is_checked_before_buffering_the_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "capture.json"
            (path.parent / "image.png").write_bytes(b"12345")
            with patch("visual_acceptance.MAX_PNG_BYTES", 4), self.assertRaisesRegex(ValueError, "byte limit"):
                record_image(path, {"image": {"file": "image.png"}})

    def test_rgba16_cannot_be_silently_reduced_to_identical_rgba8_pixels(self):
        def chunk(kind, data):
            return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

        def rgba16(red):
            return (b"\x89PNG\r\n\x1a\n"
                    + chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 16, 6, 0, 0, 0))
                    + chunk(b"sRGB", b"\x00")
                    + chunk(b"IDAT", zlib.compress(b"\x00" + struct.pack(">HHHH", red, 0, 0, 65535)))
                    + chunk(b"IEND", b""))

        with self.assertRaisesRegex(ValueError, "RGBA8"):
            image_metrics(rgba16(32768), rgba16(33023))

    def test_evidence_formats_cannot_enter_normative_schema_resolution(self):
        identity = "urn:resina:tool-schema:visual-capture-record:0.1.0"
        self.assertNotIn(identity, schema_registry())
        self.assertIn(identity, tool_schema_registry())
        self.assertIn("urn:resina:schema:surface-paint-ir:0.1.0", tool_schema_registry())

    def test_malformed_records_fail_before_any_image_is_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "capture.json"
            for record in ([], {}, {"kind": "nativeLabel"}):
                path.write_text(json.dumps(record), encoding="utf-8")
                with self.assertRaises(ValueError):
                    compare_records(path, path)

    def test_native_label_keeps_font_gpu_and_layout_evidence_separate_from_pixels(self):
        source = png([(255, 255, 255, 255)] * (220 * 300), (220, 300))
        record = {
            "schemaVersion": "0.1.0", "kind": "nativeLabel", "purpose": "diagnostic",
            "commit": "a" * 40, "scenario": "unit-fixture", "width": 220, "height": 300,
            "deviceScale": 1, "textScale": 1, "sampling": {"time": 0, "inputTrajectory": []},
            "semanticResult": {
                "schemaVersion": "0.1.0", "text": "Save", "layoutDirection": "ltr",
                "typography": load_json(ROOT / "conformance/headless/expected-resolution.json")["typography"]["label"],
                "size": {"width": 64, "height": 44}, "labelBounds": {"x": 16, "y": 8, "width": 36, "height": 28},
            },
            "font": {"family": "Inter", "postScriptName": "Inter-Regular", "weight": 400, "file": "Inter-Regular.otf", "gitBlob": "b" * 40},
            "renderer": {"name": "guido-native-label-test", "gitBlob": "c" * 40,
                         "gpu": {"name": "unit-fixture", "vendor": 0, "device": 0, "backend": "Vulkan", "deviceType": "Cpu", "driver": "test", "driverInfo": "test"}},
            "runtime": {"system": "test", "machine": "test"}, "colorEncoding": "rgba8-srgb-straight",
            "image": {"file": "unit-fixture.png", "gitBlob": git_blob(source)},
        }
        validate_native_label(record, source)
        self.assertEqual(compare_native_labels(record, record, (source, source))["changedPixels"], 0)
        altered = copy.deepcopy(record)
        altered["font"]["gitBlob"] = "d" * 40
        with self.assertRaisesRegex(ValueError, "context/semantics"):
            compare_native_labels(record, altered, (source, source))
        altered = copy.deepcopy(record)
        altered["renderer"]["gpu"]["driverInfo"] = "different driver"
        with self.assertRaisesRegex(ValueError, "cross-GPU"):
            compare_native_labels(record, altered, (source, source))
        altered = copy.deepcopy(record)
        altered["semanticResult"]["labelBounds"]["width"] = 220
        with self.assertRaisesRegex(ValueError, "clips"):
            validate_native_label(altered, source)
        altered = copy.deepcopy(record)
        altered["sampling"]["time"] = 1
        with self.assertRaises(ValueError):
            validate_native_label(altered, source)

    def record(self):
        request = baseline()
        request["body"]["surface"]["states"]["states"] = ["focused"]
        source = png([(20, 30, 40, 255)] * (32 * 26), (32, 26))
        record = {
            "schemaVersion": "0.1.0", "commit": "a" * 40, "purpose": "diagnostic",
            "scene": {"name": "guarded", "kind": "surfacePaint", "expectedMaterialFamily": "cast", "request": request},
            "result": expected_result(request),
            "capture": {"origin": {"x": -6, "y": -6}, "width": 32, "height": 26, "pixelsPerUnit": 1, "samplesPerAxis": 4},
            "scales": {"device": 1, "text": request["body"]["theme"]["environment"]["textScale"]},
            "sampling": {"time": 0, "initialChannels": None}, "font": None, "colorEncoding": "rgba8-srgb-straight",
            "renderer": {"name": "resina-paint-raster", "sha256": "b" * 64, "resolverSha256": "c" * 64, "gpu": None},
            "runtime": {"system": "test", "machine": "test", "python": "3.12", "pillow": "12.1.0"},
            "image": {"file": "image.png", "sha256": sha256(source)},
        }
        return record, source

    def test_record_preserves_actual_inputs_and_rejects_invented_acceptance(self):
        record, source = self.record()
        validate_record(record, source)
        mutations = [
            lambda r: r.update(purpose="accepted"),
            lambda r: r.update(colorEncoding="display-p3"),
            lambda r: r.update(font={"family": "not drawn"}),
            lambda r: r["sampling"].update(time=1),
            lambda r: r["scales"].update(device=2),
            lambda r: r["image"].update(sha256="0" * 64),
            lambda r: r["image"].update(file="../image.png"),
            lambda r: r["capture"].update(width=33),
            lambda r: r["renderer"].update(gpu="not observed"),
            lambda r: r.update(undeclared="field"),
        ]
        for mutate in mutations:
            altered = copy.deepcopy(record)
            mutate(altered)
            with self.subTest(record=altered), self.assertRaises(ValueError):
                validate_record(altered, source)

    def test_actual_color_contrast_and_requested_dimensions_cannot_be_self_reported(self):
        record, source = self.record()
        mutations = [
            lambda r: r["result"]["body"].update(foreground=copy.deepcopy(r["result"]["body"]["pigment"]["body"])),
            lambda r: r["result"]["body"]["edge"].update(color=copy.deepcopy(r["scene"]["request"]["body"]["adjacentColor"])),
            lambda r: r["scene"]["request"]["body"]["size"].update(width=100),
            lambda r: r["result"]["focus"]["indicator"].update(contrastRatio=10),
            lambda r: r["result"]["body"]["lighting"]["direction"].update(y=1),
            lambda r: r["result"]["body"].update(edgeWidth=2),
        ]
        for index, mutate in enumerate(mutations):
            changed = copy.deepcopy(record)
            mutate(changed)
            with self.subTest(mutation=index), self.assertRaises(ValueError):
                validate_record(changed, source)

    def test_identical_images_do_not_hide_changed_geometry_light_or_context(self):
        record, source = self.record()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ("before", "after"):
                (root / name).mkdir()
                (root / name / "image.png").write_bytes(source)
                (root / name / "capture.json").write_text(json.dumps(record), encoding="utf-8")
            before, after = root / "before/capture.json", root / "after/capture.json"
            self.assertEqual(compare_records(before, after)["changedPixels"], 0)
            for field in ("commit", "runtime"):
                altered = copy.deepcopy(record)
                if field == "commit":
                    altered[field] = "b" * 40
                else:
                    altered[field]["system"] = "other host"
                after.write_text(json.dumps(altered), encoding="utf-8")
                self.assertEqual(compare_records(before, after)["changedPixels"], 0)
            altered = copy.deepcopy(record)
            altered["scene"]["request"]["body"]["appearance"]["keyLight"]["direction"]["y"] *= -1
            after.write_text(json.dumps(altered), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "context differs|key light differs"):
                compare_records(before, after)
            altered = copy.deepcopy(record)
            altered["result"]["body"]["lighting"]["direction"]["y"] *= -1
            after.write_text(json.dumps(altered), encoding="utf-8")
            with self.assertRaises(ValueError):
                compare_records(before, after)

    def test_viewport_rejects_invalid_and_excessive_work_before_capture(self):
        record, _ = self.record()
        for changes in ({"width": True}, {"width": 0}, {"width": 4194304}, {"samplesPerAxis": 9}, {"pixelsPerUnit": float("nan")}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                validate_viewport(record["capture"] | changes)

    def test_metric_conversion_matches_independent_public_vectors(self):
        for vector in load_json(ROOT / "conformance/color/oklab-conversion-vectors.json"):
            if "srgb" in vector and "oklab" in vector:
                with self.subTest(vector=vector["name"]):
                    for actual, expected in zip(srgb_to_oklab(vector["srgb"]), vector["oklab"], strict=True):
                        self.assertAlmostEqual(actual, expected, delta=1e-12)

    def test_identical_pixels_pass_and_one_pixel_changes_cannot_hide_in_an_average(self):
        original = png([(120, 130, 140, 255)] * 100, (10, 10))
        self.assertEqual(image_metrics(original, original)["changedPixels"], 0)
        altered = png([(121, 130, 140, 255)] + [(120, 130, 140, 255)] * 99, (10, 10))
        result = image_metrics(original, altered)
        self.assertEqual(result["changedPixels"], 1)
        self.assertGreater(result["maximumOklabDistance"], result["meanOklabDistance"])
        self.assertGreater(result["maximumOklabDistance"], 0)

    def test_alpha_is_checked_separately_and_on_both_extreme_backdrops(self):
        original = png([(255, 255, 255, 128)], (1, 1))
        altered = png([(255, 255, 255, 127)], (1, 1))
        result = image_metrics(original, altered)
        self.assertGreater(result["maximumAlphaDifference"], 0)
        self.assertGreater(result["maximumOklabDistance"], 0)

    def test_hidden_rgb_does_not_invent_visible_color_error(self):
        result = image_metrics(png([(255, 0, 0, 0)], (1, 1)), png([(0, 0, 0, 0)], (1, 1)))
        self.assertEqual(result["maximumOklabDistance"], 0)
        self.assertEqual(result["maximumAlphaDifference"], 0)
        self.assertEqual(result["changedPixels"], 1)

    def test_dimensions_format_and_color_encoding_cannot_be_silently_normalized(self):
        original = png([(0, 0, 0, 255)], (1, 1))
        with self.assertRaisesRegex(ValueError, "dimensions"):
            image_metrics(original, png([(0, 0, 0, 255)] * 2, (2, 1)))
        with self.assertRaises(ValueError):
            image_metrics(original, b"not a PNG")
        output = io.BytesIO()
        Image.new("RGB", (1, 1)).save(output, format="PNG")
        with self.assertRaisesRegex(ValueError, "RGBA"):
            image_metrics(original, output.getvalue())

    def test_semantic_checks_reject_missing_focus_and_capture_clipping(self):
        request = baseline()
        request["body"]["surface"]["states"]["states"] = ["focused"]
        result = expected_result(request)
        scene = {"name": "guarded", "kind": "surfacePaint", "expectedMaterialFamily": "cast", "request": request}
        viewport = {"origin": {"x": -6, "y": -6}, "width": 32, "height": 26, "pixelsPerUnit": 1, "samplesPerAxis": 4}
        validate_capture(scene, result, viewport)
        with self.assertRaisesRegex(ValueError, "clips"):
            validate_capture(scene, result, viewport | {"width": 1})
        altered = copy.deepcopy(result)
        altered.pop("focus")
        with self.assertRaises(ValueError):
            validate_capture(scene, altered, viewport)


if __name__ == "__main__":
    unittest.main()
