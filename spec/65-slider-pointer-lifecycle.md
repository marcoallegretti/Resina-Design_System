# Slider pointer lifecycle (candidate, 0.1.0)

Blueprint sections 85, 98, 99 and 177 require portable, operable Slider interaction.
This contract composes checked [hit regions](40-hit-region-ir.md),
[allocation](61-slider-layout.md), [anchors](64-slider-pointer-anchor.md) and
[cancellable edits](63-slider-edit.md). It defines pointer ownership, previews,
completion and cancellation. It does not execute product code or native protocols.

## Inputs and ownership

Each delivery supplies immutable current controller state, checked current
committed value, explicit [value policy](67-slider-value-policy.md), a nonempty
current revision, checked current visible layout,
current checked control hit region, live enabled/readOnly flags, explicit routing
capability and one event. Layout contains the visible preview while held and the
committed value when starting from idle. Revision follows spec63 and also changes
when control identity, target meaning or coordinate frame is replaced. Do not
reuse a revision after an intervening change. If the old coordinate frame cannot
be preserved consistently, abort explicitly.

The control region covers the complete allocation and reserves the actual painted
footprints and full child targets. Revalidate it against current clipping,
neighbors and environment using spec40. Down supplies a separately checked region
for its explicitly selected thumb or track target. That region must cover the
allocated part and fit within the control region. These checks are necessary
geometry constraints; the producer must also derive targets from actual painted
footprints and choose among overlapping targets within the same control. The
controller never infers target selection from a toolkit hit box.

Every pointer-bearing event has a nonempty opaque **press identity**, compared
exactly. It identifies a particular held gesture, not just a reusable device or
native pointer number. Do not reuse it while any earlier delivery, acquisition
acknowledgment or effect may still arrive. The producer retains its native pointer
binding outside normative state and correlates all deliveries with that press.
This prevents delayed acquisition or termination from taking a newer gesture.

Map primary pointer gestures to down/move/up, and preserve their termination even
if modifiers change. Do not feed a synthesized click as another gesture after
the same pointer sequence; one physical activation must produce at most one
committed intent. Coordinates use the same physical logical-pixel frame as
layout and hit regions, with placement/device conversion performed once by the
producer. Both coordinates must be finite, including the unused cross axis.
Empty revision/identity and nonfinite points fail before ownership or availability
guards. Down target coverage is validated before availability/ownership guards.
No default device, primary button, coordinate, focus or routing capability exists.

## Routing capabilities

| Capability | Required guarantee |
| --- | --- |
| continuous | The owner can request capture or equivalent continued routing, prove acquisition and report failure/loss. |
| terminationOnly | Down's owner can reliably receive final release or cancellation, including interruption; continuous moves are not required. |
| unavailable | Reliable termination cannot be provided; do not arm a pointer edit. |

These are current delivery guarantees, independent of renderer tier or backend
name. A request is not acquisition proof. Continuous down arms a pending hold and
emits an acquire request with its exact press identity. Only a matching actual
routingAcquired delivery moves pending to acquired. RoutingLost covers both failed
acquisition and subsequent loss. Native capture or equivalent owner routing must
be established and verified by the producer; returning an armed result alone does
not install either.

Termination-only down retains its declared mode with no acquire request. A later
capability upgrade does not change that held mode. Losing reliable termination
cancels it. Pending/acquired continuous ownership is cancelled on loss or downgrade
of its routing capability; restart through an available fallback instead of
silently changing a held gesture's meaning.

## State and transitions

Idle has explicit null hold. A hold retains exact press identity, target, phase
(pending/acquired/terminationOnly), complete control and original target regions,
complete initial anchor and complete numeric edit session. This is transient
interaction state, separate from paint IR and native routing handles.

Down inside the selected region, with no hold, begins a cancellable edit. Thumb
down anchors its actual grab point and preserves its exact current value. Track
down uses the thumb-center anchor and previews the clicked position. Enabled and
writable down arms a hold only when reliable routing is available. Both paths
produce no commit on down. Invalid applicable mapping still fails when starting
unavailable; a well-formed unavailable start exposes current committed state.
Down outside its target and competing presses do not start another edit.

| Event | Held gesture rule |
| --- | --- |
| routingAcquired(id) | Matching pending hold becomes acquired; no value change. Duplicate, foreign or unsolicited acknowledgments are ignored. |
| move(id, point) | Matching acquired hold previews through its original anchor and spec63. Pending and termination-only moves are ignored. |
| up(id, point) | Matching acquired hold maps the final point, then commits once under live permission. Matching termination-only hold does the same only inside its original target rectangle. Pending release cancels without committing. |
| cancel(id), routingLost(id) | Matching hold closes, discards preview and clears routing without committing. Foreign identities do not take ownership. |
| abort | Close any hold without committing, including removal, window deactivation, Escape, interrupted delivery or touch arbitration loss. Idle abort is ignored. |
| refresh | Reconcile current committed state, permission, delivery and geometry when they change without a pointer event. It never commits. |

