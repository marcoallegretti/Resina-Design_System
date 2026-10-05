# Surface readability across background ranges (candidate, 0.1.0)

This contract composes [common edge contrast](73-common-background-contrast.md)
with existing [surface readability](28-surface-readability.md) and
[Frost readability](26-frost-surface-readability.md). It replaces the single edge
adjacent color with a nonempty set of checked opaque ranges. Content readability
still uses the actual resolved body and its separate post-treatment backdrop.
The ranges do not certify content over a spatially varying backdrop.

## Resolution

Bind the complete surface intent through its actual theme and environment.
Preserve existing material, treatment and base-state checks. The public base
surface operation accepts Rest and Focused; component body operations retain
their own validated interaction states. Do not remove Dragging or other states
to enter the base operation.

Every supplied range must already have opaque, channel-ordered bounds. Reject an
empty set at the existing adjacent-background validation stage. Preserve the
existing treatment, foreground, backdrop and threshold validation order for each
operation. Never substitute one range endpoint as a fabricated adjacent color.

For Cast, Elastomer and an opaque Gel body, resolve content contrast against the
actual opaque body and foreground. For Frost, preserve the actual capability and
accessibility resolution, portable body, opaque fallback and post-treatment backdrop. Run the
existing Frost legibility operation before edge selection. Do not force opacity,
change environment flags, weaken the content threshold or infer a backdrop from
edge regions.

Once content readability succeeds, select one edge using Outline, then
OutlineStrong, against every supplied range. Apply the strict, unrounded edge
threshold and report both conservative bounds if neither candidate passes.
Failure publishes no readability result, even when a content fallback succeeded.

The producer owns actual adjacency, range provenance and coverage. Separate flat
regions need separate uniform ranges; shaded regions need proved bounds. A
successful result carries the complete binding, foreground, body, composited
body, content ratio and fallback decision, common edge and Frost representation.
It is readability data, not complete geometry or a render-ready component.

## Reference and evidence

The Rust reference exposes typed generic and Frost operations over range sets.
It adds no source protocol or unchecked range parser. Existing single-color
operations preserve wire shapes, complete results and diagnostic text. Nested
errors retain their sources. A private background dispatch serves the concrete
single-color and range inputs; it introduces no renderer-specific behavior.

Tests replay existing public Frost cases through both range operations and
generic surface cases across all four families, comparing complete single-color
results and failures, including omitted backdrops and rejected states. The eight
[common contrast cases](../conformance/color/common-contrast-cases.json) replay
across Cast, Frost and Elastomer under normal capabilities, reduced transparency
and absent translucency support. They check common selection, state retention
and Frost body/representation agreement. A content failure takes precedence over
incompatible nonempty edge regions; an edge failure cannot publish a partially
readable result.

All tiers follow this law. Actual rendered background coverage, Slider part
geometry and paint assembly, native exposure and visual calibration still need
their own evidence.
