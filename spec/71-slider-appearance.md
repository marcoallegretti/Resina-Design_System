# Slider appearance profiles (candidate, 0.1.0)

Blueprint sections 19, 59–67 and 98 require material-specific interaction feedback
and independent navigation. This contract selects static response coefficients
for the Slider's separate track and thumb from its complete
[interaction projection](70-slider-states.md). It owns neither gesture lifecycle
nor value adoption.

## Authored profiles

The [appearance schema](../schemas/slider-appearance.schema.json) requires track
and thumb profiles. Each part requires Cast, Frost and Elastomer, each with explicit
hover, pressed, dragging, disabled, readOnly and readOnlyHover responses. Every
coefficient MUST be validated, including unselected parts, families and phases.
No profile is inferred from another part, family or phase. Rest is the defined
identity response: bodyMix zero and depthScale one.

The document, each part's family collection and each family's phase collection
MUST be objects with their required named members. Positional arrays MUST NOT be
interpreted as these records. Unknown, missing and duplicate decoded member names
fail; object member order is immaterial. Each response follows the same object
requirement in the command appearance contract.

Responses use the checked bodyMix and depthScale domains and mathematics of
[command appearance](43-command-paint.md#authored-responses). Sharing those
coefficient laws does not share command state precedence. Depth compression is
not in-plane elastic deformation. These responses MUST NOT modify numeric value,
part allocation, hit regions, semantic roles or state observations.

Gel MUST NOT be selected as a persistent track or thumb material in this profile,
including at rest. The blueprint permits Gel for transient drag feedback;
such an overlay requires its own representation and fallback contract.

## Body phase

Input is one checked state set and an explicit readOnly flag. Supported states are
rest, hover, focused, pressed, disabled and dragging. Other states fail explicitly.
The input MUST obey the complete projection from spec70:

- Disabled or read-only controls cannot retain pressed or dragging.
- Dragging requires pressed, including a newly acquired pointer edit that has
  not yet moved or changed value.
- Rest is present exactly when disabled, hover and pressed are absent.

Incoherent input fails rather than being repaired. Body phase is disabled,
otherwise dragging, otherwise pressed, otherwise readOnlyHover when read-only
and hovered, otherwise readOnly when read-only, otherwise hover when hovered,
otherwise rest. Disabled read-only controls use disabled. Enabled read-only
controls MUST NOT be painted as disabled by this selection.

Focused is independent and does not replace the body phase. The complete state
set remains unchanged and MUST remain available for navigation paint and semantic
resolution. Hover can coexist with disabled and focus, as in spec70.

## Composition and evidence

Selected coefficients are an input to subsequent paint resolution. Paint MUST
apply them before final body/content/adjacent-edge contrast checks and resolve
focus against the actual response geometry and surroundings. Frost retains actual
capabilities and transparency preferences; selecting a phase does not override
the environment. Every tier uses the same phase law and requires valid material
fallbacks. Static endpoints remain available when motion is reduced.

The [public appearance fixture](../conformance/appearance/slider-appearance.json)
uses distinct arithmetic coefficients to detect incorrect part/family/phase
selection. It is not a production theme or visual calibration. The
[record vectors](../conformance/appearance/slider-appearance-vectors.json) pair a
complete named document with complete positional substitutions at every document,
part and family boundary. Their negative outcomes require rejection without
depending on implementation-specific diagnostic wording. The
[phase cases](../conformance/interaction/slider-phase-cases.json) cover every
nonempty combination of supported signals for writable and read-only controls,
plus unsupported-state diagnostics. Their
[record schema](../schemas/slider-phase-cases.schema.json) distinguishes successful
phases from errors. Rust provides checked authored profiles and pure phase
selection; it is a reference implementation of this contract.

Complete Slider paint, mixed track/canvas contrast, motion, native delivery,
localization, accessibility exposure and rendered visual review remain required.
