import argparse
import math
from pathlib import Path
import re
import subprocess
import sys

from PIL import Image


def check_image(path):
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (160, 128):
            raise ValueError("unexpected Quickshell focus capture format or dimensions")
        image = source.convert("RGBA")
    count = 0
    alpha_sum = 0
    for y in range(image.height):
        for x in range(image.width):
            px, py = (x + 0.5) / 4 - 8, (y + 0.5) / 4 - 8
            distance = math.hypot(max(-px, px - 20, 0), max(-py, py - 14, 0))
            pixel = image.getpixel((x, y))
            alpha_sum += pixel[3]
            if pixel[3] and pixel[:3] != (255, 255, 255):
                raise ValueError(f"focus pigment differs at pixel ({x},{y})")
            if 2.5 <= distance <= 3.5:
                if pixel[3] != 255:
                    raise ValueError(f"focus ring pixel ({x},{y}) is not opaque")
                count += 1
            elif distance <= 1.5 or distance >= 4.5:
                if pixel[3] != 0:
                    raise ValueError(f"focus hole/exterior pixel ({x},{y}) is painted")
                count += 1
    if image.getchannel("A").getbbox() != (16, 16, 128, 104):
        raise ValueError("focus capture does not preserve the complete expected paint bounds")
    area = alpha_sum / (255 * 16)
    expected_area = 2 * (20 + 14) * (4 - 2) + math.pi * (4 ** 2 - 2 ** 2)
    if abs(area - expected_area) > 1.0:
        raise ValueError(f"focus coverage area {area} differs from analytic ring area {expected_area}")
    return count, area


def main():
    parser = argparse.ArgumentParser(description="Verify the Quickshell focus baseline probe and its fresh PNG capture.")
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command:
        parser.error("provide a Quickshell probe command")
    if arguments.image.exists():
        parser.error("capture path must not already exist")
    try:
        result = subprocess.run(command, capture_output=True, encoding="utf-8", errors="strict", timeout=20, check=False)
        log = result.stdout + result.stderr
        if result.returncode or "RESINA_FOCUS_FAIL" in log or not re.search(r"RESINA_FOCUS_PASS 3197 [134](?:\s|$)", log):
            raise ValueError(f"runtime probe failed (exit {result.returncode}): {log[-2000:]}")
        count, area = check_image(arguments.image)
    except (OSError, UnicodeError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell focus runtime: {error}", file=sys.stderr)
        return 1
    print(f"Quickshell focus runtime passed 3197 containment points, {count} independent off-boundary pixels and {area:.6f} logical px² coverage")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
