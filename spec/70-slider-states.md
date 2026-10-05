# Slider interaction projection (candidate, 0.1.0)

Blueprint sections 59–67, 85 and 98 require composed interaction feedback,
independent navigation and multiple input capabilities. This contract projects
[value presentation](69-slider-presentation.md) and the current
[pointer lifecycle](65-slider-pointer-lifecycle.md) into the existing
[state set](14-interaction.md), before Slider appearance resolution.

## Inputs and coherence

Supply checked presentation and pointer state, explicit enabled, readOnly, actual
focused, hovered and keyPressed booleans. The presentation MUST be exactly the one
formed from current committed value/revision/policy and the pointer's actual edit.
Stale session identity or inconsistent visible/editing state fails before permission
checks. This profile uses pointer-owned previews; an unrelated edit session is not
an interchangeable gesture owner.

Hover is a current independent observation, including from another pointer. It is
not inferred from capture, focus, value changes or a pressed key. A producer without
hover supplies false explicitly. Focus is actual observed focus, not a queued request.
Enabled and readOnly are independent. Enabled read-only controls remain focusable;
readOnly MUST NOT become disabled.

KeyPressed describes a currently held, owned and supported Slider adjustment
binding, independently of whether the last accepted adjustment changed the numeric
value. Arbitrary or unsupported keys do not supply this signal. Key-up, focus loss,
permission loss, removal and interrupted delivery clear keyboard ownership through
the owner. Semantic actions do not invent a physical key press. No key identity,
repeat timing, platform code or native focus transfer enters this projection.

A pointer hold or key press while disabled or read-only fails. A key press without
actual focus fails. A pointer hold can coexist with a key that remains physically
pressed after a previous adjustment; a new accepted keyboard adjustment interrupts
the pointer as specified in spec68/69. Do not confuse a held-key observation with
delivery of a new keyboard intent.
Errors do not clear ownership or fabricate a fallback state. The owner performs
the existing cancellation/refresh transition and resolves a new coherent snapshot.

## Projection

1. Include disabled exactly when enabled is false.
2. Include hover exactly when hovered is true, including read-only or disabled discovery.
3. Include pressed while a valid pointer hold exists or keyPressed is true. All
   pending, acquired and termination-only holds give contact feedback.
4. Include dragging exactly during an acquired edit with continuous pointer delivery.
   Here dragging denotes owned pointer tracking that previews values until
   termination; it does not assert a distance threshold, prior physical movement or
   a numeric change. Both thumb and track gestures can enter this mode. Pending
   acquisition and termination-only clicks MUST NOT claim continuous manipulation.
   This applies to continuous and stopped value policies alike.
5. If disabled, hover and pressed are all absent, include rest as the body base.
6. Include focused independently exactly when focused is true, including disabled
   and read-only discovery.

Return one complete nonempty versioned state set in canonical order. Preserve all
simultaneous signals and their existing composition layers. ReadOnly does not add
a canonical state; retain it in the accessible/control permission snapshot. No
selected, checked, active, busy or validation state is inferred from a numeric value.
This operation does not activate a command, commit a value or choose a winning
material response. Ordinary command/Toggle paint does not support this complete
Slider state profile; dropping dragging to pass their guards is invalid.

## Reference and evidence

Rust exposes a pure typed resolver using existing checked state and presentation
objects; no unchecked state parser or new production source protocol is introduced.
[Public cases](../conformance/interaction/slider-states-cases.json) define complete
idle signal combinations and link pending/acquired/release-only cases to actual
pointer traces through a [test-record schema](../schemas/slider-states-cases.schema.json).
Runtime replay checks both pointer targets, independent focus/hover, stale permission,
simultaneous input, complete signals and presentation coherence. The existing generic
state-set schema remains the resolved representation.

[WCAG dragging guidance](https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html)
distinguishes continuous pointer manipulation from a click/tap alternative, and
[Pointer Events](https://www.w3.org/TR/pointerevents3/#process-pending-pointer-capture)
distinguishes pending from acquired routing. These inform the portable law; neither
standard defines this exact state projection or certifies native acquisition.
All tiers use the same explicit observations without backend detection. Complete
Slider paint, motion, native keyboard/pointer/assistive delivery and visual
certification remain required.
