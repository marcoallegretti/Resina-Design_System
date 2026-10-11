import argparse
import copy
import itertools
import json
import math
import sys
from fractions import Fraction
from pathlib import Path

from check_extruded_contour_backend import contour_mismatch
from check_schemas import ROOT, check_case, load_json, parse_json, validator_for
from check_slider_layout_backend import layout_expected, layout_mismatch
from check_slider_part_paint_backend import color, contrast, expected_result, range_contrast
from check_slider_value_backend import slider_value_mismatch


def paint_ranges(body):
    pigments = body["pigment"]
    uniform = lambda value: dict(lower=value, upper=value)
    ranges = [uniform(pigments["body"]), uniform(body["edge"]["color"])]
    if any(body["lighting"]["sideOffset"].values()):
        ranges.append(uniform(pigments["side"]))
    if body["highlightWidth"] > 0 and pigments["profile"]["highlightLift"] > 0:
        margin = 4 * sys.float_info.epsilon
        lower = [max(0, math.nextafter(c - margin, -math.inf)) for c in pigments["body"]["components"]]
        upper = [min(1, math.nextafter(c + margin, math.inf)) for c in pigments["highlight"]["components"]]
        ranges.append(dict(lower=color(lower), upper=color(upper)))
    return ranges


def fixture_expected(config):
    body = load_json(ROOT / "conformance/ir/command-paint-request.json")["surface"]["body"]
    theme = parse_json(body["theme"]["themeSource"], "snapshot arithmetic theme")
    theme["tokens"]["palette"]["dark"] = {"$value": color([0.02] * 3)}
    theme["tokens"]["palette"]["middle"] = {"$value": color([0.5] * 3)}
    theme["materialAssignments"]["control"]["interactive"] = config["family"]
    for assignment in ("colorAssignments", "opaqueColorAssignments"):
        theme[assignment]["roles"].update({"surface.base": "palette.dark", "surface.high": "palette.base",
                                            "outline.strong": "palette.middle", "focus": "palette.middle"})
    environment = body["theme"]["environment"]
    environment.update(layoutDirection=config["direction"], textScale=config["textScale"])
    current = dict(schemaVersion="0.1.0", minimum=-10, maximum=30, value=0)
    horizontal = config["orientation"] == "horizontal"
    visible = dict(current)
    if config["pose"] == "preview":
        visible["value"] = -8 if horizontal and config["direction"] == "rtl" else 8
    layout = layout_expected(dict(schemaVersion="0.1.0", value=visible,
                                  layoutDirection=config["direction"], orientation=config["orientation"],
                                  minimumPosition="start", allocationSize=dict(width=160 if horizontal else 40,
                                  height=40 if horizontal else 160), thumbSize=dict(width=24, height=24),
                                  trackThickness=6, insets=dict(start=8, end=8, top=8, bottom=8)))
    enabled = config["pose"] not in ("disabled", "disabledFocused")
    focused = config["pose"] in ("focused", "disabledFocused", "readOnly", "preview")
    read_only = config["pose"] == "readOnly"
    signals = ["pressed", "dragging"] if config["pose"] == "preview" else ["rest" if enabled else "disabled"]
    if focused:
        signals.insert(0 if signals[0] == "pressed" else 1, "focused")
    surface = body["surface"]
    surface["states"]["states"] = signals
    white, black = color([1] * 3), color([0] * 3)
    canvas = [dict(lower=white, upper=white)]
    appearance = load_json(ROOT / "conformance/appearance/slider-appearance.json")
    request = dict(schemaVersion="0.1.0", part="track", readOnly=read_only,
                   theme=dict(themeSource=json.dumps(theme), environment=environment),
                   surface=surface, size={f: layout["trackBounds"][f] for f in ("width", "height")},
                   appearance=body["appearance"], interactionAppearance=appearance,
                   foregroundRole="content.primary", postTreatmentBackdrop=white,
                   adjacentRanges=canvas, surroundingRanges=canvas,
                   minimumContentContrast=1, minimumEdgeContrast=3)
    track = expected_result(request)
    surrounding = paint_ranges(track["paint"]["body"]) + canvas
    request.update(part="thumb", size={f: layout["thumbBounds"][f] for f in ("width", "height")},
                   adjacentRanges=surrounding, surroundingRanges=surrounding)
    surface["colorRole"] = "surface.high"
    thumb = expected_result(request)
    scale = config["textScale"]
    typography = dict(familyRole="sans", fontSize=dict(value=20 * scale, unit="px"), fontWeight=600,
                      lineHeight=1.4, letterSpacing=dict(value=0.25 * scale, unit="px"))
    label = dict(schemaVersion="0.1.0", text="Volume", typography=typography,
                 size=dict(width=100, height=64), labelBounds=dict(x=0, y=(64 - 28 * scale) / 2,
                 width=100, height=28 * scale), layoutDirection=config["direction"])
    return dict(schemaVersion="0.1.0", presentation=dict(schemaVersion="0.1.0", revision="r0",
                committed=dict(current, progress=0.25), visible=layout["value"],
                valuePolicy=dict(kind="continuous"), editing=config["pose"] == "preview"),
                layout=layout, track=track, thumb=thumb, label=label, labelOrigin=dict(x=190, y=0),
                labelForeground=black, labelBackground=white, labelContrastRatio=21,
                trackContrastRatio=range_contrast(track["paint"]["body"]["edge"]["color"], canvas),
                thumbContrastRatio=range_contrast(thumb["paint"]["body"]["edge"]["color"], surrounding),
                hitRegion=dict(schemaVersion="0.1.0", bounds=dict(x=-12, y=-12, width=320, height=196),
                minimumSize=dict(width=48, height=48)), accessibility=dict(schemaVersion="0.1.0",
                role="slider", name="Volume", description="Output level", value=visible,
                valueText="Current level", state=dict(enabled=enabled, readOnly=read_only, focused=focused),
                orientation=config["orientation"], actions=[dict(kind=k, available=enabled and not read_only)
                for k in ("setValue", "increase", "decrease")], focusable=True, relationships=[]),
                reducedMotion=True)


