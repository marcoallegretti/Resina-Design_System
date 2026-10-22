# Authored material response calibration (candidate, 0.1.0)

This calibration supplies explicit [scalar dynamics](44-spring-trajectories.md)
for the existing Command/Toggle body and depth channels and Toggle thumb travel.
The definitions are authored response choices, not physical measurements of a
material. They introduce no new motion policy, interpolation, clock or IR.
Consumers still supply complete validated requests and retained initial state.
There are no implicit coefficients or automatic profile lookup.

## Profile ownership and units

| Existing channel | Frost definition | Elastomer definition |
| --- | --- | --- |
| Command/Toggle `bodyMix` | [frost-body](../definitions/motion/frost-body.json) | [elastomer-body](../definitions/motion/elastomer-body.json) |
| Command/Toggle `depthScale` | [frost-depth](../definitions/motion/frost-depth.json) | [elastomer-depth](../definitions/motion/elastomer-depth.json) |
| Toggle checked thumb progress | [toggle-travel](../definitions/motion/toggle-travel.json) | [toggle-travel](../definitions/motion/toggle-travel.json) |

All five documents conform to the existing spring-dynamics schema. Their mass
is normalized to 1 M. Stiffness uses M/s², damping M/s, position thresholds
dimensionless channel units, and velocity thresholds channel units/s. Position
and velocity are explicit per-channel initial conditions, not profile defaults.
Body uses position threshold 0.0001 and velocity threshold 0.01; depth/travel
use 0.001 and 0.05. Changing thresholds changes the actual stopping condition.

Frost is critically damped with natural angular frequency 40 rad/s. It approaches
a resting target without unit-step overshoot. Elastomer keeps the same frequency
but uses damping ratio 0.8 for pigment and 0.7 for depth: a small pigment peak
and stronger controlled compression response. The equations, retained state and
paint-only saturation remain the existing contracts. A depth peak is not in-plane
deformation. Focus and selection semantics stay immediate and independent.

Toggle travel uses critical damping to avoid endpoint overshoot on a unit step
from rest. Interrupted travel retains velocity and can briefly continue in the
previous direction before reversing toward its new target. Its progress is a
separately bounded placement channel; it need not use the depth response's
overshoot. Both current material families
share this travel choice while retaining their distinct body/depth response.

Cast remains immediate under the existing material policy, including at time
zero. Reduced motion takes precedence for every supported family and retains
endpoint feedback. All supplied dynamics must still validate even when ignored
by an immediate policy; this calibration invents no dummy Cast spring.

## Response criteria and method

Measure a dimensionless unit step from position/velocity 0/0 to target 1 with
ordinary motion. The sample grid is 0.0001 seconds over [0,1] seconds; the first
sample at or beyond 90% and the first conservative settled sample are reported
with that grid resolution, not as continuous event times or native cadence.
Settling uses the existing energy-radius rule, including both thresholds.

| Profile | 90% by (s) | Settled by (s) | Unit-step peak | Retarget settled by (s) |
| --- | ---: | ---: | ---: | ---: |
| Frost body | 0.100 | 0.310 | 1, monotone | 0.310 |
| Frost depth | 0.100 | 0.250 | 1, monotone | 0.250 |
| Elastomer body | 0.080 | 0.320 | 1.010–1.020 | 0.320 |
| Elastomer depth | 0.075 | 0.280 | 1.040–1.050 | 0.280 |
| Toggle travel | 0.100 | 0.250 | 1, monotone | 0.250 |

Retarget at elapsed times 0.016, 0.033, 0.050, 0.075, 0.100 and 0.150 seconds
to target 0 using the complete raw returned position and velocity. Time-zero
restart must preserve that state exactly unless the existing stopping rule has
already settled it. The retarget bound counts from the restart, not the original
gesture. These finite conditions do not promise bounded settling after arbitrary
impulses or an indefinitely changing target.

The palette separates stability from response speed. A slower critical trial at
28 rad/s reached 90% at 0.139 s and settled the body at 0.4319 s; the selected
40 rad/s trial gives 0.0973/0.3023 s. A 52 rad/s trial shortened those times to
0.0749/0.2326 s but removed more of the intended restrained response. Elastomer's
0.7 depth damping yields a 4.5988% unit-step peak; raising pigment damping to
0.8 reduces its peak to 1.5165%. These are reproducible design trade-offs,
not evidence of measured human preference or a performance claim.

