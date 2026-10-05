# Slider pointer anchoring (candidate, 0.1.0)

Blueprint sections 98, 99 and 177 require Slider interaction contracts. The
[position operation](62-slider-position.md) accepts a desired thumb origin, not
a raw pointer coordinate. Two concrete owners need this conversion: dragging a
thumb from its actual grab point, and jumping along a track with the thumb center
at the pointer. This contract supplies that geometric dependency before a complete
gesture can compose [cancellable edits](63-slider-edit.md).

## Ownership

Create an immutable anchor from the initial checked Slider layout. A grab anchor
also requires an explicit finite physical point, including its cross-axis
coordinate. A center anchor uses the initial thumb's main-axis center instead.
No hit test, pointer identity, device kind, input permission, capture, focus or
product value change is inferred. The owner must first choose the intended target
using its stable painted-footprint hit region; expanded hit targets can yield a
grab point outside the visible thumb. Those offsets remain valid and are not
clamped to the thumb's center or edge.

The anchor retains initial thumb origin and reference pointer coordinate, main
thumb extent, orientation, layout direction and minimum placement. Keep that
same anchor across every preview and the final release. Recreating it from each
preview would turn previous movement into a new grab and drift the mapping.

Each mapping supplies current checked visual layout and current finite physical
point in the same coordinate frame and units as the original anchor. The owner
must cancel if that coordinate frame changes and cannot be converted consistently.
Axis, layout direction, minimum placement and main thumb extent must match the
initial anchor exactly; incompatible layout fails. A controller must abort that
edit, rather than silently reinterpreting a held gesture. Value, track length,
insets and cross-axis geometry may change without altering the held main-axis
grab distance. External committed value changes still belong to spec63's conflict
rule; a geometric anchor grants no permission to overwrite them.

## Mapping

Use the orientation to select the main coordinate; cross-axis movement alone
does not change the desired origin. At the reference main coordinate return the
exact initial thumb origin. Otherwise, desired origin is initial thumb origin
plus current pointer coordinate minus reference pointer coordinate. Evaluate the
pointer displacement before adding the origin. A grab at any valid offset is
stationary on press, including an expanded-target grab outside the visible thumb.
For track jumps the reference coordinate is the initial thumb center, so the
result preserves its center-to-origin distance.

This operation does not clamp to endpoints. Feed its finite desired origin through
spec62 to apply current bounds, direction, numeric representability and live
permission, or through spec63's preview action before release. Mapping a release
point is not itself proof that the release is eligible to commit.

The Rust reference uses binary64 physical coordinates. Center construction uses
half the main extent and its sum with the initial origin. A nonrepresentable
half, nonfinite center or center indistinguishable from origin fails. The reference
coordinate is the represented center; no arbitrary epsilon or exact rational
coordinate is substituted. Displacement overflow, nonfinite desired origin or
movement that rounds back to the original origin (or reverses direction) fails
diagnostically. Both coordinates are validated before compatibility checks.
Errors return no partial origin and never mutate an anchor. Far finite origins
that remain representable are passed to spec62's endpoint saturation.

## Evidence and remaining component duties

[Qt Quick Slider source, v6.10.0](https://github.com/qt/qtdeclarative/blob/v6.10.0/src/quicktemplates/qquickslider.cpp)
uses half the handle extent in positionAt and retains a press point. This is
evidence for distinguishing pointer position from thumb origin, not a normative
dependency or a claim that Qt uses this grab-preserving policy. Resina explicitly
distinguishes thumb grabs from track jumps and performs no toolkit detection.

All capability tiers use the same coordinate conversion. A termination-only
pointer path can use the same held anchor at release when continuous preview
delivery is unavailable; a keyboard or semantic adjustment path uses explicit
value intents without a pointer anchor. A complete owner still requires reliable
termination, identity, target selection, capture or equivalent routing, touch
arbitration, cancellation, release eligibility and localized accessible feedback.
No complete gesture, native delivery or styled Slider is certified here.

The Rust reference exposes only checked constructors and a desired-origin method;
anchors serialize as part of complete pointer lifecycle state for conformance.
There is no production deserialization or source request protocol for anchors.
[Public cases](../conformance/interaction/slider-anchor-cases.json) cover both axes,
directions, minimum placements, expanded-target offsets, preview continuity,
track resizing, incompatible geometry and far finite coordinates. Typed tests
check nonfinite points, arithmetic loss and composition with the value mapper.
The [case schema](../schemas/slider-anchor-cases.schema.json) defines conformance
records, not an independently authoritative numeric result or gesture history.
