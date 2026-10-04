# Binary toggle activation (candidate, 0.1.0)

Blueprint §§59–61, 98–99 and 177 require persistent selection to coexist with
interaction and focus. This contract supplies the binary on/off lifecycle for
Toggle before renderer-specific components. It composes the existing
[activation lifecycle](42-command-activation.md) with an explicit current checked
value. It does not define a complete Toggle's anatomy, paint, motion or native
accessibility delivery.

## Inputs and ownership

The request requires version `0.1.0`, validated activation `state`, boolean
`checked` and one activation `event`. The checked value is the current committed
on/off value: true is on, false is off. There is no mixed value. Momentary
pressed feedback is independent and MUST NOT replace checked state.

The component owner supplies current activation and checked state together.
An external property update supplies its new checked value on the next request;
the resolver does not remember or restore the value from pointer/key down. Use
live availability when delivering semantic invocation and gestures, rather than
an old paint or accessibility snapshot. Gesture routing, hit membership, native
repeat metadata and capture obligations remain those of the activation contract.

## Transition and result

1. Resolve the complete activation transition against current state and event.
2. If its `activate` intent is true, invert the supplied checked value once.
   Otherwise retain it, including during disabling, blur and cancellation.
3. Publish version, the complete `activation` result and next boolean `checked`
   together. Any invalid input publishes no result.

The nested activation result owns next gesture state, pressed feedback and
capture effects. Its `activate` identifies an accepted user toggle, not a second
command to dispatch after applying checked state. Commit both next state and
checked value before notifying product code. Notify once for that accepted
toggle; deterministic replay for conformance does not authorize replaying a
product side effect. There is no independent cached selection state.

Pointer down only arms; inside release toggles, outside release cancels. Space
toggles on release. This contract also supports Enter, toggling on the initial
press, with repeated deliveries suppressed. Semantic invoke toggles without
requiring focus and supersedes an armed gesture. Availability and focus changes
retain the committed checked value. A disabled focused toggle can remain on and
discoverable without accepting another toggle.

## Component integration

Binary Toggle uses on/off semantics. A native accessibility mapping must expose
its boolean checked value independently of actual focus and availability. Its
intrinsic label MUST stay stable when checked changes; expose the state as state,
not by replacing the label with the opposite action. Checkbox mixed state,
Radio group selection and toggle-button pressed semantics are separate contracts.
The operation does not publish an accessibility tree or certify its delivery.

Paint composition must retain checked selection alongside hover, pressed,
disabled and focus signals. Ordinary command paint is not a checked-state
appearance contract. Authored selection appearance, control geometry, labels,
text scaling, localization and lower-capability rendering remain component
requirements; this operation supplies no placeholder native control.

The binary semantics and stable label follow the
[W3C switch pattern](https://www.w3.org/WAI/ARIA/apg/patterns/switch/) and
[WAI-ARIA 1.2 switch role](https://www.w3.org/TR/wai-aria-1.2/#switch).
Space is required and Enter is optional in that pattern; supporting Enter and
reusing the press/release timing are explicit Resina choices. These web sources
inform portable semantics, not toolkit-specific IR or full native conformance.

## Reference and conformance

Rust supplies `resolve_toggle_activation` and `resolve_toggle_activation_source`.
The [request](../schemas/toggle-activation-request.schema.json),
[result](../schemas/toggle-activation-result.schema.json) and
[public cases](../conformance/interaction/toggle-activation-cases.json) reuse the
versioned activation event/state/result contracts. The result schema validates
its complete nested activation and boolean output; the relation to the input's
checked value is a transition law checked by conformance.

`resina-toggle-activation <path|->` reads strict UTF-8 JSON up to 1 MiB. Success
exits 0 with complete JSON; invalid input exits 1 with a diagnostic and no output;
usage errors exit 2. The external checker tests the same protocol without
importing the Rust implementation. Cases cover on/off inversion, momentary
holds, current release membership, repeats, cancellation, disabled discovery,
semantic invocation and invalid input. Sequence tests cover external checked
updates during a gesture and invocation superseding a held key.
