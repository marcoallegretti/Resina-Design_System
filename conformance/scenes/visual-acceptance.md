# Visual acceptance evidence

Visual acceptance supplements semantic conformance. Neither deterministic pixels
nor agreement with a previous image makes a material, component or backend
accepted. The [design guide](../../docs/design-language.md) distinguishes current
contracts, candidate styling and proposed behavior.

## Reproducible capture

The [capture record](../../tools/schemas/visual-capture-record.schema.json) preserves a
committed source revision, the complete request and resolved IR, capture bounds,
device/text scales, explicit sample time and retained initial motion channels,
renderer and resolver binary digests, runtime, color encoding and image digest.
The [scene schema](../../tools/schemas/visual-scene.schema.json) refers to existing
requests and results; it does not introduce another Resina IR or material law.
Evidence formats live under `tools/schemas/` in a distinct tool namespace. They
can consume normative contracts; normative schema resolution cannot consume
these renderer/runtime records.

The current producers are the bounded CPU surface, focus, complete paint and
sampled Command raster commands. They draw no text and use no GPU: their font
and GPU fields are explicitly null. Environment text scale is recorded, but
these images cannot establish text rendering, localization or native delivery.
Records are candidates or diagnostics. A producer cannot self-assign accepted
status. Unsupported scene kinds fail explicitly.

Install the existing schema and rendering requirements, build the public
commands and choose a new ignored output directory:

```sh
python -m pip install -r tools/requirements-schema.txt -r tools/requirements-render.txt
cargo build -p resina-resolver -p resina-raster --bins --locked
python tools/capture_material_acceptance.py --bin-dir target/debug --output target/visual-candidate-1
```

On Windows add `--executable-suffix .exe`. Captures require a clean committed
source tree. Existing outputs are preserved. An interrupted suite leaves only
its completed individual records and reports failure; no partial suite is
accepted. Each request is resolved and rendered twice. Both semantic output and
decoded pixels must repeat before its record is written. Validate the relevant
public protocols and schemas independently before evaluating images.

The authored [environment variants](visual-environments.json) retain the source
capabilities and input model. Eight explicit combinations exercise every device
scale (1, 1.25, 1.5, 2), text-context scale (1, 1.5, 2), LTR/RTL and all eight
reduced-motion/transparency/high-contrast combinations. This is a level-coverage
selection, not the full cross product. Each combination captures the sixteen
complete Light/Dark, four-family rest/focus specimens from the existing catalog.
Their actual surrounding color and readability guards remain authored inputs.

Eight additional **diagnostic** Command frames sample press and interrupted
release at explicit times, retaining raw position and velocity at the retarget.
Their numerical response profile comes from the existing arithmetic fixture.
They exercise frame identification and continuity, not accepted production
calibration, native scheduling or complete component content.

The [lit control scenes](lit-control-scenes.json), described by their
[manifest schema](../../schemas/lit-control-scene-manifest.schema.json), add the
Command bodies (standard and primary) and the Toggle track and thumb (unchecked
and checked) as lit control bodies in the Light and Dark themes. Each part is
captured at rest, hovered, pressed, disabled and, where it draws a ring, focused,
with both light channels. The standard Command body and the unchecked track are
also captured hovered with no light, ambient light only and interaction light
only, under high contrast, under reduced motion, with every renderer capability
off and in right-to-left layout. Colors, material families and the response
profiles come from the authored themes and definitions through the theme
backend, with no scene-local palette. The capture adds these as `commandPaint`
and `togglePartPaint` candidate records through the same resolved-paint and
containment gates as the other kinds; they are candidates, not accepted styling,
and the images draw no text.

```sh
python tools/check_lit_control_scenes.py \
  --theme-backend target/debug/resina-theme-resolve \
  --command-backend target/debug/resina-command-paint \
  --toggle-backend target/debug/resina-toggle-part-paint \
  --command-raster target/debug/resina-command-raster \
  --toggle-raster target/debug/resina-toggle-part-raster
```

