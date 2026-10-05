# Slider position mapping (candidate, 0.1.0)

Blueprint sections 98, 99 and 177 require an operable Slider. This contract maps
an explicit desired thumb origin through the current [allocation](61-slider-layout.md)
to the current [bounded value](58-slider-value.md), then applies the existing
[live adjustment permission](59-slider-adjustment.md).

## Inputs and ownership

Supply a checked complete current SliderLayoutIr, a finite desiredOrigin in the
allocation's logical-px main-axis coordinates, and live enabled/readOnly booleans.
The layout owns both the current value and numeric endpoint placement. Rebuild it
from current bounds, value, scale, insets and direction before delivery; a stale
layout cannot certify current state. Horizontal uses physical x, vertical y.
No direction is inferred from a backend, language or pointer device.

DesiredOrigin is the requested thumb rectangle origin, not the pointer location.
A drag owner subtracts its explicit grab offset after converting native
coordinates to allocation space. A track-jump owner explicitly chooses its thumb
anchor. Neither offset, jump policy nor native coordinate conversion is guessed.
The same mapping serves these two concrete needs.

This operation requires continuous linear bounded-value progress. A discrete or
transformed-value owner needs its own allowed-value policy; no grid, snapping,
logarithmic scale or step default is inferred.

## Mapping

A desired origin equal to the current resolved origin preserves the exact
current value. Rounded layout positions are not invertible in general; a
stationary thumb must not drift numerically or trigger a change notification.

A finite origin at or beyond a numeric endpoint selects that exact bound. This
is an explicit bounded movement policy, separate from rejection of invalid
authored current values. It works for either numeric direction and both axes.
Interior origins normalize distance to the two endpoint origins. The reference
uses the nearer endpoint to retain small distances, then interpolates current
numeric bounds. When the numeric span overflows binary64, weighted endpoint
terms avoid using the overflowing span.

Interior mapping MUST remain strict interior in both normalized progress and
numeric value. A requested movement that cannot change the numeric value in the
requested direction fails. Resolve the candidate through spec58 and project it
through spec61; its thumb origin MUST move in the requested physical direction.
Arithmetic loss that leaves a stationary or reversed thumb fails. No arbitrary
epsilon, fabricated endpoint, hidden rounding grid or partial result is used.
Finite input and representability checks apply even when permission is denied.

The result is the complete [SliderAdjustmentIr](../schemas/slider-adjustment-ir.schema.json):
current or next value, accepted and changed. Disabled/readOnly delivery is
unchanged and unaccepted for a valid mapping. Exact stationary or saturated
endpoint requests can be accepted without changing the value. Owners commit
the complete value and freshly resolved geometry/semantics before notifying
once when changed. This pure operation performs no callback.

## Gesture boundary

Target reservation must cover actual painted geometry and remain stable during
movement, as in spec61. Pointer identity, capture acquisition/failure/loss,
cancellation, focus, independent hover, native touch/scroll arbitration and
permission changes belong to a complete gesture lifecycle. Coordinate mapping
alone does not authorize an update or establish capture. Reduced-motion and
lower-capability rendering consume the same committed value and allocation.

[Pointer Events Level 3](https://www.w3.org/TR/pointerevents3/#implicit-release-of-pointer-capture)
requires capture release after pointerup/pointercancel. This informs the native
adapter lifecycle boundary; no DOM or native capture object enters this contract.

## Evidence and reference

[Public cases](../conformance/interaction/slider-position-cases.json) cover both
axes, LTR/RTL, both minimum positions, interior movement,
endpoint saturation, no-ops, live permission, extreme numeric spans and
unrepresentable mapping. Exact rational expectations operate on represented
fixture numbers. Tests verify complete results and current-layout rebuilding,
monotonic direction and scale identities. Fixtures are not calibrated dimensions.
No rendered Slider, native gesture or assistive-technology delivery is certified.

The Rust reference exposes resolve_slider_position over typed checked input.
The [case schema](../schemas/slider-position-cases.schema.json) describes test
fixtures, not a source/command protocol. The existing
result schema remains authoritative; no duplicate output IR is introduced.
