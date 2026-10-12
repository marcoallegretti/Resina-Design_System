# Accepted opaque rest/focus seed

The bounded opaque seed at source revision
`4806e447892fdd31f75d48be675c6261ea35e708` is accepted for regression comparison.
It covers the existing complete static opaque paint contracts, with no change
to material semantics or component defaults. Acceptance does not establish a
release stage or integrated backend qualification.

## Accepted set and bounds

The exact set contains 128 captures: the eight environment names below, each
with Light and Dark, Cast/Elastomer/Frost/Gel, and rest/focused paint. Scenario
names follow `<theme>-<family>-paint-<state>`.

```text
standard-ltr
motion-rtl
opaque-ltr
contrast-rtl
motion-opaque-ltr
motion-contrast-rtl
opaque-contrast-ltr
all-preferences-rtl
```

The source revision pins the authored catalog, complete requests, resolved
semantics, surroundings, environment variants and capture grids. Regenerate
the baseline from that immutable revision; do not regenerate it from the
candidate revision. Capture records retain the actual producer identities.
The comparison requires matching complete context, validated semantic/geometry
results within the existing absolute `1e-12` bound, and **zero changed decoded
RGBA8 pixels**. No cross-renderer or cross-GPU tolerance is accepted.

Image inspection covered the opaque front/silhouette, edge/highlight separation,
family hierarchy, focus continuity and containment, Light/Dark and every
authored environment variant at device scales 1, 1.25, 1.5 and 2. The minimum
measured contrast ratios across the set were 7.129 for content, 3.607 for the
edge and 5.378 for focus. These observations support this particular seed;
they do not replace the request's independently checked readability guards.
Two complete captures repeated without semantic or decoded-pixel changes.
Executed radius, reversed-light, clipped-viewport, missing-focus and substituted
motion-frame negatives demonstrated regression detection.

## Reproduce and compare

Use a clean checkout of the pinned revision, install its rendering/schema
requirements, build its locked public commands and run
`tools/capture_material_acceptance.py` as described in
[visual acceptance evidence](visual-acceptance.md). Choose a new ignored output
directory. Repeat the capture from the clean candidate checkout with its own
built commands. Retain both collections and their records.

Run this comparison from the candidate checkout, supplying the two collection
paths. On Windows use its Python executable in place of `python`.

```sh
python - baseline-output candidate-output <<'PY'
import itertools
import json
from pathlib import Path
import sys

sys.path.insert(0, "tools")
from visual_acceptance import compare_records

revision = "4806e447892fdd31f75d48be675c6261ea35e708"
environments = (
    "standard-ltr", "motion-rtl", "opaque-ltr", "contrast-rtl",
    "motion-opaque-ltr", "motion-contrast-rtl", "opaque-contrast-ltr",
    "all-preferences-rtl",
)
scenes = [f"{theme}-{family}-paint-{state}" for theme, family, state in
          itertools.product(("light", "dark"), ("cast", "elastomer", "frost", "gel"),
                            ("rest", "focused"))]
expected = {Path(environment) / scene / "capture.json"
            for environment, scene in itertools.product(environments, scenes)}
before, after = map(Path, sys.argv[1:])
for root in (before, after):
    actual = {path.relative_to(root) for path in root.glob("*/*/capture.json")
              if path.relative_to(root).parts[0] != "motion"}
    if actual != expected:
        raise ValueError("opaque collection differs from the accepted 128-record set")
for relative in sorted(expected):
    baseline = json.loads((before / relative).read_text(encoding="utf-8"))
    if baseline["commit"] != revision or baseline["purpose"] != "candidate":
        raise ValueError("baseline is not the pinned opaque seed")
    metrics = compare_records(before / relative, after / relative)
    if metrics["changedPixels"]:
        raise ValueError(f"opaque seed regression: {relative}: {metrics}")
print("Accepted opaque seed: all 128 semantic and pixel comparisons passed")
PY
```

Changing an accepted context, semantic result or pixel requires an explained
contract/design change and renewed acceptance. A reproducible difference is
still a difference; never update the baseline solely to remove a failure.

## Explicit exclusions

The eight sampled Command motion records remain arithmetic diagnostics.
The 90 repeated native black-on-white label captures support measured text
containment and repeatable readback with the recorded DejaVu Sans and llvmpipe
Vulkan environment. They remain diagnostic layout/ink probes, including
expanded German and shaped Arabic; fixed narrow wrapping does not establish
accepted component proportions or typography styling.

Enhanced Frost/Lens response, deformation, environmental pigment, authored
control profiles, neutral/primary and additional interaction-state styling,
busy backdrops, native input and assistive-technology qualification retain the
owning work listed in the [coverage matrix](visual-acceptance.md#acceptance-and-remaining-coverage).
The seed's text-free CPU images cannot establish those behaviors.
