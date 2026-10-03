import argparse
from pathlib import Path
import re
import struct
import subprocess
import sys


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
        with arguments.image.open("rb") as image:
            header = image.read(24)
        if len(header) != 24 or header[:16] != b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR" or struct.unpack(">II", header[16:24]) != (160, 128):
            raise ValueError("runtime probe did not create the expected PNG capture")
    except (OSError, UnicodeError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Quickshell focus runtime: {error}", file=sys.stderr)
        return 1
    print("Quickshell focus runtime passed 3197 off-boundary containment points and fresh image capture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