The checker resolves every scene, compares the resolved paint with the independent
semantics of the same request (phase, response, body colour, light, depth, label
and edge contrast) and compares every raster pixel with bounds from the
[independent lit control body arithmetic](../../tools/lit_body_arithmetic.py): RGB
within one step, exact alpha, at the capture scale and sampling and at scale 1 with
two samples per axis, with repeated output identical. It also requires that
the thumb edge was measured against the resolved track body. For the standard
Command body in each theme it also requires that the pixels of the no-light,
ambient-only, high-contrast, reduced-motion and all-capabilities-off scenes leave
the bounds of the hovered scene with both light channels, and those of the
ambient-only and interaction-only scenes the bounds of the no-light scene, so that the evidence
would notice a light that was not applied or not suppressed; the right-to-left
scene stays within the bounds of the left-to-right scene, because light never
mirrors. That control only shows light does not mirror: the Command body is a
uniform rounded shape and the Toggle parts are capsules, so asymmetric per-corner
shapes are not exercised. Adding ambient light on top of interaction light changes
the pixels by about one RGB step or less, so the interaction-only scene is compared
with the no-light scene, and the margin of the ambient-only control is as small as
1.7 steps against the one-step tolerance (Dark Cast). Scene colours come from
`resina-theme-resolve`, so the checker proves paint and raster given that theme
resolution, which `tools/check_theme_backend.py` covers. Passing it certifies the CPU realization of these scenes against the
arithmetic, not their appearance.

The [visibility table](../materials/lit-control-visibility.json) states how
visible the static key is on each of these bodies, and it is faint on one half of
most of them: the Light `surface.high` highlight is at most 0.0009 ΔE_OK and only
its shading half (0.030 to 0.074) shows; the Dark `surface.high` shading half is
0.0085 to 0.021; the Light track highlight is 0.0025 to 0.0095, the Light thumb
shading 0.006 to 0.027 and the Dark thumb highlight 0.0009 to 0.0086. The table is
evidence about the lit body as authored, not an acceptance of its appearance.

After capture, run the executed negative evidence against the same raster:

```sh
python tools/check_visual_regressions.py --captures target/visual-candidate-1 --raster target/debug/resina-paint-raster
```

The checker renders altered radius, reversed key light, missing focus and a
clipped viewport from real public requests. It also substitutes a later
diagnostic motion sample. All five changes must be observable at the relevant
geometry, context or pixel gate; unrelated execution failures do not count as
successful regression detection.

## Separate semantic and image gates

### Native text readback

The existing GUIdo label tests include short labels, a longer German label,
Arabic shaping, the shared label-expansion cases and explicit hard breaks.
Their line-coverage, unconstrained-ink and glyph-overhang checks are separate
from material paint. `label_render` includes device scale 1.5 alongside its
existing scales and text scales 1, 1.5 and 2.

With `RESINA_LABEL_CAPTURE_DIR` set to a new ignored directory, the complete
label test also writes tagged PNGs and [native label records](../../tools/schemas/native-label-capture.schema.json).
Each record contains the actual measured label IR, scales, static sample state,
source commit, the resolved font master (family, PostScript name, weight and
file identity), test-binary identity,
runtime and the GPU adapter/driver from the context that rendered the frame.
File identities use Git blob hashes; they identify bytes, not acceptance.
The producer rejects an uncommitted tree and never overwrites an existing image
or record. Existing PPM diagnostic captures remain available.

```sh
export CONFORMANCE_FONTS="$PWD/target/conformance-fonts"
python tools/conformance_font.py fetch --output "$CONFORMANCE_FONTS"
RESINA_LABEL_FONT_DIR=$CONFORMANCE_FONTS \
RESINA_LABEL_CAPTURE_DIR="$PWD/target/native-label-candidate-1" \
cargo test --manifest-path backends/guido/resina-guido/Cargo.toml \
  --features testing --locked --test label_render --test label_ink
```

These are black-on-white diagnostic layout/ink probes. They do not establish
authored component styling, actual native input or assistive-technology delivery.
Native label comparison checks font/GPU identity, complete semantic layout,
content containment, scales and the PNG's recorded digest before applying the
same conservative pixel gate. Cross-GPU tolerance is not inferred.

```sh
python tools/visual_acceptance.py compare path/to/before/capture.json path/to/after/capture.json
```

