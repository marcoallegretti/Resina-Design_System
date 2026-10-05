# Intrinsic Slider accessibility (candidate, 0.1.0)

Blueprint sections 85 and 98 require all eight accessibility dimensions before
native mapping. This contract covers a single bounded Slider with a complete
intrinsic label and optional intrinsic description. It depends on
[bounded values](58-slider-value.md) and [value adjustment](59-slider-adjustment.md).

## Inputs and ownership

The reference takes a checked complete CommandLabelIr and checked SliderValueIr,
optional description and valueText, explicit enabled/readOnly/actual focused and
focusable booleans, and explicit horizontal/vertical orientation. Inputs MUST
belong to one control and one coherent committed snapshot. The owner binds a
stable identity. The shared label object supplies portable complete text and
layout; it does not change the control's semantic role.

Localization precedes resolution. Preserve complete label, description and
valueText exactly; null means absent, while supplied blank text fails. ValueText
is the owner's localized human-readable representation of the current value,
including units or category names when numeric speech alone is insufficient.
It MUST correspond to the committed numeric value and be updated with it.
Resolution does not invent locale, units, formatting, bounds or orientation.

Enabled controls MUST be focusable, including read-only controls. Actual focus
requires focusability. Disabled controls MAY remain focusable and actually focused
for discovery. Focusability and read-only permission are independent.

## Output

The [snapshot schema](../schemas/slider-accessibility-ir.schema.json) requires:

| Dimension | Intrinsic Slider semantics |
| --- | --- |
| role | slider, a portable semantic role |
| name | Complete resolved label, stable when value changes |
| description | Complete supplied description or null |
| value | Complete validated numeric minimum, maximum and current value |
| state | Explicit enabled, readOnly and actual focused |
| actions | setValue, increase and decrease, in that order |
| focusability | Explicit focusable boolean |
| relationships | Empty for this intrinsic-label/description profile |

ValueText and orientation are also explicit fields. The value object retains its
0.1.0 version; normalized render progress is not part of semantic value. All three
actions are available exactly when enabled and writable. Available means permission
to attempt a valid adjustment, independently of actual focus. Endpoint invocations
may be accepted without changing the value, following spec59. It does not certify
an arbitrary amount or guarantee change. Numeric increase/decrease are independent
of physical RTL or vertical placement; orientation describes the control's axis,
not reversed numeric direction. Blank-text and relational numeric invariants need
checked resolution; schema shape validation alone is not a semantic certificate.

Mixed/multiple-thumb ranges, external label/description relationships and extra
states/actions are unsupported by this profile. Owners MUST NOT discard required
relationships to fit it. Hover, press, capture, material paint and native objects
do not enter the snapshot.

## Delivery

Publish coherent updates after label, description, value, valueText, permission,
actual focus, focusability or orientation changes. A semantic action delivers its
explicit value or positive amount through spec59 using current value and current
permission, rather than trusting a previously available action. The owner supplies
increment/decrement amounts and validates any discrete grid; no default amount or
step is inferred. Endpoint keyboard intents can use absolute endpoint adjustments.

Commit the complete next bounded value and its updated valueText before publishing
semantics or notifying product code once when changed. External value/bound changes
require a freshly validated value and coherent localized text. A stale semantic
snapshot is neither permission to mutate nor a stable gesture target certificate.

Native mappings must preserve all dimensions and avoid duplicate interactive label
children. Report unsupported tree/action delivery explicitly. Native text
normalization follows the existing [semantic delivery rules](47-command-accessibility.md).
Headless resolution proves neither native assistive-technology delivery nor a
complete keyboard/pointer/touch lifecycle or rendered Slider.

## Evidence and standards

[Public cases](../conformance/accessibility/slider-cases.json) cover both axes,
permission/focus combinations, endpoint permissions, disabled discovery, complete
localized text with explicit label direction, absent text and invalid focus/text
policy. Schema mutations reject missing dimensions, wrong role/axis, unsupported
relationships/actions and inconsistent permission. Sequence tests commit numeric
adjustment with updated valueText and check stale action delivery after disabling
or becoming read-only. Label measurement callbacks in these semantic tests supply
headless fixture advances, not font-engine evidence.

The [W3C Slider pattern](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) and
[WAI-ARIA 1.2 slider role](https://www.w3.org/TR/wai-aria-1.2/#slider) inform naming,
numeric bounds/value, value text, orientation and read-only state. These are Web
standards. The explicit snapshot, three numeric actions and focus policy are
portable Resina contracts, not Web attributes or native constants. No standalone
serialized request/command or native backend is added by this typed operation.
