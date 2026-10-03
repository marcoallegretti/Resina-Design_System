import argparse
import json
import math
import subprocess
import sys

from check_schemas import ROOT, load_json, parse_json


def mismatch(actual, expected, pointer=""):
    if isinstance(expected, dict):
        if not isinstance(actual, dict) or actual.keys() != expected.keys():
            return pointer or "/"
        for name, value in expected.items():
            escaped = name.replace("~", "~0").replace("/", "~1")
            found = mismatch(actual[name], value, f"{pointer}/{escaped}")
            if found:
                return found
        return None
    if isinstance(expected, list):
        if not isinstance(actual, list) or len(actual) != len(expected):
            return pointer or "/"
        for index, value in enumerate(expected):
            found = mismatch(actual[index], value, f"{pointer}/{index}")
            if found:
                return found
        return None
    if isinstance(expected, bool) or isinstance(actual, bool):
        return None if actual is expected else pointer or "/"
    if isinstance(expected, (int, float)) and isinstance(actual, (int, float)):
        return None if actual == expected else pointer or "/"
    return None if type(actual) is type(expected) and actual == expected else pointer or "/"


def cases():
    sources = load_json(ROOT / "conformance/tokens/source-pipeline-vectors.json")
    documents = load_json(ROOT / "conformance/tokens/document-vectors.json")
    for category, vectors in (("source", sources), ("document", documents)):
        names = set()
        for vector in vectors:
            name = vector["name"]
            if name in names or ("expected" in vector) == ("error" in vector):
                raise ValueError(f"invalid {category} vector: {name}")
            names.add(name)
            source = (
                vector["source"]
                if category == "source"
                else json.dumps(vector["document"], ensure_ascii=False, allow_nan=False)
            )
            yield f"{category}: {name}", source, vector.get("expected")
    foundation = (ROOT / "tokens/foundation.json").read_text(encoding="utf-8")
    spatial = load_json(ROOT / "conformance/spatial/foundation-vectors.json")
    type_sizes = load_json(ROOT / "conformance/typography/foundation-vectors.json")
    radii = load_json(ROOT / "conformance/geometry/foundation-radius-vectors.json")
    depths = load_json(ROOT / "conformance/elevation/foundation-depth-vectors.json")
    expected = {
        vector["path"]: {"token_type": "dimension", "value": vector["value"]}
        for vector in spatial + type_sizes + radii + depths
    }
    if len(expected) != len(spatial) + len(type_sizes) + len(radii) + len(depths):
        raise ValueError("duplicate foundation token path")
    yield "foundation: authored spatial, type, radius, and depth scales", foundation, expected


def run_backend(command, source, timeout):
    try:
        return subprocess.run(
            command,
            input=source,
            text=True,
            encoding="utf-8",
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise AssertionError(f"backend exceeded {timeout} seconds") from error


def check_case(command, name, source, expected, timeout):
    completed = run_backend(command, source, timeout)
    if expected is None:
        if completed.returncode != 1 or completed.stdout or not completed.stderr.strip():
            raise AssertionError(
                f"invalid source must exit 1 with empty stdout and a diagnostic; "
                f"got exit {completed.returncode}, stdout {completed.stdout[:200]!r}, "
                f"stderr {completed.stderr[:200]!r}"
            )
        return
    if completed.returncode != 0 or completed.stderr:
        raise AssertionError(
            f"valid source must exit 0 without stderr; got exit {completed.returncode}, "
            f"stderr {completed.stderr[:200]!r}"
        )
    try:
        actual = parse_json(completed.stdout, f"backend output for {name}")
    except ValueError as error:
        raise AssertionError(f"backend did not emit one strict JSON document: {error}") from error
    differing = mismatch(actual, expected)
    if differing:
        raise AssertionError(f"output differs from the conformance result at {differing}")
    repeated = run_backend(command, source, timeout)
    if repeated.returncode != 0 or repeated.stderr:
        raise AssertionError("repeating a valid source changed its exit status or emitted a diagnostic")
    try:
        repeated_output = parse_json(repeated.stdout, f"repeat output for {name}")
    except ValueError as error:
        raise AssertionError(f"repeat output is not one strict JSON document: {error}") from error
    differing = mismatch(repeated_output, actual)
    if differing:
        raise AssertionError(f"repeating the same source changed the result at {differing}")


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina token backend using the public stdin/stdout protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    checked = 0
    for name, source, expected in cases():
        try:
            check_case(command, name, source, expected, arguments.timeout)
        except (AssertionError, OSError, ValueError) as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            return 1
        checked += 1
    print(f"Token backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
