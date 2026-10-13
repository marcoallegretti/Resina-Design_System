# Bounded material light (candidate, 0.1.0)

The [design decision](https://github.com/marcoallegretti/Resina-Design_System/issues/129)
adds optional ambient and interaction response to the existing
[opaque pigment](29-opaque-pigment.md) and physical [key light](34-key-light.md).
Appearance resolution owns the response. Products supply observations and actual
interaction state; backends realize the resolved values. This operation does not
acquire wallpaper, rebind a theme role, change body pigment, select geometry,
simulate energy or introduce a physical reflectance model. Environmental pigment
adaptation is a separate contract.

## Inputs and fallback

The versioned light document requires `keyLight`, `ambient` and `interaction`.
Absence of either optional channel is explicit JSON null. Ambient is a finite
proportion in [0,1). Interaction is an object containing a physical nonzero
planar `direction` and a finite `strength` in [0,1). Directions use the key-light
normalization law and do not mirror with logical layout direction. Validate all
supplied channels, including channels suppressed by policy. Genuine decimal zero
is valid; a negative nonzero source coefficient is invalid even when binary64
decoding would erase it to negative zero.

The operation also requires a complete environment snapshot, explicit `enabled`
and `interactionActive` booleans, an opaque pigment request and ordered full
unit-normal objects `{x,y,z}`. Normals must be finite with each component in
[-1,1] and squared length within 1e-12 of one. Do not normalize their planar
projection: its magnitude carries the face tilt. Empty and repeated sample
arrays are valid and preserve order. This normal input is not a geometry or
containment certificate.

High contrast suppresses both optional channels and retains the static key
baseline. Interaction also resolves to zero when the surface is disabled, the
interaction is inactive, dynamic lighting is unavailable, quality is economy,
or reduced motion is requested. Balanced and full admit the same authored
strength; they do not silently scale it. Static ambient attenuation requires no
dynamic drawing primitive. Reduced transparency does not change this opaque
operation. Null ambient resolves to zero; null interaction resolves to no
direction and zero strength. Suppressed interaction retains no effective
direction. Suppression is a published resolution result, not invalid-input
recovery.

The caller MUST clear `interactionActive` after the interaction ends. Temporal
sampling and input acquisition retain their existing owners. This operation
neither manufactures an interaction nor specifies a new animation timeline.
Lighting MUST NOT be the sole hover, press, selection, focus or disabled
indication. Existing state, accessibility, exterior edge and focus contracts
remain necessary at component integration.

## Signed response and bounded colors

Let d be the normalized key direction, e the admitted interaction direction,
a the effective ambient proportion and t the effective interaction strength.
For full unit normal n, define `k = clamp(n.x*d.x+n.y*d.y,-1,1)`.
If t is zero, define u=k without evaluating an absent direction. Otherwise
define `j = clamp(n.x*e.x+n.y*e.y,-1,1)` and
`u = (1-t)*k+t*j`. Finally `q = clamp((1-a)*u,-1,1)`.
This is a convex directional mixture followed by bounded attenuation toward the
body. The stable key input remains unchanged; optional interaction is a separate
physical contribution. Material differences use the existing authored family
pigment profiles, without an additional material parameter collection.

For q>0 mix body to highlight with weight q; for q<0 mix body to side with
weight -q, using the encoded-sRGB formula from
[relief paint](83-opaque-relief-paint.md). Zero q returns the exact body. Alpha
is exactly one. An exact flat normal `{x:0,y:0,z:1}` returns the exact body for
every admitted light, preserving the guarded uniform content plateau.

The convex weights keep q in [-1,1]. Every color therefore remains within the
existing separate body-to-side and body-to-highlight channel ranges. Publish
both outward-rounded ranges, ordered body-to-highlight then body-to-side, using
the four-binary64-epsilon and adjacent-value rule in the relief paint contract.
These are source-color ranges, not spatial
adjacency, raster filtering or a new contrast certificate. Body, exterior edge,
foreground and focus colors do not change. Integration must retain the original
guarded content region and independently certify any newly exposed background.

## Public boundary and evidence

`resina-material-light <path|->` accepts one UTF-8 request up to 1 MiB. Success
exits 0 with one result and no diagnostic; invalid input exits 1 with a diagnostic
and no result; usage exits 2. Require named objects, all fields, supported version
0.1.0 and recursively unique decoded JSON names. Reject unknown, positional,
missing, nonfinite, out-of-range and invalid unit-normal inputs without partial
samples. The result retains the opaque pigment, normalized key, effective optional
channels, two source-color ranges and ordered signed weights and opaque colors.
Numeric channels, directions and weights use absolute tolerance 1e-12 without
relative tolerance; structure and exact-body invariants remain exact.

Proof requires authored vectors and an independent external protocol checker for
every material family, changed physical direction, RTL invariance, absent data,
policy suppression, valid zeros and invalid unused inputs. Complete paint and
accepted scenes in two independent realizations are subsequent integration
evidence; the headless operation alone does not establish them.
