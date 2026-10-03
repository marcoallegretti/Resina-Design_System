# Surface body and navigation IR (candidate, 0.1.0)

Blueprint §§60–62 require state channels to compose without one replacing the
others. The [surface paint request](../schemas/surface-paint-request.schema.json)
provides `schemaVersion` `0.1.0`, one complete [opaque body request](36-opaque-surface-ir.md)
as `body`, and an optional actual opaque sRGB `surroundingColor` behind the
navigation ring. `surroundingColor` MUST be supplied when the surface states
include `focused`. A supplied color MUST be valid and opaque even when unfocused;
null is invalid. Unknown members, duplicate JSON members, unsupported versions
and invalid nested inputs MUST fail diagnostically.

Compile the body's theme once and resolve one semantic snapshot against its
supplied environment. Resolve the body with its existing readability, shape,
depth, pigment, band and optical guards. Only `rest`, `focused`, or both are
currently supported; every other body state MUST fail. Preserve all supplied
states in canonical order, without inferring `rest`.

When focused, resolve [focus indicator IR](37-focus-indicator-ir.md) against the
same semantic snapshot, surface intent, size, shape assignments, depth assignments
and physical key light. Use the supplied surrounding color for the existing
focus contrast guard. A body or focus failure MUST publish no result, including
when the other channel could succeed. An unfocused result MUST omit `focus`;
it MUST NOT include a null or inactive placeholder indicator.

The [result](../schemas/surface-paint-ir.schema.json) contains `schemaVersion`
`0.1.0`, complete `body` IR and, exactly when focused, complete `focus` IR.
Their state sets, material and color roles, material family, surface form and
swept silhouette MUST agree. Their geometry uses one shared physical coordinate
origin and logical `px` scale. Cross-channel equality is a semantic invariant;
the schemas validate structure, while conformance also checks this invariant.

A consumer MUST retain both channels as one resolution result and publish their
replacement together. Body paint follows the opaque body contract, and navigation
paint follows the exterior ring contract. The ring's transparent hole and gap
MUST preserve the body paint. Reserve space for the complete outer focus contour,
including negative origins, so its navigation cue is not clipped. This IR is
computed design intent; it is not a widget tree, toolkit image, event system,
accessibility tree or a complete component contract.

`resina-surface-paint <path|->` accepts one UTF-8 request of at most 1 MiB.
Success exits 0 with one complete JSON result and no diagnostic; resolution or
input failure exits 1 with a diagnostic and no result; usage errors exit 2.
The [external checker](../tools/check_surface_paint_backend.py) verifies the
command protocol, deterministic output, public body/ring expectations and
cross-channel invariants. Existing material scenes and component-independent
fallbacks remain authoritative. This contract does not add unsupported state
appearance, optical effects, motion, input behavior or accessibility semantics.