def check_snapshot(document):
    check_case(validator_for("schemas/slider-snapshot-ir.schema.json"), "Slider snapshot", document, True)
    visible = document["presentation"]["visible"]
    presentation = document["presentation"]
    values = [visible, presentation["committed"]]
    policy = presentation["valuePolicy"]
    if policy["kind"] == "stops":
        stops = policy["stops"]["values"]
        if any(a["value"] >= b["value"] for a, b in zip(stops, stops[1:])):
            raise ValueError("stops must remain strictly ordered")
        if stops[0]["value"] != visible["minimum"] or stops[-1]["value"] != visible["maximum"]:
            raise ValueError("stops must retain both domain endpoints")
        allowed = {stop["value"] for stop in stops}
        if any(value["value"] not in allowed for value in values):
            raise ValueError("committed and visible values must belong to the stop domain")
        values += stops
    for value in values:
        low, high, current = (Fraction(float(value[k])) for k in ("minimum", "maximum", "value"))
        if not low < high or not low <= current <= high:
            raise ValueError("snapshot values must remain in a nondegenerate domain")
        progress = (current - low) / (high - low)
        if 0 < progress < 1 and not 0 < float(progress) < 1:
            raise ValueError("interior value progress must remain representable")
        if slider_value_mismatch(value, dict(value, progress=float(progress))):
            raise ValueError("snapshot value progress disagrees with its domain")
        if any(value[k] != visible[k] for k in ("minimum", "maximum")):
            raise ValueError("snapshot values must share their domain")
    if not presentation["editing"] and presentation["committed"] != visible:
        raise ValueError("nonediting presentation must retain its committed value")
    states = document["track"]["paint"]["body"]["states"]["states"]
    if presentation["editing"] and "pressed" not in states:
        raise ValueError("pointer editing requires contact feedback")
    if "dragging" in states and not presentation["editing"]:
        raise ValueError("dragging requires a pointer edit")
    if "pressed" in states and not presentation["editing"] and not document["accessibility"]["state"]["focused"]:
        raise ValueError("keyboard contact feedback requires actual focus")
    if document["layout"]["value"] != visible:
        raise ValueError("layout must use the visible value")
    layout = document["layout"]
    for field in ("minimumThumbBounds", "maximumThumbBounds"):
        if any(layout[field][k] != layout["thumbBounds"][k] for k in ("width", "height")):
            raise ValueError("travel endpoints must retain the current thumb size")
    axis, cross, extent, cross_extent = (("x", "y", "width", "height") if layout["orientation"] == "horizontal"
                                        else ("y", "x", "height", "width"))
    minimum, maximum = (Fraction(float(layout[k][axis])) for k in ("minimumThumbBounds", "maximumThumbBounds"))
    reverse = layout["orientation"] == "horizontal" and layout["layoutDirection"] == "rtl"
    reverse ^= layout["minimumPosition"] == "end"
    if minimum == maximum or (minimum > maximum) != reverse:
        raise ValueError("travel endpoints must retain the numeric direction")
    expected_layout = copy.deepcopy(layout)
    expected_layout["thumbBounds"][axis] = float(minimum + (maximum - minimum) * Fraction(float(visible["progress"])))
    expected_layout["trackBounds"][axis] = float(min(minimum, maximum) + Fraction(float(layout["minimumThumbBounds"][extent])) / 2)
    expected_layout["trackBounds"][extent] = float(abs(maximum - minimum))
    expected_layout["trackBounds"][cross] = float(Fraction(float(layout["minimumThumbBounds"][cross])) +
        (Fraction(float(layout["minimumThumbBounds"][cross_extent])) - Fraction(float(layout["trackBounds"][cross_extent]))) / 2)
    difference = layout_mismatch(layout, expected_layout)
    if difference:
        raise ValueError("incoherent allocated geometry at " + difference)
    if document["accessibility"]["value"] != {k: v for k, v in visible.items() if k != "progress"}:
        raise ValueError("accessible value must use the visible value")
    if document["accessibility"]["name"] != document["label"]["text"]:
        raise ValueError("accessible name must use the complete label")
    # Python also strips U+001C–U+001F, which are outside Unicode White_Space.
    whitespace = "\t\n\v\f\r \u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000"
    for text in (document["label"]["text"], document["accessibility"]["description"], document["accessibility"]["valueText"]):
        if text is not None and not text.strip(whitespace):
            raise ValueError("supplied semantic text must be nonblank")
    if set(states) != set(document["thumb"]["paint"]["body"]["states"]["states"]):
        raise ValueError("parts must share complete states")
    actual = contrast(document["labelForeground"], document["labelBackground"])
    if abs(actual - document["labelContrastRatio"]) > 1e-12:
        raise ValueError("label contrast report must match its published colors")
    for part in ("track", "thumb"):
        front = document[part]["paint"]["body"]["geometry"]["front"]["bounds"]
        allocation = document["layout"][part + "Bounds"]
        if front != dict(x=0, y=0, width=allocation["width"], height=allocation["height"]):
            raise ValueError(f"{part} front must match its allocation")
        if document[part]["paint"]["body"]["edge"]["contrastRatio"] - document[part + "ContrastRatio"] > 1e-12:
            raise ValueError(f"{part} original edge report exceeds the verified bound")

    def box(bounds, origin=None, offset=None):
        x, y, width, height = (Fraction(float(bounds[k])) for k in ("x", "y", "width", "height"))
        if origin:
            x += Fraction(float(origin["x"]))
            y += Fraction(float(origin["y"]))
        if offset:
            x += Fraction(float(offset["x"]))
            y += Fraction(float(offset["y"]))
        result = x, y, x + width, y + height
        try:
            rounded = tuple(map(float, result))
        except OverflowError:
            raise ValueError("placed bounds must have finite representable endpoints") from None
        if not all(map(math.isfinite, rounded)) or rounded[2] <= rounded[0] or rounded[3] <= rounded[1]:
            raise ValueError("placed bounds must retain positive representable extents")
        return result

    region = document["hitRegion"]
    if any(region["bounds"][k] < region["minimumSize"][k] for k in ("width", "height")):
        raise ValueError("reserved target must meet its published minimum size")
    target = box(region["bounds"])
    label = box(dict(x=0, y=0, **document["label"]["size"]), document["labelOrigin"])
    label_local = box(document["label"]["labelBounds"])
    if label_local[2] > Fraction(float(document["label"]["size"]["width"])) or label_local[3] > Fraction(float(document["label"]["size"]["height"])):
        raise ValueError("complete label bounds must fit their layout slot")
    track = box(document["track"]["paint"]["body"]["geometry"]["silhouette"]["bounds"], layout["trackBounds"])
    footprints = [track]
    contours = [(document["thumb"]["paint"]["body"]["geometry"]["silhouette"]["bounds"], dict(x=0, y=0))]
    if "focus" in document["thumb"]["paint"]:
        focus = document["thumb"]["paint"]["focus"]
        if set(focus["indicator"]["binding"]["states"]["states"]) != set(states):
            raise ValueError("navigation must retain the complete part state set")
        outer = focus["geometry"]["outer"]
        contours.append((outer["contour"]["bounds"], outer["offset"]))
    for bounds, offset in contours:
        endpoints = [box(bounds, layout[k], offset) for k in ("minimumThumbBounds", "maximumThumbBounds")]
        footprints.append(tuple((min if i < 2 else max)(a[i] for a in endpoints) for i in range(4)))
        footprints.append(box(bounds, layout["thumbBounds"], offset))
    for footprint in [label] + footprints:
        if any((footprint[i] < target[i] if i < 2 else footprint[i] > target[i]) for i in range(4)):
            raise ValueError("target must cover the label and full travel paint")
    if any(max(label[0], b[0]) < min(label[2], b[2]) and max(label[1], b[1]) < min(label[3], b[3]) for b in footprints):
        raise ValueError("label must not overlap travel paint")


