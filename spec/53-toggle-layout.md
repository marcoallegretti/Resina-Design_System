# Horizontal binary Toggle part layout (candidate, 0.1.0)

Blueprint sections 85, 98 and 177 require authored component anatomy, adaptation
and lower-capability rendering. This contract allocates the track and movable
thumb of a horizontal binary Toggle before shape, material and paint resolution.
It is component layout policy, not a generic toolkit widget or complete Toggle.

## Inputs

The [request](../schemas/toggle-layout-request.schema.json) requires version
0.1.0, explicit trackSize and thumbSize in logical px, nonnegative logical
insets (start/end/top/bottom), actual layoutDirection and boolean checked.
Sizes MUST be finite and positive. Dimensions are authored/resolved inputs;
this operation neither invents token defaults nor shrinks an oversized thumb.
Missing, duplicate, unknown and unsupported fields fail.

## Allocation

Track bounds have origin (0,0) and the supplied size. Subtract start then end
from track width, and top then bottom from height, to obtain its interior.
The thumb must fit that interior. Horizontal travel is interior width minus
thumb width and MUST be positive; identical on/off positions are unsupported.
Vertical slack is interior height minus thumb height and may be zero. The thumb
y origin is top plus half that slack.

Off sits at logical start; on sits at logical end. In LTR the off x origin is
start and the on x origin is trackWidth minus end minus thumbWidth. In RTL the
off x origin is trackWidth minus start minus thumbWidth and on x origin is end.
Only logical placement mirrors: vertical allocation and physical lighting do not.

The [result](../schemas/toggle-layout-ir.schema.json) records version,
layout direction, checked, trackBounds, offThumbBounds, onThumbBounds and
thumbBounds. The current thumb is the on endpoint exactly when checked is true.
Both endpoints retain the supplied thumb size; track allocation is independent
of checked. Dimensions and coordinates follow finite binary64 arithmetic.
Subtracting a positive term must not lose that term entirely; positive vertical
slack must produce a distinguishable centered position. Endpoint additions must
be finite and distinguishable from their origins. Overflow, swallowed extents,
coincident endpoints and failed containment produce no result.

## Ownership and subsequent resolution

These are allocation rectangles, not visible silhouettes or hit regions. Resolve
authored shape and material roles for each part through their own contracts;
no capsule, radius, pigment or material family is inferred from Toggle identity.
A label/description keeps its complete localized text and owns separate measured
layout. Text scaling must be handled before selecting the overall component
allocation; this operation does not truncate labels or rescale authored parts.

The owner reserves one stable component target covering the complete actual
painted footprint and both thumb positions using the existing hit-region
contract, including extrusion and focus placement where applicable. The track
rectangle alone is not proof of that painted-footprint coverage. Thumb motion,
press feedback and checked updates must not recenter the reserved target or
change control identity. Thumb animation may later interpolate these endpoints;
reduced motion can use the selected static endpoint. Checked selection remains
independent of hover, press, availability and actual focus.

## Conformance

[Public cases](../conformance/geometry/toggle-layout-cases.json) cover asymmetric
insets, fractional sizes, vertical centering, both directions and checked values,
zero insets, invalid fit, missing fields and arithmetic loss. Integration tests
verify mirror identities, stable track ownership and a reserved target covering
both allocated endpoints. They do not certify painted-footprint coverage.

`resina-toggle-layout <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits
0 with complete JSON and no diagnostic; invalid input exits 1 with no result;
usage errors exit 2. The [external checker](../tools/check_toggle_layout_backend.py)
validates repeated output and duplicate rejection independently of Rust.
Structure/state are exact; geometry comparison uses the repository's numeric
conformance tolerance. Shape, checked appearance, focus paint, motion, native
routing, accessible tree delivery and visual review remain separate requirements.
