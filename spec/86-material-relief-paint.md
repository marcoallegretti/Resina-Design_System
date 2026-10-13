# Material-lit relief paint (candidate, 0.1.0)

Compose the [bounded material light](85-bounded-material-light.md) with the
existing [relief face and guards](83-opaque-relief-paint.md). This operation
paints a local opaque face. It does not select component anatomy, acquire
observations, animate, project a side plane, replace focus or disabled-state
identification, or introduce translucent composition. Existing key-light,
material-light and relief-paint protocol versions and behavior remain unchanged.

## One light authority

The request contains one complete light document, complete environment and
actual enabled/interaction state. There is no second key-light field. Resolve
the light policy once, validating original source coefficients even when their
channels are suppressed. Evaluate the same signed bounded law over the full
analytic relief normal at every interior point. Do not normalize its planar
projection or mirror physical light under RTL.

Ambient and interaction channels modify the face's existing body/highlight/side
mixture only. The body pigment is retained exactly. A flat normal returns it
exactly for every admitted channel and policy. Absent optional channels reproduce
the existing static relief modulation. Recessed normals invert the directional
response without changing the physical light.

## Preserved guards

Retain the existing exact exterior-edge classification, edge candidate contrast,
positive representable inset, conservative uniform content rectangle and opaque
foreground contrast threshold. Lighting cannot modify the exterior edge pigment
or its adjacent-background requirement. Outside points yield null. The complete
reserved content rectangle remains the exact body color, including its boundary.
Reject collapsed or uncertifiable content and precision failures diagnostically.

Publish three outward source-color ranges in this order: body/highlight,
body/side, then the exact edge color. The first two use the existing four-epsilon
and adjacent-value enclosure; the edge range has identical lower and upper
colors. These bounds do not certify filtering, actual spatial adjacency,
contrast of content outside the reserved rectangle, or focus containment.
Focus and independent interaction/disabled identification retain their contracts.

## Public boundary

The [request](../schemas/material-relief-paint-request.schema.json) contains
version 0.1.0, outline, relief profile, pigment, light, environment, enabled and
interactionActive, complete edge request, edge width, foreground, minimum content
contrast and ordered physical points. All nested source contracts remain strict.
Validate every supplied candidate and point, including unused or outside data,
before publishing a result. Reject duplicate decoded names, unknown/missing or
positional members, nonfinite values and unsupported versions. Preserve original
nested source through light, pigment and outline admission.

The [result](../schemas/material-relief-paint-result.schema.json) contains
version 0.1.0, one paint and ordered point/color samples. The paint retains
surface, pigment, resolved light (`keyDirection`, `ambient`, `interaction`), edge,
edge width, foreground, content contrast, content bounds and color ranges.
Empty points are valid. A late failure rejects the whole request without partial
samples. `resina-material-relief-paint <path|->` uses the existing 1 MiB UTF-8
boundary and status 0/1/2 convention. Derived numbers retain the applicable
geometry/light tolerance; input body, exact flat colors, classification, structure
and versions remain exact.

Proof includes absent-channel compatibility with authored relief vectors,
independent rectangular normal/modulation arithmetic, active and suppressed
channels, rotations, RTL, invalid original source and unchanged edge/content
guards. The public checker is required in addition to reference tests. Raster,
GPU, complete native content/focus and independently evaluated fractional-scale
scene evidence remain separate requirements before complete appearance is accepted.
