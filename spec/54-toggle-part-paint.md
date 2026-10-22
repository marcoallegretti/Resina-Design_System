# Opaque binary Toggle part paint (candidate, 0.2.0)

Blueprint sections 59–62, 85, 98 and 177 require checked selection to coexist
with interaction, availability and navigation before native shortcuts. This
operation resolves one persistent track or thumb's static paint. It uses the
existing portable material binding, contrast and fallback laws, and paints the
part as a [lit control body](88-lit-control-body.md): the lit relief face
projected with the key-light side plane and guarded edge. It does not publish a
complete assembled Toggle.

## Input and selection ownership

The [request](../schemas/toggle-part-paint-request.schema.json) requires version
0.2.0, explicit part (`track` or `thumb`), complete surface paint input,
checkedColorRole, interactionAppearance, `light` and `relief`. Version 0.1.0
requests are rejected. interactionAppearance uses the existing
[authored interaction response schema](../schemas/command-appearance.schema.json)
for Cast/Frost/Elastomer hover, pressed and disabled channels. These are shared
body-response coefficients, not an ordinary-command state policy.

The request MUST be a JSON object. `part` and `checkedColorRole` MUST be string names from their respective schemas. Positional requests and object-encoded names are invalid.

The original surface's color role is its unchecked role. When its full state set
contains checked, bind checkedColorRole through the actual theme instead. Both
source and opaque fallback assignments belong to that role. Keep material role,
form, treatment stack and every state unchanged. Color-role choice MUST NOT infer
another material family, override capabilities or manufacture an opaque fallback.
Track and thumb each request a persistent control role; Gel is not a persistent
control body. Selection feedback overlays require a separate contract.

