# Coherent command snapshot (candidate, 0.1.0)

Blueprint §§84–85 and 106–108 require content, interaction, appearance and
accessible semantics to describe the same command. This operation binds the
existing ordinary command contracts into one checked publication snapshot. It
accepts either static command paint or the current command paint from a sampled
motion result. It does not define a complete Button or native event adapter.

## Inputs and ownership

Supply validated complete label IR, validated command paint IR, current committed
activation state, explicit hover observation, intrinsic optional description,
explicit focusability, environment, available rectangle, component target
minimum and occupied neighboring reservations. All input objects MUST describe
one command in one coherent layout/environment/theme/font context. Actual focus
comes from successful native observation. Supplied paint MUST already have been
resolved using the current projected interaction state.

The producer remains responsible for common theme/environment provenance,
typography scaling, complete font measurement and native pointer coordinate
mapping. The operation can compare physical geometry, declared label direction
and semantic signals; those checks do not prove theme identity, font selection,
native focus or native accessibility delivery. Do not relabel an old paint or
measurement as current merely because its dimensions happen to match.

## Validation and publication

1. Compare the paint body's complete state set to the exact projection of current
   activation and explicit hover defined in [command states](48-command-states.md).
   A matching body phase alone is insufficient: disabled hover and independent
   actual focus must remain coherent. Reject stale or additional signals and
   identify both the expected and supplied state sets diagnostically.
2. Require label layout direction to equal the supplied environment direction.
3. Validate the complete label layout box against the same body's front size and
   uniform content contour using [content containment](46-command-label-layout.md).
4. Resolve the rectangular hit region from that body's full visible silhouette,
   excluding its independent navigation ring, using the [target law](40-hit-region-ir.md).
   Retain all minimum, clipping, overlap and representability diagnostics.
5. Resolve [accessible semantics](47-command-accessibility.md) from this exact
   label and current activation, with the explicit description/focusability.
6. Publish the four channels together only after every check succeeds. On any
   failure, publish no snapshot or newly resolved member. Do not repair stale
   paint, remove focus, change padding, clip the target or weaken semantic policy.

The snapshot is not authorization to execute an action. Native semantic invoke
and raw gesture delivery still use the current activation lifecycle at delivery
time, including when availability changes after snapshot creation. Component
owners must coordinate replacement with their native paint/input/accessibility
publication mechanisms. A headless result does not provide a platform transaction.

## Reference and conformance

Rust provides `resolve_command_snapshot` and a read-only `CommandSnapshot` whose
label and paint borrow the validated inputs and whose hit region/accessibility
are owned resolved members. It does not retain a borrow of the live activation,
description or layout context: the owner can continue receiving state changes,
and action delivery still checks current availability. This reference composition
object introduces no new wire
format; its members retain their existing portable IR schemas. Independent
implementations can implement the same composition law without borrowing Rust
objects or importing a toolkit.

The [public state cases](../conformance/interaction/command-snapshot-cases.json)
independently specify coherent and stale signals, including cases whose body
phase matches while hover or focus does not. Tests connect live lifecycle
transitions to the exact paint projection and
complete label, target and accessibility members; exercise sampled motion;
reject stale hover/focus/availability/held-gesture paint and direction mismatch;
and preserve the causes of content, target and semantic failures. Supplied label
extents are explicit layout arithmetic fixtures, not native font measurements.
Native capture, key repeat metadata, assistive technology delivery, material
calibration and complete component conformance remain separate requirements.
