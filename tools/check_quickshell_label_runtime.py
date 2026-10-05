import argparse
import math
import os
from pathlib import Path
import re
import subprocess
import sys


def check_log(result, scale):
    log = result.stdout + result.stderr
    matches = re.findall(r"RESINA_LABEL_PASS (\d+) (\d+) ([0-9.]+)(?:\s|$)", log)
    if result.returncode or "RESINA_LABEL_FAIL" in log or len(matches) != 1:
        raise ValueError(f"native label probe failed (exit {result.returncode}): {log[-2000:]}")
    count, failures, actual_scale = matches[0]
    if int(count) != 243 or int(failures) != 13 or not math.isclose(float(actual_scale), scale, rel_tol=0, abs_tol=0.00001):
        raise ValueError(f"native label evidence counts or actual scale differ: {matches[0]}")


def main():
    parser = argparse.ArgumentParser(description="Check complete native Qt Quick label measurement on actual device scales.")
    parser.add_argument("--font", type=Path, required=True)
    parser.add_argument("--quickshell", default="quickshell")
    parser.add_argument("--platform", choices=("offscreen", "wayland"), default="offscreen")
    parser.add_argument("--scales", nargs="+", type=float, default=[0.5, 1, 1.25, 2, 1.3])
    arguments = parser.parse_args()
    if not arguments.font.is_file():
        parser.error("provide the DejaVu Sans font file")
    if any(not math.isfinite(scale) or scale <= 0 for scale in arguments.scales):
        parser.error("device scales must be finite and positive")
    root = Path(__file__).resolve().parent.parent
    environment = os.environ.copy()
    environment.update(
        QT_QPA_PLATFORM=arguments.platform,
        QT_QUICK_BACKEND="software",
        RESINA_LABEL_FONT=arguments.font.resolve().as_uri(),
        RESINA_LABEL_MEASURE_QML=(root / "backends/quickshell/resina-qml/qml/ResinaLabelMeasure.qml").as_uri(),
    )
    try:
        for scale in arguments.scales:
            environment.update(QT_SCALE_FACTOR=str(scale), RESINA_LABEL_DEVICE_SCALE=str(scale))
            result = subprocess.run(
                [arguments.quickshell, "--no-color", "--path", str(root / "conformance/quickshell/label-probe.qml")],
                env=environment, capture_output=True, encoding="utf-8", errors="strict", timeout=20, check=False,
            )
            check_log(result, scale)
    except (OSError, UnicodeError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell label runtime: {error}", file=sys.stderr)
        return 1
    print(f"Quickshell native label measurement passed {243 * len(arguments.scales)} cases and {13 * len(arguments.scales)} diagnostic cases on {arguments.platform}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
