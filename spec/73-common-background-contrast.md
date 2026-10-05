# Common edge and focus contrast (candidate, 0.1.0)

Slider edges and navigation can border several regions, including directional
track highlights and canvas. This contract extends existing edge and focus color
selection to a nonempty set of checked [opaque ranges](72-bounded-background-contrast.md).
It selects one coherent color before paint assembly. It does not infer adjacency
or certify the supplied ranges cover the actual rendered background.

## Selection

For each candidate, calculate the conservative contrast bound against every
supplied range. Its complete-background bound is the minimum of those bounds.
Only a candidate that meets the applicable threshold across every range passes.
Prefer the original semantic color when it passes; otherwise try the existing
outline.strong fallback. If both fail, report both complete-background bounds.
Never select different candidates per region to certify one common edge or ring.
Region order and repetitions do not change the selection or bound.

Edge selection retains outline, then outline.strong, with an explicit finite
minimumContrast in [1,21]. Validate threshold, outline opacity and strong-outline
opacity before checking nonempty regions. Even an unused authored candidate must
be valid. Empty adjacent ranges fail explicitly.

Focus selection binds the actual complete surface intent through the existing
resolved theme/environment context, requires focused, and requires nonempty
surrounding ranges. It retains focus, then outline.strong, the existing strict
3:1 threshold, stroke width and gap. No state is removed or focus inferred; body
interaction and navigation remain independent. An empty set cannot certify focus.

Threshold comparisons MUST be strict and unrounded. Preserve all current source
protocols and single-color diagnostics. A single uniform range gives the complete
existing single-color result. The selected edge/focus result's contrastRatio is
the worst conservative bound across the supplied range set; it is not a claim of
contrast against one sampled pixel or an exact spatial minimum.

## Ownership and evidence

The producer supplies all actual adjacent/surrounding regions, with current
post-treatment colors and proved coverage. Disconnected flat regions use separate
uniform ranges; shaded regions need valid bounds. A caller must not drop a failing
region, repair its bounds, substitute guessed surroundings or change capabilities.
All tiers use the same color-selection law. Insufficient contrast publishes no
partial selection, and paint assembly must subsequently remain atomic.

The Rust reference exposes typed range-set edge and focus operations. Existing
single-color source operations retain their wire shapes and behavior; no unchecked
range parser or new native protocol is introduced. A small shared private reducer
serves these two concrete consumers without owning thresholds, bindings or roles.

[Public cases](../conformance/color/common-contrast-cases.json), with a
[strict test schema](../schemas/common-contrast-cases.schema.json), replay preferred
and fallback selection, incompatible per-region candidates, interior crossings,
separate flat regions and explicit empty-set failures through both operations.
Tests verify complete binding states, preserved focus geometry constants, failure
bounds and exact uniform-result compatibility. Final Slider geometry, actual
range provenance, complete paint, native exposure and rendered quality remain
required; successful color selection alone does not certify them.