def check_capture(records):
    keys = ("family", "direction", "orientation", "pose", "textScale")
    if not isinstance(records, list):
        raise ValueError("snapshot capture must be an array of test records")
    for record in records:
        if not isinstance(record, dict) or record.keys() != {"config", "snapshot"}:
            raise ValueError("snapshot capture requires complete named test records")
        config = record["config"]
        if not isinstance(config, dict) or config.keys() != set(keys):
            raise ValueError("snapshot capture requires complete named configurations")
        if any(not isinstance(config[k], str) for k in keys[:-1]) or type(config["textScale"]) not in (int, float):
            raise ValueError("snapshot capture configuration has invalid member types")
    expected = set(itertools.product(("cast", "frost", "elastomer"), ("ltr", "rtl"),
                   ("horizontal", "vertical"), ("rest", "focused", "disabled", "disabledFocused", "readOnly", "preview"), (1, 2)))
    actual = [tuple(r["config"][k] for k in keys) for r in records]
    if len(actual) != len(set(actual)) or set(actual) != expected:
        raise ValueError("incomplete or duplicate snapshot capture matrix")
    for record in records:
        check_snapshot(record["snapshot"])
        difference = contour_mismatch(record["snapshot"], fixture_expected(record["config"]))
        if difference:
            raise ValueError(f"snapshot capture differs at {difference}: {record['config']}")


def main():
    parser = argparse.ArgumentParser(description="Check serialized Slider snapshot coherence or a reference test capture.")
    parser.add_argument("path")
    parser.add_argument("--capture", action="store_true")
    args = parser.parse_args()
    try:
        document = load_json(Path(args.path))
        if args.capture:
            check_capture(document)
        else:
            check_snapshot(document)
    except (AssertionError, KeyError, TypeError, OSError, ValueError, RecursionError) as error:
        print(f"FAIL Slider snapshot IR: {error}", file=sys.stderr)
        return 1
    print("Slider snapshot IR verification passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
