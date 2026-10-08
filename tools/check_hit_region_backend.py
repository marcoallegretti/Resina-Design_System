import argparse
import json
import math
import sys
from fractions import Fraction

from backend_source import duplicate_member_source, nonfinite_member_source
from check_color_guard_backend import check_failure, check_success
from check_schemas import ROOT, apply_changes, check_case, load_json, validator_for


def baseline():
    return load_json(ROOT / "conformance/interaction/hit-region-request.json")


def cases():
    base = baseline()
    public = load_json(ROOT / "conformance/interaction/hit-region-cases.json")
    for vector in load_json(ROOT / "conformance/environment/source-shape-vectors.json"):
        public.append({
            "name": f"invalid environment shape: {vector['name']}",
            "requestChanges": [{"path": "/environment" + vector["path"], "value": vector["value"]}],
            "requestSchemaValid": False,
            "failure": True,
        })
    return [{**case, "request": apply_changes(base, case["requestChanges"])} for case in public]


def hit_region_mismatch(actual, expected):
    return None if actual == expected else "/"


def validate_membership_vectors(vectors=None):
    if vectors is None:
        vectors = load_json(ROOT / "conformance/interaction/hit-membership-vectors.json")
    if not vectors:
        raise ValueError("hit membership vectors must not be empty")
    validator = validator_for("schemas/hit-membership-case.schema.json")
    names = set()
    for vector in vectors:
        name = vector["name"]
        check_case(validator, name, vector, True)
        if name in names:
            raise ValueError(f"duplicate hit membership case: {name}")
        names.add(name)
        bounds = vector["bounds"]
        if min(bounds["width"], bounds["height"]) < 24:
            raise ValueError(f"{name}: bounds must be a complete fine-input target")
        left, top, width, height = (Fraction(float(bounds[field])) for field in ("x", "y", "width", "height"))
        for point in vector["points"]:
            x, y = Fraction(float(point["x"])), Fraction(float(point["y"]))
            expected = left <= x < left + width and top <= y < top + height
            if point["expected"] != expected:
                raise ValueError(f"{name}: membership disagrees with exact arithmetic")
    return len(vectors)


def main():
    parser = argparse.ArgumentParser(description="Check portable Resina hit-region placement.")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or not math.isfinite(arguments.timeout) or arguments.timeout <= 0:
        parser.error("provide a backend command and a positive finite timeout")
    base = baseline()
    missing = dict(base)
    del missing["occupiedRegions"]
    failures = (
        ("duplicate nested bounds", duplicate_member_source(base, "/visualBounds/x")),
        ("nonfinite extent", nonfinite_member_source(base, "/visualBounds/width")),
        ("missing neighbors", json.dumps(missing)),
        ("unknown root", json.dumps({**base, "unexpected": True})),
        ("positional root", json.dumps([base[field] for field in (
            "schemaVersion", "environment", "visualBounds", "availableBounds",
            "componentMinimum", "occupiedRegions",
        )])),
    )
    case_validator = validator_for("schemas/hit-region-case.schema.json")
    request_validator = validator_for("schemas/hit-region-request.schema.json")
    result_validator = validator_for("schemas/hit-region-ir.schema.json")
    public_cases = cases()
    names = set()
    try:
        validate_membership_vectors()
        for case in public_cases:
            name = case["name"]
            check_case(case_validator, name, {key: value for key, value in case.items() if key != "request"}, True)
            if name in names:
                raise ValueError(f"duplicate hit region case: {name}")
            names.add(name)
            check_case(request_validator, name, case["request"], case["requestSchemaValid"])
            source = json.dumps(case["request"], ensure_ascii=False, allow_nan=False)
            if "expected" in case:
                check_success(command, source, case["expected"], result_validator, arguments.timeout, name, hit_region_mismatch)
            else:
                check_failure(command, source, arguments.timeout, name)
        check_failure(command, duplicate_member_source(base, "/schemaVersion"), arguments.timeout, "duplicate root version")
        for name, source in failures:
            check_failure(command, source, arguments.timeout, name)
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL hit region: {error}", file=sys.stderr)
        return 1
    print(f"Hit region backend passed {len(public_cases) + len(failures) + 1} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
