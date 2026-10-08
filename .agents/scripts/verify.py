"""Run explicit contributor checks without installing dependencies or editing files."""

import argparse
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def commands(profile):
    if profile == "toolkit":
        return [
            [sys.executable, "tools/check_contributor_toolkit.py"],
            [sys.executable, "-m", "unittest", "discover", "-s", "tools",
             "-p", "test_contributor_toolkit.py"],
        ]
    if profile == "baseline":
        return [
            ["cargo", "fmt", "--all", "--", "--check"],
            ["cargo", "test", "--workspace", "--locked"],
            ["cargo", "clippy", "--workspace", "--all-targets", "--locked",
             "--", "-D", "warnings"],
        ]
    if profile == "schemas":
        return [[sys.executable, "tools/check_schemas.py"]]
    raise ValueError(f"unknown profile: {profile}")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("profiles", nargs="+", choices=("toolkit", "baseline", "schemas"))
    parser.add_argument("--dry-run", action="store_true", help="print checks without running them")
    args = parser.parse_args(argv)
    for profile in dict.fromkeys(args.profiles):
        for command in commands(profile):
            print(f"[{profile}] {subprocess.list2cmdline(command)}", flush=True)
            if args.dry_run:
                continue
            try:
                result = subprocess.run(command, cwd=ROOT, check=False)
            except OSError as error:
                print(f"Cannot run {command[0]}: {error}", file=sys.stderr)
                return 1
            if result.returncode:
                return result.returncode if result.returncode > 0 else 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