Captured movement and release may leave the control without losing the gesture;
no path-following constraint is imposed. The producer must offer an actual abort
path. For a termination-only gesture, moving release outside the original target
aborts it. Track clicks require no dragging and provide a single-pointer
alternative to thumb dragging, independently of keyboard/semantic adjustment.
Touch scrolling/arbitration belongs to the producer: do not claim acquisition
before establishing the actual owner, and report loss if another owner takes it.

After basic event validation, explicit matching cancellation/abort closes first,
even when external state or visual layout became stale. Otherwise changed revision
or committed value/bounds closes with conflict; lost live permission closes with
unavailable; lost delivery closes with deliveryUnavailable. None overwrite product
state. Matching-baseline layout must contain the held edit's complete preview or
resolution fails diagnostically. A changed control region closes with targetChanged.
Allocation outside its reserved region or incompatible anchor geometry closes with
layoutChanged. These checks run on every held delivery, including foreign events
and refresh, before ordinary event transitions. No cancelled ownership resurrects
when permission or capability returns. Orphan termination is ignored.

Closed noncommitting results expose the **currently committed** value, not the old
baseline. Pending/acquired closure emits release for the exact owned press; it
means cancel pending acquisition and clear current ownership, not blindly call a
native API after the platform has already released it. Termination-only closure
has no routing effect. Producers make cleanup idempotent and never apply an old
press's effect to a newer native binding.

## Results, diagnostics and commit

Results require version, complete next state, complete visible preview, explicit
commit (accepted spec59 adjustment or null), explicit routing effect
(acquire/release with identity or null), and outcome: ignored, armed,
routingAcquired, previewed, committed, cancelled, unavailable, conflict,
deliveryUnavailable, layoutChanged or targetChanged. Committed is the only outcome
with an accepted commit intent and always has null hold. Armed exposes pending or
termination-only phase explicitly; it never pretends pending routing is acquired.

Commit next controller state before executing one product intent and notifying
once when changed. Accepted no-op is distinct from a changed value. Pure replay is
not permission to execute the effect twice. Rebuild visible layout, localized value
text and accessibility from preview during a hold and from current committed state
after cancellation. Actual focus remains independent and is never invented here.

Mapping/coherence diagnostics return no partial result and leave the supplied
immutable state unchanged. The owner must deliver abort before dropping an edit
that cannot continue reliably; clearing its state privately would lose routing
cleanup. Abort does not require coherent stale visual geometry. Invalid current
value or revision must first be repaired by its owner; no fallback number or
identity is guessed.

## Standards and conformance

[Pointer Events Level 3](https://www.w3.org/TR/2026/REC-pointerevents3-20260630/#process-pending-pointer-capture)
distinguishes pending from acquired capture and releases capture after termination.
[WCAG pointer cancellation guidance](https://www.w3.org/WAI/WCAG22/Understanding/pointer-cancellation.html)
supports release completion with an abort path.
[WCAG dragging guidance](https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html)
uses track clicks as a Slider's single-pointer alternative and distinguishes that
requirement from keyboard accessibility. These inform the portable contract;
they do not turn it into a browser-specific protocol or certify native delivery.

[Public traces](../conformance/interaction/slider-pointer-cases.json) check complete
serialized states, held anchors/regions/edit sessions, previews, commit intents,
routing effects and outcomes. Shared named layout and region fixtures are explicit:
each step replaces the layout's value with its required visualValue and selects
its required layout/control names; each region request receives the case's required
environment. No fixture default is a production input policy. Typed tests cover
nonfinite coordinates and numeric failures followed by actual resolver abort.

[State](../schemas/slider-pointer-state.schema.json),
[result](../schemas/slider-pointer-result.schema.json) and
[trace](../schemas/slider-pointer-cases.schema.json) schemas require complete fields
and local phase/outcome constraints. Runtime conformance still owns cross-field
identity/numeric equality, revision history and routing chronology. The Rust
reference provides checked idle state and pure typed resolution, serialization for
complete conformance, and no production deserialization/source request API.

All tiers use this lifecycle. Complete controls still require keyboard policy,
step/grid admissibility, localized semantics, actual native routing/assistive
delivery and styled material/fallback rendering. Native pixel and protocol evidence
must be supplied by each backend; these headless traces certify neither.
