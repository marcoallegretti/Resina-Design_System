import argparse
import math
from pathlib import Path
import subprocess
import sys

from PIL import Image


def check_image(path, scale):
    with Image.open(path) as source:
        if source.format != "PNG" or source.size != (int(40 * scale), int(34 * scale)):
            raise ValueError("unexpected native focus capture format or dimensions")
        image = source.convert("RGBA")
    count = 0
    painted = 0
    for y in range(image.height):
        for x in range(image.width):
            px, py = (x + 0.5) / scale - 8, (y + 0.5) / scale - 8
            distance = math.hypot(max(-px, px - 20, 0), max(-py, py - 14, 0))
            if min(abs(distance - 2), abs(distance - 4)) * scale <= math.sqrt(0.5):
                continue
            inside = 2 < distance < 4
            expected = (255, 255, 255, 255) if inside else (32, 32, 32, 255)
            if image.getpixel((x, y)) != expected:
                raise ValueError(f"native focus pixel ({x},{y}) differs from the distance oracle")
            count += 1
            painted += inside
    if not 0 < painted < count:
        raise ValueError("capture does not exercise both painted and clear pixels")
    return count


def main():
    parser = argparse.ArgumentParser(description="Verify fresh Slint focus-probe pixels against independent rectangle-distance geometry.")
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("--scale", type=float, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command or not math.isfinite(arguments.scale) or not 1 <= arguments.scale <= 4:
        parser.error("provide a viewer command and scale in 1..=4")
    if arguments.image.exists():
        parser.error("capture path must not already exist")
    try:
        result = subprocess.run(command, capture_output=True, timeout=20, check=False)
        if result.returncode:
            raise ValueError(f"viewer failed (exit {result.returncode}): {result.stderr[-2000:]!r}")
        count = check_image(arguments.image, arguments.scale)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Slint focus runtime: {error}", file=sys.stderr)
        return 1
    print(f"Slint focus runtime passed {count} independent off-boundary pixels at scale {arguments.scale}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