Reproduce the measurements with
`python tools/material_response_measurements.py`. The independent existing
trajectory equation checker reports the explicit grid, every retained restart
state and settling time. The Rust profile test separately applies the bounds
above to the reference sampler. The [authored trajectory vectors](../conformance/motion/material-response-cases.json)
use closed-form scalar equations at 0.050 s and a retained-velocity reversal,
plus settled and reduced-motion samples. Run
`python tools/check_material_response_backend.py -- <trajectory-command> -`
to verify those cases through an external public protocol, rather than deriving
expected values from the implementation under test.

The defined profiles produce these grid measurements:

| Profile | First 90% (s) | First settled (s) | Peak position | Worst sampled retarget settling (s) |
| --- | ---: | ---: | ---: | ---: |
| Frost body | 0.0973 | 0.3023 | 1 | 0.3022 |
| Frost depth | 0.0973 | 0.2392 | 1 | 0.2391 |
| Elastomer body | 0.0746 | 0.3130 | 1.015165 | 0.3139 |
| Elastomer depth | 0.0658 | 0.2677 | 1.045988 | 0.2703 |
| Toggle travel | 0.0973 | 0.2392 | 1 | 0.2391 |

## Composition and boundaries

Actual phase targets still come from complete authored appearance, with the
existing rest/hover/press/disabled precedence. Selection changes the current
Toggle role immediately; focus remains independently current. On every sampled
frame, validate actual color/edge/focus contrast, content containment, original
reserved target and complete component coherence. Unit-step bounds alone cannot
certify a painted path. Retarget from raw samples; never replace retained
velocity with zero or feed projected paint back into the equation.

Slider's current part paint and value/snapshot contracts are static. Direct
pointer/key value adoption and thumb placement remain immediate, without spring
lag. These definitions do not introduce a sampled Slider paint operation or
animate its numeric value. Authored Slider phase targets and full component
styling remain with their own acceptance work. A new sampled Slider contract
would require a separate explicit architectural decision.

Arithmetic spring, Command and Toggle fixtures retain their original values and
expected results. They continue to exercise damping regimes and diagnostic paths;
they are not silently promoted into these calibrated definitions. Native frame
scheduling, complete component styling, deformation and backend qualification
remain separate contracts. Acceptance of these profiles requires their response
proof and actual bounded paint/visual evidence; determinism alone is insufficient.

## Repeatable native composition evidence

The calibrated Command test samples Light/Dark Frost/Elastomer hover, press and
disable, followed by a release retaining the raw 0.050 s state. Each leg is
sampled at 0, 0.016, 0.050, 0.100, 0.200 and 0.350 s, with German LTR and Arabic
RTL content at text scale 2 and device scales 1.25, 1.5 and 2: 864 complete
frames. Each frame checks the original reserved target, coherent current
snapshot, complete shaped glyph footprints and solid native glyph contrast.

The calibrated Toggle test produces 180 complete frames at device scale 1.25,
covering Light/Dark Cast/Frost/Elastomer, LTR/RTL, checked/unchecked, press,
retarget, disable and reduced motion, plus increased text at scale 2. Track and
thumb use the authored body/depth dynamics; travel uses its separate profile.
Current actual adjacency, selection semantics, retained state, disabled focus
and action availability, target bounds, label ink and containment are checked.
The original arithmetic Toggle test remains independently executable.

For example, in a Linux checkout with the native backend's documented build
dependencies and the pinned static masters:

```bash
export CONFORMANCE_FONTS="$PWD/target/conformance-fonts"
python tools/conformance_font.py fetch --output "$CONFORMANCE_FONTS"
cargo build -p resina-resolver --bin resina-theme-resolve --locked
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > target/material-scenes.json
export RESINA_SCENES="$PWD/target/material-scenes.json"
export RESINA_LABEL_FONT_DIR=$CONFORMANCE_FONTS
RESINA_COMMAND_CAPTURE_DIR="$PWD/target/calibrated-command" cargo test --manifest-path backends/guido/resina-guido/Cargo.toml --locked --features testing --test command_content calibrated_command_profiles_preserve_complete_native_content_on_retarget
RESINA_TOGGLE_CAPTURE_DIR="$PWD/target/calibrated-toggle" cargo test --manifest-path backends/guido/resina-guido/Cargo.toml --locked --features testing --test toggle_content frames_keep_current_adjacency_semantics_and_raw_retarget_state -- --test-threads=1
```

Use fresh capture directories: output refuses to overwrite an existing frame.
The Toggle calibrated frames are in the `calibrated/` child directory. Preserve
the committed source, supplied scene file, actual font and adapter identity when
recording a run. The checked Linux specimens were recorded with DejaVu Sans,
before the label checks moved to the Inter masters, and llvmpipe Vulkan;
they are sampled native composition evidence, not hardware cadence, native
input, assistive-technology qualification or full component-style acceptance.
