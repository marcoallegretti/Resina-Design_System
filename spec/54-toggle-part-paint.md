# Opaque binary Toggle part paint (candidate, 0.1.0)

Blueprint sections 59–62, 85, 98 and 177 require checked selection to coexist
with interaction, availability and navigation before native shortcuts. This
operation resolves one persistent track or thumb's static paint. It uses the
existing portable surface geometry, material binding, contrast and fallback laws.
It does not publish a complete assembled Toggle.

## Input and selection ownership

The [request](../schemas/toggle-part-paint-request.schema.json) requires version
0.1.0, explicit part (`track` or `thumb`), complete surface paint input,
checkedColorRole and interactionAppearance. The latter uses the existing
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
adjacent-edge guards. Preserve every input signal in the body IR. An off/on role
may resolve to equal colors; complete component state must still be discernible
through the thumb's distinct endpoints, not through color alone.

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

The [result](../schemas/toggle-part-paint-ir.schema.json) records version, part,
checked, phase, response and complete body/navigation paint. Its body color role
records the actual selected role. Structural schema checks require checked to
match the body state set, phase precedence and track-only focus ownership.
Ordinary command paint continues to reject checked; this operation does not
remove selection to pass that guard.

Use coherent current state, theme and environment for both parts. Apply
[part layout](53-toggle-layout.md) externally; emitted geometry is part-local.
The owner must supply each part's actual adjacent color/backdrop, verify thumb
contrast against the painted track, reserve the complete painted footprint and
track navigation bounds in a stable target, and bind complete labels and
[binary semantics](52-toggle-accessibility.md) to one control identity. These
requirements cannot be proven by isolated part paint. Labels, assembly, motion,
event routing and native accessibility delivery remain component obligations.

## Evidence and protocol

[Public cases](../conformance/ir/toggle-part-paint-cases.json) use an explicit
[arithmetic fixture](../conformance/ir/toggle-part-paint-request.json), covering
both parts, selection and independent state channels. Rust tests cover all 63
nonempty supported state combinations across three families and both parts;
actual-capability Frost tests distinguish opaque fallback from unresolved
translucency. GUIdo tests verify real GPU pixels at fractional scales and
track-owned navigation; isolated diagnostic captures are not complete UI evidence.

`resina-toggle-part-paint <path|->` reads strict UTF-8 JSON up to 1 MiB. Success
exits 0 with complete JSON and no diagnostic; invalid input exits 1 with a
diagnostic and no output; usage exits 2. The
[external checker](../tools/check_toggle_part_paint_backend.py) independently
checks selected-role arithmetic, full states, contrast, depth, navigation and
repeated output. `resina-toggle-part-raster` consumes this request and writes
actual CPU RGBA8 PNG output with the existing bounded viewport protocol.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
