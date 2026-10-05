# Slider value presentation (candidate, 0.1.0)

Blueprint sections 85 and 98 require coherent visible and accessible Slider
values. This contract composes [edit sessions](63-slider-edit.md),
[value policy](67-slider-value-policy.md), [keyboard adjustment](68-slider-keyboard.md)
and [intrinsic semantics](60-slider-accessibility.md).

## Numeric presentation

Supply the complete checked committed value, a nonempty opaque revision, complete
value policy and optional checked edit session. An open session MUST match the
committed value, revision and policy. Its checked preview becomes visible; without
an edit, visible equals committed. Both values MUST satisfy the current domain.
An inconsistent session fails explicitly rather than rebasing or hiding it.

The owner resolves layout, localized value text and accessibility from the visible
value. Preview semantics describe the value currently presented without committing
it to product state or issuing a product change notification. Cancellation restores
the current committed value; conflict restores the replacement committed value.
Localization remains the owner's responsibility: this operation cannot verify that
arbitrary text describes a numeric value.

## Adopting an adjustment

Keyboard and semantic adjustments use the current visible value. Record the actual
value and revision used to compute the candidate. At delivery, resolve a fresh
presentation and compare those stamps before checking permission. A stale value or
revision fails, including change-and-return transitions with a new revision.
Candidate bounds MUST match the committed bounds and its target MUST satisfy the
complete current value policy, even when delivery is unavailable.

Recheck explicit enabled, readOnly and sourceAvailable state. For keyboard delivery,
sourceAvailable includes actual focus and ownership of the selected binding. For a
semantic action it means the routed action is currently owned and available; it
does not invent a keyboard focus requirement. Revision alone cannot certify focus
or routing permission. An originally rejected candidate cannot become accepted
merely because permission subsequently returns.

An accepted candidate is adopted against the committed value. The resulting
changed flag compares its target to that baseline, independently of the candidate's
changed flag. An endpoint no-op against a preview may therefore commit a product
change. Returning a preview to the committed baseline is accepted without a product
change. Rejected adoption returns unchanged committed state as an adjustment result;
that result is not a visible frame and MUST NOT replace an ongoing preview blindly.

Accepted delivery MUST close the old edit and explicitly abort pointer ownership,
including accepted product no-ops. Commit the complete value, update revision when
value or meaning changes, and rebuild layout, localized text and semantics before
notifying product code once when changed. Permission loss uses existing edit/pointer
cleanup; rejecting an unrelated unfocused key does not itself specify pointer
cancellation. Resolve every delivery from live state; do not replay cached results.
The pure reference executes neither callbacks nor native ownership effects.

## Representation and evidence

The [presentation schema](../schemas/slider-presentation.schema.json) requires
complete committed/visible values, policy, revision and editing state. Schema shape
cannot prove session identity, domain membership, history or publication order.
[Public vectors](../conformance/interaction/slider-presentation-cases.json) link to
existing continuous and stopped edit traces through a
[test-record schema](../schemas/slider-presentation-cases.schema.json).
Typed tests check session coherence, preview promotion, product no-op adoption,
permission loss, stale stamps and invalid stopped targets. Accessibility integration
checks preview, cancel, commit and external replacement with corresponding value
text. Text measurement there uses fixture advances, not native font evidence.

[WAI-ARIA 1.2](https://www.w3.org/TR/wai-aria-1.2/#aria-valuenow) identifies the
current range value and corresponding value text. That standard informs semantic
coherence; the portable edit/adoption contract and cancellation rules are Resina
requirements rather than Web attributes. This numeric presentation is not a full
styled snapshot or a native controller. Rendering, localization, routing and native
assistive delivery remain required in every capability tier.
