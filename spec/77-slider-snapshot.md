# Coherent Slider snapshot (candidate, 0.1.0)

Blueprint sections85,98 and177 require one control whose presented value,
appearance, semantics and interaction target agree. This contract joins checked
[presentation](69-slider-presentation.md), [interaction projection](70-slider-states.md),
[part allocation](61-slider-layout.md) and [part paint](76-slider-part-paint.md).
It does not adopt an adjustment, acquire native routing or invent a value label.

## Coherence

Supply the current presentation and pointer state, enabled/readOnly/focused/
hovered/keyPressed observations, current layout, track and thumb paint, a complete
measured localized label and its placement/colors, description and value text,
focusability, actual canvas color ranges, contrast requirements, environment,
one already reserved target, clipping bounds, minimum target size and neighbors.

The part slots MUST be correct. Both parts' complete state sets and readOnly flags
must match the current interaction projection. That projection verifies the
presentation against the pointer's actual edit, revision and domain. Layout must
use the visible value, including preview; accessibility uses that same value.
Layout, label and part paint directions must match the actual environment.
The reference part paint retains the direction used during its resolution.
A pointer hold must still own the same reserved control bounds. No stale
presentation, paint, value, direction or target is silently repaired.

Each part's front geometry must have local origin zero and the exact dimensions
of its allocated rectangle. Slider anatomy permits a thumb wider than the track;
the Toggle rule requiring thumb containment inside track content does not apply.

## Background evidence

Canvas ranges MUST be nonempty and cover all actual canvas colors adjacent to
the parts and thumb navigation throughout travel. The track edge must pass its
explicit threshold against every canvas range. Combine those ranges with the
track's complete [paint color cover](75-opaque-paint-color-ranges.md), including
side pigment, exterior edge and continuous highlights. The thumb edge must pass
its threshold against every combined range; focused thumb navigation must pass
3:1 against that same cover.

These are conservative whole-track checks. They may reject an arrangement in
which a represented color never actually borders the thumb. This profile does
not guess spatial exclusions or discard colors to make contrast pass. A future
profile using narrower spatial evidence must prove that evidence explicitly.
Canvas coverage, filtering, quantization and antialiasing remain producer and
renderer obligations; this operation does not certify pixels it has not seen.

Published part contrast reports must not exceed the independently verified
common lower bound. A more conservative original report is permitted. The
reference permits only a 1e-12 arithmetic comparison tolerance for report
agreement; threshold decisions themselves have no such tolerance. Rechecking
never changes the selected color, phase, body, focus geometry or original report.
An insufficient or overstated report fails before snapshot publication.

Label foreground/background must be opaque and clear the explicit label
threshold. All authored thresholds must be finite and in [1,21]; producers must
choose the applicable accessibility requirement rather than infer a permissive
one from missing content. The label box includes all measured text and padding.
Text shaping, actual scaled font metrics, localization and the correctness of
arbitrary supplied value text remain producer obligations. ReadOnly remains
independent of disabled and enabled controls remain focusable.

## Placement and target

Place the track silhouette at its allocated origin. Translate the thumb
silhouette through both numeric endpoint allocations and its current allocation.
The conservative box covering both endpoints covers the complete linear travel
of this fixed current shape. Apply the same operation to the outer thumb focus
geometry, including its local offset. Cover bounds must round outward and retain
both endpoints under exact containment; overflow, lost positive extents or an
unrepresentable placement fails. The reference uses adjacent representable
values for the cover endpoints and extents, then verifies containment.

Revalidate the reserved target against current environment minimums, clipping
and neighboring occupied regions. Its bounds MUST remain unchanged. It must
cover track, current and swept thumb, current and swept navigation, and the
whole label layout box. The label must not overlap any of those paint covers,
including a thumb position between endpoints. Exact endpoint comparisons retain
addition residuals for containment and overlap. This check does not recenter a
target around a moving thumb or shrink paint to fit an undersized target.

The owner reserves enough space for all authored response and motion states.
This snapshot verifies the current resolved shape across its full travel; it
does not claim coverage for a future deformation or different response depth.
Every subsequent pose must retain and revalidate the same reservation.

## Publication and evidence

Only complete success yields a typed snapshot containing the current presentation,
layout, unchanged part paint, label placement/colors, verified contrast bounds,
target, intrinsic Slider semantics and actual reducedMotion preference. No source
protocol, renderer dependency, native tree or default control skin is added.

Tests compose all persistent families, LTR/RTL, both axes, focus, enabled and
readOnly states. Actual pointer acquisition and preview drive layout, dragging
paint and visible-value semantics without product commit. Negative evidence
checks stale states/value/direction/part slots, target changes, geometry sizes,
background coverage, contrast reports, label overlap and intrinsic focusability.
Fixture text advances are arithmetic test inputs, not native font evidence.

This is a composed portable snapshot, not component-complete certification.
Native pointer/keyboard/assistive delivery, real typography, calibrated motion,
renderer pixels and high-end visual review remain mandatory for a released
Slider. All capability tiers use the same coherence and target laws.
