import argparse
from pathlib import Path
import subprocess
import sys


EXPECTED = {
    "excluded": False,
    "disabled-discoverable": True,
    "enabled-retained": True,
    "disabled-retained": True,
    "unsafe-old-focus": True,
    "unsafe-successor-focus": True,
    "transferred-old-focus": False,
    "transferred-successor-focus": True,
    "excluded-after-transfer": False,
}


def check_observations(output):
    observed = {}
    for line in output.splitlines():
        if not line.startswith("resina-focus-recovery "):
            continue
        fields = line.split()
        if len(fields) != 3 or fields[2] not in ("true", "false"):
            raise ValueError("malformed native focus observation")
        name = fields[1]
        if name not in EXPECTED or name in observed:
            raise ValueError(f"unknown or duplicate native focus observation: {name}")
        observed[name] = fields[2] == "true"
    if observed != EXPECTED:
        raise ValueError(f"native focus observations differ: {observed!r}")
    return len(observed)


def main():
    parser = argparse.ArgumentParser(description="Verify pinned Slint focus eligibility and recovery ordering.")
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command[1:] if arguments.command[:1] == ["--"] else arguments.command
    if not command or arguments.image.exists():
        parser.error("provide a viewer command and a fresh capture path")
    try:
        version = subprocess.run(command + ["--version"], capture_output=True, text=True, timeout=20, check=False)
        if version.returncode or version.stdout.strip() != "slint-viewer 1.18.1":
            raise ValueError("requires the official slint-viewer 1.18.1")
        result = subprocess.run(
            command + ["--screenshot", arguments.image.as_posix(), "conformance/slint/focus-recovery.slint"],
            capture_output=True, text=True, timeout=20, check=False,
        )
        if result.returncode:
            raise ValueError(f"viewer failed (exit {result.returncode}): {result.stderr[-2000:]}")
        if not arguments.image.is_file():
            raise ValueError("viewer did not produce the fresh capture")
        count = check_observations(result.stdout + "\n" + result.stderr)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(f"FAIL Slint focus recovery: {error}", file=sys.stderr)
        return 1
    print(f"Slint focus recovery passed {count} native observations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
