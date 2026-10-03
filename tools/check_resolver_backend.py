import argparse
import json
import math
import sys

from check_schemas import ROOT, load_json, validator_for
from check_token_backend import check_case


def request_source(resolver, input_source, external_sources):
    return json.dumps(
        {
            "schemaVersion": "0.1.0",
            "resolver": resolver,
            "input": input_source,
            "externalSources": external_sources,
        },
        ensure_ascii=False,
        allow_nan=False,
    )


def cases():
    case_validator = validator_for("schemas/resolver-module-case.schema.json")
    request_validator = validator_for("schemas/resolver-module-request.schema.json")
    vectors = load_json(ROOT / "conformance/tokens/resolver-module-vectors.json")
    names = set()
    for vector in vectors:
        errors = list(case_validator.iter_errors(vector))
        if errors:
            raise ValueError(f"{vector.get('name')}: {errors[0].message}")
        name = vector["name"]
        if name in names:
            raise ValueError(f"duplicate resolver module case name: {name}")
        names.add(name)
        source = request_source(vector["resolver"], vector["input"], vector["externalSources"])
        request_errors = list(request_validator.iter_errors(json.loads(source)))
        if request_errors:
            raise ValueError(f"{name}: {request_errors[0].message}")
        yield name, source, vector.get("expected")

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
    resolver = json.dumps(
        {
            "version": "2025.10",
            "resolutionOrder": [
                {
                    "type": "set",
                    "name": "foundation",
                    "sources": [{"$ref": "foundation.json"}],
                }
            ],
        }
    )
    yield (
        "foundation: authored spatial, type, radius, and depth scales",
        request_source(resolver, "{}", {"foundation.json": foundation}),
        expected,
    )

    baseline = request_source(resolver, "{}", {"foundation.json": foundation})
    yield "request: duplicate envelope member", baseline.replace(
        '"schemaVersion": "0.1.0",',
        '"schemaVersion": "0.1.0", "schemaVersion": "0.1.0",',
        1,
    ), None
    for name, change in (
        ("unsupported schema version", {"schemaVersion": "0.2.0"}),
        ("unknown request member", {"unknown": True}),
        ("missing input source", {"input": None}),
    ):
        request = json.loads(baseline)
        request.update(change)
        if name == "missing input source":
            del request["input"]
        yield f"request: {name}", json.dumps(request, ensure_ascii=False), None


def main():
    parser = argparse.ArgumentParser(
        description="Check a DTCG resolver backend using the Resina stdin/stdout protocol."
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
    print(f"Resolver backend passed {checked} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
