# Ordinary command accessibility (candidate, schema 0.1.0)

Blueprint §§85 and 98 require component accessibility semantics before native widget mapping. This contract resolves the semantics of an ordinary command with an intrinsic complete text label and optional intrinsic description. It supplies the eight accessibility dimensions required by §85. It is part of the command contract; native accessibility tree delivery, focus recovery and a complete Button remain separate conformance requirements.

## Inputs and ownership

The resolver takes a validated [complete label](46-command-label-layout.md), validated current [activation state](42-command-activation.md), explicit optional description and explicit focusability. Inputs MUST belong to the same command and be obtained coherently. The owner binds this snapshot to its stable control identity; do not derive identity from the accessible name or transfer another control's activation state. The name MUST equal the complete label text exactly, including Unicode, whitespace, punctuation and explicit line breaks. Do not use a shortened draw string, translate it again, trim it, substitute a tooltip or invent a separate label. The producer owns localization of both label and description before resolution. A supplied description MUST be nonblank; null means no description. Preserve supplied description text exactly.

Focusability means the control can receive native keyboard focus; it is distinct from actual focused state and eligibility within a particular [tab sequence](41-focus-traversal.md). Enabled ordinary commands MUST be focusable. Actual focused state requires focusability. Disabled commands MAY remain focusable, including actually focused, for discoverability. The component owner must choose the disabled policy explicitly and retain coherent native state during updates. The resolver does not clear focus or infer it from a queued request.

## Output

The [portable snapshot](../schemas/command-accessibility-ir.schema.json) requires schema version 0.1.0 and:

| Dimension | Ordinary command semantics |
| --- | --- |
| role | `button`, the semantic role of an action command, independent of native role constants |
| name | Complete resolved text label |
| description | Supplied complete description or null |
| value | null: an ordinary command has no editable, numeric, checked or toggle value |
| state | Explicit `enabled` and actual `focused`, taken from current activation state |
| actions | Exactly one supported `invoke` action with `available` equal to `enabled` |
| focusability | Explicit `focusable` Boolean |
| relationships | Empty: this intrinsic-label/intrinsic-description profile has no external semantic node relationships |

The momentary press hold is interaction feedback, not a toggle value or accessibility pressed state. Do not expose it as checked, selected or toggle-pressed. Disabled snapshots retain the supported action while making its unavailability explicit. Hover, paint, spring values, hit-region geometry, native objects and backend names do not belong in this snapshot.

External label/description nodes, controlling relationships, menu buttons, toggles, icon-only controls and links require their own complete contracts. A producer MUST NOT use this intrinsic ordinary command profile to discard relationships required by such content. Empty relationships and null value here are explicit semantics, not defaults for unsupported component kinds. Native accessibility names/descriptions may require platform-owned nodes; that mapping does not change the portable content.

## Delivery and conformance

Publish coherent snapshots when label, description, availability, actual focus or focusability changes. Use current activation state when handling an accessibility `invoke`: the snapshot is not authorization to replay an action after it becomes unavailable. Route invocation through the same activation lifecycle, commit its next state before delivering the product side effect and execute each intent once. Invoke supersedes a held raw gesture; its later release must not activate again. Keep the ordinary command semantic node singular; a backend must not expose duplicate interactive children for its intrinsic label.

Native backends must map role, complete name, description, enabled/focused/focusable and action availability to their actual accessibility API, preserve semantics across reduced rendering capabilities, and deliver invocation through the lifecycle. The snapshot preserves source text rather than running a platform accessible-name algorithm. A native platform may apply its specified whitespace normalization when computing the exposed name/description; adapters MUST retain every label word and verify that mapping against the actual platform rules. The [W3C accessible-name 1.1 terminology](https://www.w3.org/TR/accname-1.1/#terminology), for example, defines a flat string with normalized whitespace. Do not impose source-byte identity on an API that requires that normalization. Unsupported native accessibility support must be reported explicitly in backend conformance; a painted control or headless snapshot alone does not prove assistive technology delivery. Screen-reader, keyboard and platform tree evidence remain required for a complete component.

The Rust reference accepts validated label and activation objects and produces a read-only snapshot. It has no renderer, font engine or platform dependency. The [public cases](../conformance/accessibility/command-cases.json) cover focused/described commands, disabled focus discovery/exclusion, full localized labels, held keyboard/pointer gestures and inconsistent focus policy. Integration tests connect semantic invocation to live availability and prevent a later raw release from activating again. The semantics tests use supplied layout advances only to obtain validated label objects; they do not claim native font measurement. Structural schema checks reject missing dimensions, unsupported value/relationship/action/state fields and action availability inconsistent with enabled state; reference validation also rejects blank descriptions.

These choices are informed by the [W3C button pattern](https://www.w3.org/WAI/ARIA/apg/patterns/button/), [label-in-name guidance](https://www.w3.org/WAI/WCAG22/Understanding/label-in-name.html) and [disabled-control focus guidance](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/#focusability-of-disabled-controls). Those sources describe Web accessibility practices; the snapshot and its exact-name, stable supported-action and explicit focusability policies are Resina decisions. Native mappings are not Web attributes in the normative layer.
