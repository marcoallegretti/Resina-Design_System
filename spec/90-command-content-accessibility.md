# Command content accessibility (candidate, schema 0.1.0)

A Command shows its content in one of three forms: a text label, an icon with a text label, or an icon alone. This contract defines the content document that states the form, and the accessible semantics of the two forms that show an icon. An icon-only Command has no visible text, so its name cannot come from layout. Its producer supplies a complete localized accessible label, and the name is never taken from the icon.

## Content forms

A [command content document](../schemas/command-content.schema.json) has `schemaVersion` `0.1.0`, a `form` and exactly the members of that form:

| Form | Members | Meaning |
| --- | --- | --- |
| `label` | `label` | A visible text label, laid out by the [label layout](46-command-label-layout.md) contract |
| `iconAndLabel` | `icon`, `label` | A decorative icon beside a visible text label, laid out by the same contract |
| `iconOnly` | `icon`, `accessibleLabel` | An icon with no visible text, named by a label that is neither drawn nor laid out |

`icon` is a valid [icon content](87-icon-content.md) document, symbolic or colour. The form is a content choice, orthogonal to the emphasis variant of the [component anatomy](78-component-anatomy.md): every form can be `standard` or `primary`. The form is never inferred from the variant, the icon, the text or the available space.

`label` and `accessibleLabel` are complete localized text. The producer owns localization before resolution. Both MUST be nonblank: text that is empty or contains only Unicode `White_Space` characters is blank. This is the same rule that spec/46 applies to label text and spec/47 applies to descriptions. Format characters such as U+200B ZERO WIDTH SPACE, U+2060 WORD JOINER and U+FEFF ZERO WIDTH NO-BREAK SPACE are not `White_Space`, so a label made only of them is nonblank and accepted. Such text names nothing a person can hear or read, and supplying it does not meet the obligation to provide a complete label. Changing the blankness rule would change all three contracts together. Text is otherwise preserved exactly, including whitespace, punctuation and explicit line breaks.

The following are errors:
- missing, unknown or duplicate members, or an unsupported version;
- an unknown form, or a member that belongs to another form, such as `label` on an `iconOnly` document;
- a blank `label` or `accessibleLabel`;
- an invalid icon document.

A present `null` member is an error, not an absent member. Duplicate JSON members MUST be rejected before conversion to an object tree. An invalid document is not published in part. In particular, a missing, null or blank `accessibleLabel` is a diagnostic. A resolver MUST NOT replace it with the icon reference, a segment of the reference, a directional variant, a name supplied by an icon mapping, a tooltip or any default text.

## Ownership

The [ordinary command accessibility](47-command-accessibility.md) contract alone resolves the `label` form, from its successfully laid-out label. This contract resolves `iconAndLabel` and `iconOnly`. Given a `label` document, its resolver reports an error and publishes nothing; a producer resolves that Command through spec/47.

## Accessible semantics

The resolver takes a validated content document, the laid-out label when the form shows one, validated current [activation state](42-command-activation.md), an explicit optional description and explicit focusability. Description and focusability follow spec/47: a supplied description is nonblank, enabled commands are focusable, actual focus requires focusability and a disabled command MAY remain focusable for discoverability. The output is the [command accessibility snapshot](../schemas/command-accessibility-ir.schema.json) at schema version 0.1.0, with every dimension defined as in spec/47 except the source of the name:

| Form | Laid-out label | Name |
| --- | --- | --- |
| `iconAndLabel` | Required | The laid-out label text, which equals `label` exactly |
| `iconOnly` | None | `accessibleLabel` exactly |

An `iconAndLabel` Command, like a spec/47 Command, has a snapshot only after its label has been laid out successfully by spec/46. If layout fails or has not run, there is no laid-out label and no snapshot is published. The following are also errors, reported before the spec/47 description and focusability errors:
- a `label` document;
- an `iconAndLabel` document without a laid-out label;
- an `iconAndLabel` document whose laid-out label text differs from `label`, because the two inputs do not belong to the same content;
- an `iconOnly` document with a laid-out label, because an accessible label is never laid out.

The name of an `iconAndLabel` Command therefore equals the name spec/47 resolves from the same laid-out label. The resolver never compares the supplied text with the icon. Text that happens to equal the icon's name segment or reference is accepted as supplied text, and it is not derived from the icon.

The icon contributes nothing to the snapshot: not to the role, name, description, value, state, actions or relationships. A Command in either form is one accessible node. In `iconAndLabel` the icon is decorative and hidden from assistive technology. In `iconOnly` the icon is the visible presentation of the same node that the accessible label names. The icon is not a separate image node, and the accessible label is not a separate hidden text node. Relationships stay empty, because the accessible label is intrinsic content of the Command and not an external labelling node. A Command labelled by another visible node needs the external relationship contract that spec/47 excludes.

## Delivery and conformance

Publication, invocation and native mapping follow spec/47. Publish a coherent snapshot when the content, the laid-out label, the description, availability, actual focus or focusability changes. A native backend maps the name to the platform accessible name of the Command node and exposes no accessible object for the icon. A tooltip is presentation: it neither supplies nor replaces the name.

This contract does not define icon size roles, icon placement or the layout of an icon beside a label, icon contrast in any state, icon mapping lookup or resource failure, rendering, the binding of content into the Command snapshot, or the IconButton component. Those need their own contracts and evidence. A headless snapshot does not prove assistive technology delivery; screen-reader and platform tree evidence remain required for a complete component.

The [content vectors](../conformance/content/command-content-vectors.json) list valid and rejected documents, including a label made only of zero-width format characters. A case with `schemaValid` `true` and an `error` is valid against the schema but violates a rule the schema cannot express, such as a label made of Unicode spaces. The [duplicate-member document](../conformance/content/invalid-duplicate-command-content-member.json) repeats `accessibleLabel` and is rejected. The [accessibility cases](../conformance/accessibility/command-content-cases.json) give the text of the laid-out label as `laidOutLabel`, or null when there is none. They resolve both icon forms, including disabled focus discovery and exclusion, held gestures, localized, whitespace-preserving and zero-width labels, and labels equal to the icon's name segment or reference, and they reject a `label` document and each laid-out label error. The schema checker derives the outcome of every case from its inputs by these rules and compares every snapshot dimension. The Rust reference parses each document from text and from a value, lays out each label with supplied measurements before resolving it, checks that `iconAndLabel` resolves the same snapshot as spec/47 for every spec/47 case and that the `label` form is rejected, and checks that a label whose layout fails yields no snapshot.

These choices follow [WCAG 2.2 non-text content](https://www.w3.org/WAI/WCAG22/Understanding/non-text-content.html), which requires a text alternative for a control that is only an image and lets decorative images be ignored by assistive technology, and [name, role, value](https://www.w3.org/WAI/WCAG22/Understanding/name-role-value.html). The content forms, the required accessible label and the single-node rule are Resina decisions.