The checker validates records, request/result schemas, material and state
bindings, actual content/edge guards, body/navigation coherence and containment
of required paint. Content, edge and focus ratios are recomputed from their
actual opaque colors. Front dimensions, physical light direction and paint-band
widths must match the request. Full contour resolution still requires the
independent public protocol cases and baseline geometry comparison.
Sampled Command results are also checked against the
independent trajectory/paint checker. PNG dimensions and digest must match the
record. Complete requests, viewport, sample state, font and scales must agree
before comparison; a changed source commit or recorded host alone does not
invalidate a comparison. Resolved semantics and geometry use the existing
`1e-12` absolute numerical comparator. Context or semantic changes require an
explained contract/design change and a newly evaluated baseline.

Images must be single-frame straight RGBA8 PNG with perceptual sRGB encoding.
No resampling, color-profile substitution or dimension normalization occurs.
The tool reports changed pixels, maximum and mean Oklab distance and maximum
alpha difference. Oklab conversion follows [the pinned color contract](../../spec/04-color-conversion.md)
and is checked against its independent public vectors. Color distance is
evaluated after linear-light compositing on both black and white, with alpha
checked separately. Hidden RGB at zero alpha does not invent visible color
distance, but still counts as changed bytes.

For an identical CPU producer, request and sampling grid, the regression gate
is **zero changed decoded pixels**. This conservative criterion does not invent
a just-noticeable threshold or let a localized focus/clipping defect disappear
inside a whole-image average. Perceptual metrics explain a difference; they do
not excuse it or establish identity. Cross-renderer tolerance is unsupported
until it has separately justified and accepted bounds.

Unit/schema evidence rejects missing metadata, invented acceptance, wrong image
encoding/dimensions/digests, changed light/context, missing focus and clipped
paint. Single-pixel and alpha changes remain observable. Deliberate rendered
geometry, lighting and motion regressions require the executed negative capture
evidence, in addition to these format and comparison tests.

## Acceptance and remaining coverage

Before a seed becomes an accepted baseline, inspect its actual images together
with semantic and native evidence. Record the acceptance decision, exact source
revision, scenario/capture identity and accepted bounds. Changes require an
explained design or contract change and renewed acceptance; never regenerate an
expected image solely to remove a failure. Diagnostic response/profile fixtures
cannot be promoted into accepted component styling.

The [accepted opaque seed](accepted-opaque-seed.md) records the bounded current
rest/focus baseline, its immutable revision, exact set and reproduction gates.

| Coverage | Current evidence boundary | Owning acceptance work |
| --- | --- | --- |
| Four families, Light/Dark, opaque rest/focus, scale/direction/preferences | Accepted bounded static seed; explicit semantic/containment and exact pixel gates | #127: accepted opaque seed; new contracts append separately accepted cases |
| Long translated labels, shaped RTL script and actual text scaling | 90 repeated and inspected GUIdo diagnostic readback records with font/runtime/GPU identity; text-free CPU records do not cover these | #127: diagnostic measurement/readback evidence; authored component styling remains #134/#139/#140 |
| Neutral/primary controls and hover/press/disabled/selected styling | Existing contracts, diagnostic component fixtures and the CPU lit control scenes in both themes; shared production profiles and the scenes' appearance remain unaccepted | #134: authored profiles, followed by #139/#140 realization; #129: native realization and acceptance of the lit scenes |
| Interrupted motion | Diagnostic sampled Command paint with retained state; no accepted calibration | #133: accepted response profiles and criteria |
| Bright/dark/busy backdrops with enhanced material response | Current static catalog uses known uniform surroundings; diffusion/optics not implemented | #130/#131: shared contracts and new scenarios |
| Deformation, emergence and environmental pigment | No corresponding shared rendering contract | #132/#138: decisions, contracts and appended scenarios |
| Checkbox and Segmented Control | No shared approved semantics/anatomy | #136/#137: decisions and independent proof |
| Native interaction and assistive-technology delivery | Separate native qualification contracts; images are insufficient | #108/#109/#118 and #139/#140 |

Blocked and unexecuted rows never count as passing visual acceptance. The capture
tool reports reproducibility only. This document does not declare #127, the
integrated material identity or a release stage complete.
