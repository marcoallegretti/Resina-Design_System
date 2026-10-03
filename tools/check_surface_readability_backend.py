import argparse
import json
import math
import sys

from check_frost_surface_readability_backend import baseline as frost_baseline
from check_headless_backend import mismatch, run_backend
from check_schemas import ROOT, apply_changes, load_json, parse_json, validator_for


def baseline():
    request, _ = frost_baseline()
    request["scenario"]["resolution"]["opaqueColorAssignments"]["roles"]["content.primary"] = (
        "palette.opaqueAlt"
    )
    return request


def token_color(request, role, opaque):
    resolution = request["scenario"]["resolution"]
    assignments = "opaqueColorAssignments" if opaque else "colorAssignments"
    path = resolution[assignments]["roles"][role].split(".", 1)[1]
    value = resolution["tokens"]["palette"][path]["$value"]
    return {"colorSpace": "srgb", "components": value["components"], "alpha": 1}


def contrast(foreground, background):
    def luminance(color):
        channels = [
            value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4
            for value in color["components"]
        ]
        return sum(a * b for a, b in zip(channels, (0.2126, 0.7152, 0.0722)))

    high, low = sorted((luminance(foreground), luminance(background)), reverse=True)
    return (high + 0.05) / (low + 0.05)


def check_success(actual, request, expected):
    binding = actual["binding"]
    family = expected["family"]
    if binding["materialFamily"] != family:
        raise AssertionError("material family changed")
    for field in ("materialRole", "colorRole", "form", "states", "treatmentStack"):
        expected_field = request["scenario"]["surface"][field]
        if field == "states":
            expected_field = {**expected_field, "states": [
                state for state in ("rest", "focused") if state in expected_field["states"]
            ]}
        if mismatch(binding[field], expected_field):
            raise AssertionError(f"bound {field} changed")
    if actual["foregroundRole"] != request["foregroundRole"]:
        raise AssertionError("foreground role changed")
    foreground = token_color(request, request["foregroundRole"], family != "frost")
    if mismatch(actual["foreground"], foreground):
        raise AssertionError("foreground differs from the selected role fallback")

    if family == "frost":
        if actual.get("frostRepresentation") != expected["frostRepresentation"]:
            raise AssertionError("Frost representation changed")
        body = (
            binding["opaqueColorFallback"]
            if expected["contentFallback"]
            else binding["frostPortableBody"]
        )
    else:
        if "frostRepresentation" in actual:
            raise AssertionError("non-Frost surface acquired a Frost representation")
        body = binding["opaqueColorFallback"]
    if mismatch(actual["body"], body):
        raise AssertionError("body differs from its bound fallback")
    if actual["contentFallbackApplied"] != expected["contentFallback"]:
        raise AssertionError("content fallback flag changed")
    if body["alpha"] == 1:
        composited = body
    else:
        backdrop = request["postTreatmentBackdrop"]
        composited = {
            "colorSpace": "srgb",
            "components": [
                source * body["alpha"] + behind * (1 - body["alpha"])
                for source, behind in zip(body["components"], backdrop["components"])
            ],
            "alpha": 1,
        }
    if mismatch(actual["compositedBody"], composited):
        raise AssertionError("composited body differs from source-over calculation")
    ratio = contrast(foreground, composited)
    if not math.isclose(actual["contentContrastRatio"], ratio, rel_tol=0, abs_tol=1e-12):
        raise AssertionError("content contrast ratio is incorrect")
    if ratio < request["minimumContentContrast"]:
        raise AssertionError("content contrast is below the requested threshold")

    edge = actual["edge"]
    if edge["colorRole"] != expected["edgeRole"]:
        raise AssertionError("edge role changed")
    if edge["fallbackApplied"] != (expected["edgeRole"] == "outline.strong"):
        raise AssertionError("edge fallback flag changed")
    edge_color = token_color(request, edge["colorRole"], True)
    if mismatch(edge["color"], edge_color):
        raise AssertionError("edge color differs from its authored fallback")
    edge_ratio = contrast(edge_color, request["adjacentColor"])
    if not math.isclose(edge["contrastRatio"], edge_ratio, rel_tol=0, abs_tol=1e-12):
        raise AssertionError("edge contrast ratio is incorrect")
    if edge_ratio < request["minimumEdgeContrast"]:
        raise AssertionError("edge contrast is below the requested threshold")


def main():
    parser = argparse.ArgumentParser(
        description="Check a Resina base-state surface readability backend."
    )
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    arguments = parser.parse_args()
    command = arguments.command
    if command and command[0] == "--":
        command = command[1:]
    if not command or arguments.timeout <= 0 or not math.isfinite(arguments.timeout):
        parser.error("provide a backend command and a positive finite timeout")

    base = baseline()
    cases = load_json(ROOT / "conformance/surfaces/readability-cases.json")
    case_validator = validator_for("schemas/surface-readability-case.schema.json")
    request_validator = validator_for("schemas/surface-readability-request.schema.json")
    result_validator = validator_for("schemas/surface-readability-result.schema.json")
    names = set()
    try:
        for case in cases:
            name = case["name"]
            if name in names or list(case_validator.iter_errors(case)):
                raise ValueError(f"duplicate or invalid case: {name}")
            names.add(name)
            request = apply_changes(base, case["changes"])
            if case.get("omitBackdrop", False):
                del request["postTreatmentBackdrop"]
            valid = not list(request_validator.iter_errors(request))
            if valid != case["requestSchemaValid"]:
                raise ValueError(f"request schema disagrees with case: {name}")
            if "expected" in case and not valid:
                raise ValueError(f"successful request violates schema: {name}")
            source = json.dumps(request, ensure_ascii=False, allow_nan=False)
            completed = run_backend(command, source, arguments.timeout)
            if "errorContains" in case:
                if (
                    completed.returncode != 1
                    or completed.stdout
                    or case["errorContains"] not in completed.stderr
                ):
                    raise AssertionError(
                        f"{name}: expected diagnostic failure; "
                        f"got {completed.returncode}, {completed.stderr[:200]!r}"
                    )
                continue
            if completed.returncode != 0 or completed.stderr:
                raise AssertionError(
                    f"{name}: expected clean success; "
                    f"got {completed.returncode}, {completed.stderr[:200]!r}"
                )
            actual = parse_json(completed.stdout, name)
            errors = list(result_validator.iter_errors(actual))
            if errors:
                raise AssertionError(f"{name}: result violates schema: {errors[0].message}")
            check_success(actual, request, case["expected"])
            repeated = run_backend(command, source, arguments.timeout)
            if (
                repeated.returncode != 0
                or repeated.stderr
                or mismatch(parse_json(repeated.stdout, name), actual)
            ):
                raise AssertionError(f"{name}: repeated request changed its result")
        source = json.dumps(base)
        member = '"foregroundRole": "content.primary"'
        if source.count(member) != 1:
            raise ValueError("baseline foreground role is not unique")
        duplicate = source.replace(member, member + ", " + member, 1)
        completed = run_backend(command, duplicate, arguments.timeout)
        if (
            completed.returncode != 1
            or completed.stdout
            or "duplicate JSON member" not in completed.stderr
        ):
            raise AssertionError("duplicate request member was accepted")
    except (AssertionError, OSError, ValueError) as error:
        print(f"FAIL {error}", file=sys.stderr)
        return 1
    print(f"Surface readability backend passed {len(cases) + 1} conformance cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
