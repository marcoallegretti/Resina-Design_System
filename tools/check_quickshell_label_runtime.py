import argparse
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile

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


def check_log(result, scale, masters):
    log = result.stdout + result.stderr
    matches = re.findall(r"RESINA_LABEL_PASS (\d+) (\d+) ([0-9.]+)(?:\s|$)", log)
    if result.returncode or "RESINA_LABEL_FAIL" in log or len(matches) != 1:
        raise ValueError(f"native label probe failed (exit {result.returncode}): {log[-2000:]}")
    count, failures, actual_scale = matches[0]
    if int(count) != 432 or int(failures) != 13 or not math.isclose(float(actual_scale), scale, rel_tol=0, abs_tol=0.00001):
        raise ValueError(f"native label evidence counts or actual scale differ: {matches[0]}")
    return resolved_masters(log, masters)


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
    parser.add_argument("--font-dir", type=Path, required=True, help="directory with the pinned Inter static masters")
    parser.add_argument("--quickshell", default="quickshell")
    parser.add_argument("--platform", choices=("offscreen", "wayland"), default="offscreen")
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 1.25, 2])
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
        environment["RESINA_LABEL_FONTS"] = json.dumps([master["path"].as_uri() for master in masters])
        for scale in arguments.scales:
            environment.update(QT_SCALE_FACTOR=str(scale), RESINA_LABEL_DEVICE_SCALE=str(scale))
            result = subprocess.run(probe, env=environment, capture_output=True, encoding="utf-8",
                                    errors="strict", timeout=20, check=False)
            evidence["scales"].append({"deviceScale": scale, "masters": check_log(result, scale, masters)})
        with tempfile.TemporaryDirectory() as directory:
            isolated = dict(environment, **conformance_font.isolated_fontconfig(directory), QT_SCALE_FACTOR="1",
                            RESINA_LABEL_DEVICE_SCALE="1", RESINA_LABEL_MODE="faces",
                            RESINA_LABEL_FONTS=json.dumps([masters[0]["path"].as_uri()]))
            result = subprocess.run(probe, env=isolated, capture_output=True, encoding="utf-8",
                                    errors="strict", timeout=20, check=False)
            evidence["regularOnly"] = check_substitution(result, masters)
    except (OSError, UnicodeError, ValueError, KeyError, TypeError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell label runtime: {error}", file=sys.stderr)
        return 1
    print(json.dumps(evidence, indent=2))
    print(f"Quickshell native label measurement passed {432 * len(arguments.scales)} cases and "
          f"{13 * len(arguments.scales)} diagnostic cases on {arguments.platform}, resolved distinct Inter masters "
          "for weights 400, 500 and 600, and rejected a Regular-only registration")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
