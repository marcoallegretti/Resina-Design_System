# Opaque paint color coverage (candidate, 0.1.0)

This operation derives conservative [opaque color ranges](72-bounded-background-contrast.md)
from a complete [opaque surface IR](36-opaque-surface-ir.md). It supports common
edge and focus contrast when another part can border its painted regions. It
describes potential surface colors, not their spatial adjacency or area.

## Coverage

Retain separate uniform ranges for the resolved body and exterior edge. Include
the uniform side pigment when the physical side offset is nonzero. A zero offset
has no side region. If both highlight width and highlight lift are positive,
include a channel box covering the continuous directional highlight from the
body to its full white-lift endpoint. Otherwise that modulation is absent.
Never replace that continuum with a list of sampled colors or merge disconnected
flat regions into one box. A potentially covered region need not have a visible
sample in every geometry; over-coverage is permitted, under-coverage is not.

Bounds MUST cover the implemented paint arithmetic, including its numerical
error. The mathematical white-lift law is monotone on each encoded sRGB channel
for a bounded weight in [0,1]. The reference computes alpha as lift times weight,
then computes `alpha + body * (1-alpha)`. The represented alpha remains within
[0,lift]; multiplying by white is exact. The subtraction, body multiplication
and addition each have absolute rounding error at most half a binary64 epsilon
on these bounded operands. Propagation multiplies errors by a body channel at
most one. The sample and stored endpoint errors together are at most three
epsilons. Four epsilons of padding, followed by outward adjacent-representable
rounding, therefore encloses both the mathematical continuum and reference
paint. Clamp only the resulting bounds to the valid encoded [0,1] gamut; do not
change the resolved pigments or paint.

This numerical construction is a Rust reference choice, based on its
[floating-point arithmetic and adjacent values](https://doc.rust-lang.org/std/primitive.f64.html).
Other implementations must establish their own valid bounds for their precision.
Renderer quantization, antialiasing, filtering, blending and treatment are
separate transformations; these ranges do not certify their output.

## Ownership and evidence

The typed reference `OpaqueSurfaceIr::paint_color_ranges` derives a nonempty set
from the exact resolved surface. No pigment, family, light, environment, geometry
or semantic role is inferred or changed. It does not add a source protocol or
alter existing IR serialization. All families and tiers use the same paint law.

A consuming component must combine all actual neighboring surfaces and canvas
regions, prove coverage at the edge or focus location, and resolve one common
candidate before publication. Whole-surface ranges can conservatively reject a
candidate because a represented color is not actually adjacent. A consumer may
use narrower proved spatial bounds; it must not discard colors merely because
contrast fails. These ranges certify neither content backgrounds nor visibility,
focus geometry, hit targets, accessibility semantics or complete component paint.

[Public vectors](../conformance/ir/opaque-paint-range-cases.json), with a
[strict schema](../schemas/opaque-paint-range-cases.schema.json), check separate
flat regions, suppressed side/highlight modulation, every family and
chromatic/gamut boundary pigments. Direct paint samples across square
and rounded geometry must lie in a derived range, including intermediate
highlight weights. Weight sweeps and adjacent represented endpoints verify
numerical coverage independently of geometry, including subnormal coefficients
and gamut neighbors. Existing paint results remain unchanged; no visual preset
or native feature is introduced.
