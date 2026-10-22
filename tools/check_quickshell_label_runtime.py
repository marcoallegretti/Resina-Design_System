import argparse
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

from check_schemas import digit_advance_outcome, validator_for
import conformance_font


WEIGHTS = (400, 500, 600)


def resolved_masters(log, masters):
    faces = {}
    for line in re.findall(r"RESINA_LABEL_FACE (\{.*\})", log):
        observed = json.loads(line)
        if observed["weight"] in faces:
            raise ValueError(f"native label probe reported weight {observed['weight']} twice")
        faces[observed["weight"]] = observed["face"]
    if sorted(faces) != list(WEIGHTS):
        raise ValueError(f"native label probe did not report every weight: {sorted(faces)}")
    return [conformance_font.resolve(masters, faces[weight]["family"], faces[weight]["style"], weight)
            for weight in WEIGHTS]


def digit_records(log):
    """Weight-500 digits must be equal with tnum and fail without it on the proportional default."""
    observed = {}
    for line in re.findall(r"RESINA_LABEL_DIGITS (\{.*\})", log):
        digits = json.loads(line)
        if digits["tabular"] in observed:
            raise ValueError("native label probe reported digit advances twice")
        observed[digits["tabular"]] = digits["record"]
    if sorted(observed) != [False, True]:
        raise ValueError("native label probe did not report tabular and default digit advances")
    validator = validator_for("schemas/digit-advance-record.schema.json")
    for record in observed.values():
        errors = sorted(error.message for error in validator.iter_errors(record))
        if errors:
            raise ValueError(f"native digit-advance record is invalid: {errors[0]}")
    tabular, default = (digit_advance_outcome(observed[key]["advances"]) for key in (True, False))
    if "advance" not in tabular:
        raise ValueError(f"tnum digits are not equal: {tabular['error']}")
    if "error" not in default:
        raise ValueError("default digits of the conformance face were expected to be proportional")
    return {"tabular": {**observed[True], "outcome": tabular}, "default": {**observed[False], "outcome": default}}


def check_log(result, scale, masters, scripts):
    log = result.stdout + result.stderr
    matches = re.findall(r"RESINA_LABEL_PASS (\d+) (\d+) ([0-9.]+)(?:\s|$)", log)
    if result.returncode or "RESINA_LABEL_FAIL" in log or len(matches) != 1:
        raise ValueError(f"native label probe failed (exit {result.returncode}): {log[-2000:]}")
    count, failures, actual_scale = matches[0]
    if int(count) != 432 or int(failures) != 13 or not math.isclose(float(actual_scale), scale, rel_tol=0, abs_tol=0.00001):
        raise ValueError(f"native label evidence counts or actual scale differ: {matches[0]}")
    return {"masters": resolved_masters(log, masters), "scriptFaces": conformance_font.resolved_script_faces(
        log, scripts, WEIGHTS), "digits": digit_records(log)}


def check_substitution(result, masters):
    """A Regular-only registration must be reported as Regular for weights 500 and 600."""
    log = result.stdout + result.stderr
    if result.returncode or "RESINA_LABEL_FAIL" in log or "RESINA_LABEL_FACES_PASS" not in log:
        raise ValueError(f"Regular-only label probe failed (exit {result.returncode}): {log[-2000:]}")
    try:
        resolved_masters(log, masters)
    except ValueError as error:
        if "substituted" not in str(error):
            raise
        return str(error)
    raise ValueError("a Regular-only registration was accepted for weights 500 and 600")


def main():
    parser = argparse.ArgumentParser(description="Check complete native Qt Quick label measurement on actual device scales.")
    parser.add_argument("--font-dir", type=Path, required=True, help="directory with the pinned static masters")
    parser.add_argument("--quickshell", default="quickshell")
    parser.add_argument("--platform", choices=("offscreen", "wayland"), default="offscreen")
    parser.add_argument("--scales", nargs="+", type=float, default=[0.5, 1, 1.25, 2, 1.3])
    arguments = parser.parse_args()
    if any(not math.isfinite(scale) or scale <= 0 for scale in arguments.scales):
        parser.error("device scales must be finite and positive")
    root = Path(__file__).resolve().parent.parent
    probe = [arguments.quickshell, "--no-color", "--path", str(root / "conformance/quickshell/label-probe.qml")]
    environment = os.environ.copy()
    environment.update(
        QT_QPA_PLATFORM=arguments.platform,
        QT_QUICK_BACKEND="software",
        RESINA_LABEL_MEASURE_QML=(root / "backends/quickshell/resina-qml/qml/ResinaLabelMeasure.qml").as_uri(),
    )
    environment.pop("RESINA_LABEL_MODE", None)
    evidence = {"platform": arguments.platform, "scales": []}
    try:
        masters = conformance_font.masters(arguments.font_dir)
        scripts = conformance_font.script_masters(arguments.font_dir)
        with tempfile.TemporaryDirectory() as directory:
            environment.update(conformance_font.isolated_fontconfig(directory, arguments.font_dir),
                               RESINA_LABEL_FONTS=json.dumps([master["path"].as_uri() for master in masters]),
                               RESINA_LABEL_SCRIPT_FACES=conformance_font.qml_script_faces(root))
            evidence["visibleFaces"] = conformance_font.check_fontconfig(masters, environment)
            evidence["visibleScriptFaces"] = conformance_font.check_script_fontconfig(scripts, environment)
            for scale in arguments.scales:
                environment.update(QT_SCALE_FACTOR=str(scale), RESINA_LABEL_DEVICE_SCALE=str(scale))
                result = subprocess.run(probe, env=environment, capture_output=True, encoding="utf-8",
                                        errors="strict", timeout=20, check=False)
                evidence["scales"].append({"deviceScale": scale, **check_log(result, scale, masters, scripts)})
            isolated = dict(environment, **conformance_font.isolated_fontconfig(Path(directory) / "regular-only"),
                            QT_SCALE_FACTOR="1",
                            RESINA_LABEL_DEVICE_SCALE="1", RESINA_LABEL_MODE="faces",
                            RESINA_LABEL_FONTS=json.dumps([masters[0]["path"].as_uri()]))
            result = subprocess.run(probe, env=isolated, capture_output=True, encoding="utf-8",
                                    errors="strict", timeout=20, check=False)
            evidence["regularOnly"] = check_substitution(result, masters)
    except (OSError, UnicodeError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(f"FAIL Quickshell label runtime: {error}", file=sys.stderr)
        return 1
    print(json.dumps(evidence, indent=2))
    print(f"Quickshell native label measurement passed {432 * len(arguments.scales)} cases and "
          f"{13 * len(arguments.scales)} diagnostic cases on {arguments.platform}, resolved distinct Inter masters "
          "for weights 400, 500 and 600, drew Arabic and Hebrew with their pinned Noto masters at the same weights, "
          "measured equal tnum digit advances, and rejected a Regular-only registration and the proportional "
          "default digits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