Supported signals are rest, hover, pressed, checked, disabled and focused.
Other signals fail explicitly. Body response precedence is disabled, pressed,
hover, rest; checked does not enter that precedence. Resolve selection first,
apply authored encoded-sRGB body mix and depth scale, then rerun content and
adjacent-edge guards and resolve the lit control body. Every authored hover, pressed and disabled response of the
bound family MUST be
[perceivable from rest](43-command-paint.md#perceivable-state-responses), and the
disabled response perceivable from the pressed response, on the opaque bodies of
both the unchecked role and checkedColorRole, whichever state and selection is
being resolved, so a profile that only one selection perceives fails in both; otherwise
resolution fails with a diagnostic and no partial IR. A track at the `embedded` elevation has zero semantic depth, so its
responses, including disabled against pressed, are perceivable only through body
color. Preserve every input signal
in the body IR. An off/on role may resolve to equal colors; complete component
state must still be discernible through the thumb's distinct endpoints, not
through color alone.

## Light and relief

The body is the [lit control body](88-lit-control-body.md) of the responded body
color at the final depth, with the surface size, the bound Tier 0 shape fallback,
`circular` curvature, the actual layout direction and the family's exterior
`edgeWidth` as minimum inset. The appearance `keyLight` resolves both the side
offset and the face light; the family's `highlightWidth` is not used. The
[bounded relief](82-surface-relief.md) is part-keyed and does not depend on
state, selection, material family or depth scale.

The request carries two required members, as for [command paint](43-command-paint.md#light-and-relief):

- `light`, the unversioned light channels object with exactly `ambient` and
  `interaction`; absence of either channel is explicit null. The original source
  coefficients of every supplied channel are validated under the
  [bounded material-light source rule](85-bounded-material-light.md#inputs-and-fallback),
  including channels that the policy or the phase suppresses.
- `relief`, a complete [control relief](../schemas/control-relief.schema.json)
  document. Every part profile is validated. The track uses `toggleTrack` and the
  thumb uses `toggleThumb`; a rounded corner that cannot carry the shoulder fails
  with the lit-body diagnostic prefixed by the part's profile name.

The interaction channel is admitted exactly when the body phase is `hover` or
`pressed`: `checked`, `focused` alone and `disabled` never admit it, and the
admitted channels follow the bounded material-light environment policy unchanged.
A keyboard press reaches the pressed phase; a product without a pointer direction
supplies null interaction and no directional light appears. Light never changes
the body, edge or foreground and is never a state cue: perceivable responses and
contrast are measured on the unlit body color and the final depth, so suppressing
light never removes a cue. Neither part dims its foreground; it draws no content,
so the recorded foreground is the guarded content-role color in every phase.

## Navigation and fallback

The track owns the single component navigation treatment. Focused track paint
requires surroundingColor and resolves visible focus against its current
response geometry. Thumb paint retains focused in its full state set but emits
no separate ring. A producer MUST NOT omit the track's required navigation paint
or expose either part as another independent interactive control.

Both parts use actual environment and capabilities. Frost must resolve a valid
opaqueDimensional representation for this opaque operation; a still-translucent
result fails. Contrast-driven opaque fallback remains available after selection
and interaction. Never force reduced transparency or replace capability flags to
obtain success. Static endpoints remain usable under reduced motion.

## Output and composition

The [result](../schemas/toggle-part-paint-ir.schema.json), version 0.2.0, records
version, part, checked, phase, response and complete body/navigation paint: a
[lit control body](../schemas/lit-control-body-ir.schema.json) with the spec/39
focus member, which only the track owns. Its body color role records the actual
selected role. Structural schema checks require checked to match the body state
set, phase precedence and track-only focus ownership. Ordinary command paint
continues to reject checked; this operation does not remove selection to pass
that guard. The superseded 0.1.0
[request](../schemas/versions/toggle-part-paint-request-0.1.0.schema.json),
[result](../schemas/versions/toggle-part-paint-ir-0.1.0.schema.json) and
[body](../schemas/versions/toggle-part-body-ir-0.1.0.schema.json) schemas remain
archived. Consumers tell this body apart from the spec/36 body by these envelopes.

Use coherent current state, theme and environment for both parts. Apply
[part layout](53-toggle-layout.md) externally; emitted geometry is part-local.
The owner must supply each part's actual adjacent color/backdrop, verify thumb
contrast against the painted track and the thumb's containment in the track's
uniform face, reserve the complete painted footprint and
track navigation bounds in a stable target, and bind complete labels and
[binary semantics](52-toggle-accessibility.md) to one control identity. These
requirements cannot be proven by isolated part paint. Labels, assembly, motion,
event routing and native accessibility delivery remain component obligations.

## Evidence and protocol

[Public cases](../conformance/ir/toggle-part-paint-cases.json) use an explicit
[arithmetic fixture](../conformance/ir/toggle-part-paint-request.json), covering
both parts, selection and independent state channels. They also cover every phase
of both parts with light admitted and suppressed, including a keyboard press
without interaction and checked parts, every environment condition of the
policy, explicit and absent channels, capsule, square and asymmetric bodies in
both layout directions, and invalid light, relief and shoulder widths. Rust tests
cover all 63 nonempty supported state combinations across three families and both
parts, with and without light, and the [published results](../conformance/ir/toggle-part-paint-ir.json);
actual-capability Frost tests distinguish opaque fallback from unresolved
translucency. GUIdo tests verify real GPU pixels at fractional scales and
track-owned navigation; isolated diagnostic captures are not complete UI evidence.

`resina-toggle-part-paint <path|->` reads strict UTF-8 JSON up to 1 MiB. Success
exits 0 with complete JSON and no diagnostic; invalid input exits 1 with a
diagnostic and no output; usage exits 2. The
[external checker](../tools/check_toggle_part_paint_backend.py) independently
checks selected-role arithmetic, full states, contrast, depth, navigation and
repeated output. It derives the expected light, pigment, edge, content rectangle
and lit-body invariants with [independent arithmetic](../tools/lit_body_arithmetic.py),
and repeats every checked state set of every family and part in an admitting
environment with and without light, requiring identical output apart from
`face.light`. `resina-toggle-part-raster` consumes this request and writes actual
CPU RGBA8 PNG output with the existing bounded viewport protocol; the
[lit body raster checker](../tools/check_lit_body_raster_backend.py) with
`--component toggle` compares every pixel with the independent arithmetic.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
