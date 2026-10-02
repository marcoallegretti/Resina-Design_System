import argparse
import json
import math
import sys

from check_headless_backend import check_case
from check_schemas import ROOT, load_json, validator_for


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina surface scenario backend using the public stdin/stdout protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    resolution = load_json(ROOT / "conformance/headless/valid-request.json")
    vectors = load_json(ROOT / "conformance/surfaces/binding-vectors.json")
    if not vectors or "expected" not in vectors[0]:
        raise ValueError("surface vectors must begin with a valid baseline")
    scenario_validator = validator_for("schemas/surface-scenario.schema.json")
    result_validator = validator_for("schemas/surface-binding-result.schema.json")
    case_validator = validator_for("schemas/headless-conformance-case.schema.json")
    names = set()
    for vector in vectors:
        name = vector["name"]
        if name in names:
            raise ValueError(f"duplicate surface vector name: {name}")
        names.add(name)
        valid = "expected" in vector
        if valid == ("error" in vector):
            raise ValueError(f"surface vector must have exactly one outcome: {name}")
        scenario = {
            "schemaVersion": "0.4.0",
            "resolution": resolution,
            "surface": vector["document"],
        }
        if valid:
            errors = list(scenario_validator.iter_errors(scenario))
            if errors:
                raise ValueError(f"valid surface scenario violates the schema: {name}: {errors[0].message}")
            errors = list(result_validator.iter_errors(vector["expected"]))
            if errors:
                raise ValueError(f"surface result violates the schema: {name}: {errors[0].message}")
        case = {"name": name, "outcome": "valid" if valid else "invalid"}
        source = json.dumps(scenario, ensure_ascii=False)
        try:
            check_case(
                command,
                case,
                scenario,
                vector.get("expected"),
                source,
                result_validator,
                arguments.timeout,
            )
        except (AssertionError, OSError, ValueError) as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            return 1

    base = {
        "schemaVersion": "0.4.0",
        "resolution": resolution,
        "surface": vectors[0]["document"],
    }
    source = json.dumps(base, ensure_ascii=False)
    cases = load_json(ROOT / "conformance/surfaces/scenario-cases.json")
    for case in cases:
        errors = list(case_validator.iter_errors(case))
        if errors:
            raise ValueError(f"invalid surface scenario case: {errors[0].message}")
        name = case["name"]
        if name in names:
            raise ValueError(f"duplicate surface case name: {name}")
        names.add(name)
        try:
            check_case(
                command,
                case,
                base,
                vectors[0]["expected"],
                source,
                result_validator,
                arguments.timeout,
            )
        except (AssertionError, OSError, ValueError) as error:
            print(f"FAIL {name}: {error}", file=sys.stderr)
            return 1
    print(f"Surface backend passed {len(vectors) + len(cases)} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
