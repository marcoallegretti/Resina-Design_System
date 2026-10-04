# Binary Toggle accessibility (candidate, 0.1.0)

Blueprint sections 85 and 98 require all eight accessibility dimensions before
native mapping. This contract covers a binary on/off Toggle with intrinsic
complete label and optional intrinsic description.

## Inputs and ownership

The Rust reference takes a validated complete `CommandLabelIr`, validated current
activation, explicit boolean checked, optional description and explicit
focusability. The shared label object contains portable complete text and layout;
it does not change this control's semantic role. All inputs MUST belong to one
control and one coherent committed snapshot. The owner binds a stable identity.
Localization precedes resolution; preserve complete label and description text
exactly. Null description means absent; supplied blank descriptions fail.

Enabled controls MUST be focusable; actual focus requires focusability. Disabled
controls MAY remain focusable and actually focused for discovery. These policies
match the existing ordinary-command contract, independently of checked state.

## Output

The [snapshot schema](../schemas/toggle-accessibility-ir.schema.json) requires:

| Dimension | Binary Toggle semantics |
| --- | --- |
| role | `switch`, a portable semantic role |
| name | Complete resolved label, stable when checked changes |
| description | Complete supplied description or null |
| value | null; binary checked state is carried in state |
| state | Explicit enabled, actual focused and boolean checked |
| actions | One supported invoke action, available exactly when enabled |
| focusability | Explicit focusable boolean |
| relationships | Empty for this intrinsic-label/description profile |

Checked true means on; false means off. Mixed state, toggle-button pressed,
checkbox/radio semantics and editable values are unsupported. Momentary press,
hover, capture, native objects and paint do not enter this snapshot. A group or
external label/description relationship requires its own complete contract;
this profile MUST NOT discard such relationships.

## Delivery

Publish coherent updates after label, description, checked, availability, actual
focus or focusability changes. Checked changes MUST NOT substitute an opposite
action label. Invocation uses live activation and checked values through
[binary Toggle activation](50-toggle-activation.md), committing both next values
before notifying product code once. A stale available action is not permission
to invoke after disabling. Semantic invocation supersedes a held raw gesture;
its later release does not toggle again.

Native mappings must preserve all dimensions, avoid duplicate interactive label
children, and report unsupported tree/action delivery explicitly. Platform name
normalization follows the existing [semantic delivery rules](47-command-accessibility.md).
Headless snapshots do not prove native assistive technology delivery, focus
recovery or a complete rendered Toggle.

## Evidence

[Public cases](../conformance/accessibility/toggle-cases.json) cover both checked
values across localized complete labels, descriptions, holds, disabled discovery
and invalid focus policy. Sequence tests check stable naming through semantic
invocation, superseded release and disabling. Schema tests require every dimension
and reject nonbinary checked, role substitution and inconsistent availability.
Supplied layout advances in semantic tests are headless fixtures, not font-engine
measurement evidence.

The [W3C switch pattern](https://www.w3.org/WAI/ARIA/apg/patterns/switch/) and
[WAI-ARIA 1.2 switch](https://www.w3.org/TR/wai-aria-1.2/#switch) inform binary state
and stable labeling. Those are Web standards; this portable snapshot, null value,
explicit focus policy and invoke action are Resina contracts, not native constants
or Web attributes.
