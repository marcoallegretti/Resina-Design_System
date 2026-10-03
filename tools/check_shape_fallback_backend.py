import argparse
import copy
import json
import math
import sys

from backend_source import duplicate_member_source
from check_color_guard_backend import check_failure, check_success
from check_schemas import ROOT, load_json, validator_for


def check_request(validator, request, expected_valid, name):
    errors = list(validator.iter_errors(request))
    if bool(errors) == expected_valid:
        detail = errors[0].message if errors else "unexpectedly valid"
        raise ValueError(f"{name}: request schema: {detail}")


def main():
    parser = argparse.ArgumentParser(
        description="Check Tier 0 shape fallback through the public command protocol."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    request_validator = validator_for("schemas/shape-fallback-request.schema.json")
    result_validator = validator_for("schemas/shape-fallback-result.schema.json")
    base = {
        "schemaVersion": "0.1.0",
        "tokens": load_json(ROOT / "tokens/foundation.json"),
        "assignments": load_json(ROOT / "definitions/tier0-shapes.json"),
        "shape": "structural",
        "size": {"width": 200, "height": 80},
    }
    vectors = load_json(ROOT / "conformance/geometry/shape-fallback-vectors.json")
    count = 0
    try:
        for vector in vectors:
            request = {**base, "shape": vector["shape"], "size": vector["size"]}
            check_request(request_validator, request, True, vector["name"])
            check_success(
                command,
                json.dumps(request, ensure_ascii=False),
                {"schemaVersion": "0.1.0", "radii": vector["expected"]},
                result_validator,
                arguments.timeout,
                vector["name"],
            )
            count += 1

        custom = copy.deepcopy(base)
        custom["assignments"] = load_json(
            ROOT / "conformance/geometry/shape-fallback-assignment-vectors.json"
        )[0]["document"]
        custom["shape"] = "soft"
        check_request(request_validator, custom, True, "custom assignment")
        check_success(
            command,
            json.dumps(custom, ensure_ascii=False),
            {
                "schemaVersion": "0.1.0",
                "radii": {
                    corner: {"x": 8, "y": 8}
                    for corner in ("topStart", "topEnd", "bottomEnd", "bottomStart")
                },
            },
            result_validator,
            arguments.timeout,
            "custom assignment",
        )
        count += 1

        aliased = copy.deepcopy(base)
        aliased["tokens"]["radius"]["3"]["$value"] = "{space.3}"
        check_request(request_validator, aliased, True, "aliased radius")
        structural = next(vector for vector in vectors if vector["shape"] == "structural")
        check_success(
            command,
            json.dumps(aliased, ensure_ascii=False),
            {"schemaVersion": "0.1.0", "radii": structural["expected"]},
            result_validator,
            arguments.timeout,
            "aliased radius",
        )
        count += 1

        failures = [
            (
                "missing shape",
                {key: value for key, value in base.items() if key != "shape"},
                False,
            ),
            ("unknown shape", {**base, "shape": "pill"}, False),
            ("negative bounds", {**base, "size": {"width": -1, "height": 80}}, False),
            ("unsupported version", {**base, "schemaVersion": "0.2.0"}, False),
        ]
        for name, request, schema_valid in failures:
            check_request(request_validator, request, schema_valid, name)
            check_failure(command, json.dumps(request), arguments.timeout, name)
            count += 1

        missing_token = copy.deepcopy(base)
        missing_token["assignments"]["profiles"]["structural"]["radius"] = "radius.missing"
        check_request(request_validator, missing_token, True, "missing token")
        check_failure(command, json.dumps(missing_token), arguments.timeout, "missing token")
        count += 1

        wrong_unit = copy.deepcopy(base)
        wrong_unit["tokens"]["radius"]["3"]["$value"]["unit"] = "rem"
        check_request(request_validator, wrong_unit, True, "unsupported unit")
        check_failure(command, json.dumps(wrong_unit), arguments.timeout, "unsupported unit")
        count += 1

        wrong_type = copy.deepcopy(base)
        wrong_type["tokens"]["radius"]["3"] = {
            "$type": "color",
            "$value": {"colorSpace": "srgb", "components": [0, 0, 0]},
        }
        check_request(request_validator, wrong_type, True, "wrong token type")
        check_failure(command, json.dumps(wrong_type), arguments.timeout, "wrong token type")
        count += 1

        invalid_assignment = copy.deepcopy(base)
        invalid_assignment["assignments"]["profiles"]["structural"] = {"kind": "capsule"}
        check_request(request_validator, invalid_assignment, False, "invalid assignment")
        check_failure(
            command, json.dumps(invalid_assignment), arguments.timeout, "invalid assignment"
        )
        count += 1

        for name, source in (
            ("duplicate request member", duplicate_member_source(base, "/schemaVersion")),
            (
                "duplicate token member",
                duplicate_member_source(base, "/tokens/radius/3/$value/unit"),
            ),
        ):
            check_failure(command, source, arguments.timeout, name)
            count += 1
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Tier 0 shape backend passed {count} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
