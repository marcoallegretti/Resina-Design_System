# Cancellable Slider edits (candidate, 0.1.0)

Blueprint sections 85, 98 and 177 require operable, accessible Slider contracts.
This contract separates visible numeric preview from a committed value so a
pointer owner can abort an edit before completion. It composes [bounded values](58-slider-value.md),
[adjustment permission](59-slider-adjustment.md) and [position mapping](62-slider-position.md).

## Session ownership

Begin explicitly with a checked committed value, [value policy](67-slider-value-policy.md)
and a nonempty opaque revision
string. The owner gives the revision an exact identity within the control's
lifetime, changing it whenever committed value, bounds or their meaning change.
Never reuse a previous revision after an intervening change. Comparison is exact,
without trimming or normalization. A revision must also distinguish changes that
leave the current number equal to the original number. No timestamp or sequence
format is inferred.

The immutable session retains that revision, the complete baseline value and the
complete preview, initially equal to the baseline, and the complete value policy.
Each delivery supplies the current committed value/revision and policy,
live enabled/readOnly permission and one action:
preview with checked current visual layout and finite desired thumb origin;
commit; or cancel. Visual layout must contain the session's current preview,
including bounds and normalized progress, before a preview can be updated.
The committed value and visible preview are distinct inputs while an edit is open.

## Resolution

Validate nonempty current revision and finite preview position before guards.
Cancel closes the session and restores the currently committed value as visible
preview, with no commit intent. It does not restore the old baseline into a
product that may have changed externally.

For preview or commit, a changed revision or a committed value different from
the baseline, or a changed policy, produces conflict: close the session, expose the current committed
value and emit no commit. This check precedes layout coherence because a visual
layout from the abandoned edit is expected to be stale after an external change.
Conflict is explicit; it never silently rebases or overwrites a concurrent edit.
An owner that fails to change revision is still caught when value/bounds differ;
the reference cannot detect an unreported change-and-return history.

With matching current baseline/revision, preview requires a coherent visual
layout and maps its desired origin through spec62 under live permission. Stopped
policies select an allowed value through spec67 before publishing preview. Mapping
failures remain diagnostic even when unavailable. An accepted preview returns a
new open session and complete visible value, without committing or notifying
product code. Rebuild layout, localized value text and semantics from that visible
value before the next frame; keyboard/semantic mutation of committed state must
update revision and abandon the old edit.

Commit applies the preview's absolute value to the current baseline through
spec59 under live permission, or exact allowed selection through spec66 for stopped
policies. Accepted commit closes the session and returns its
complete adjustment intent, including accepted no-op versus changed. Unavailable
preview or commit closes the session, exposes the current committed value and
emits no commit. Disabled/readOnly state never publishes a product change.

Results require version, explicit session (open object or null), complete preview,
explicit commit (complete adjustment or null) and outcome: previewed, committed,
cancelled, unavailable or conflict. Previewed is the only open outcome; committed
is the only outcome with an accepted commit intent. Errors produce no result and
do not mutate the supplied immutable session. The owner must explicitly cancel
an edit when a diagnostic prevents reliable continued delivery.

Commit next controller state and close the session before executing one product
intent. Notify once only when the intent changed. Replaying a pure resolution
for conformance is not permission to execute the intent again. These functions
provide no platform transaction, callback or product undo stack.

## Gesture and fallback boundary

Drag and track-jump owners use this transaction to preview before pointer release
and discard preview on cancellation. Capture requests are not acquisition proof;
identity, acquisition/failure/loss, grab anchor, current coordinate conversion,
stable painted-footprint targets, touch arbitration and release eligibility still
belong to a complete gesture contract. Neither preview nor revision establishes
capture or focus. An owner must offer an actual abort path and close an edit on
interruption, capture loss, removal or permission loss. No commit on pointerdown
is inferred.

[WCAG 2.2 pointer cancellation guidance](https://www.w3.org/WAI/WCAG22/Understanding/pointer-cancellation.html)
supports completion on release with abort/undo for complex pointer interactions.
[Pointer Events Level 3](https://www.w3.org/TR/pointerevents3/#process-pending-pointer-capture)
distinguishes pending from acquired capture. This numeric transaction implements
neither native protocol and alone does not certify accessibility conformance.

All capability tiers can render the same complete preview through existing
allocation/paint contracts. When reliable continuous pointer delivery is absent,
owners need a cancellable release-based discrete path or keyboard/semantic value
adjustment. No renderer detection, animation requirement or effect default is
introduced. Complete styled controls and native assistive delivery remain required.

The Rust reference exposes an explicit begin constructor and pure typed
resolve_slider_edit. [Public traces](../conformance/interaction/slider-edit-cases.json)
check complete sessions, previews, commit intents and outcomes across both axes,
directions, cancellation, permission changes, bounds replacement, change-and-return
revisions and diagnostics. Tests also cover typed nonfinite positions and exact
opaque revision preservation. No product callback or native input is claimed.

[Session](../schemas/slider-edit-session.schema.json) and
[result](../schemas/slider-edit-result.schema.json) schemas require complete fields
and local outcome invariants. The [trace schema](../schemas/slider-edit-cases.schema.json)
describes test records, not a source/command API. Schema shape checks cannot
establish revision history, layout coherence or that an owner executes an intent once.
