# Bounded surface deformation and same-object emergence (candidate, 0.1.0)

The [deformation decision](https://github.com/marcoallegretti/Resina-Design_System/issues/132)
makes in-plane deformation time-varying parameters of the existing geometry,
not a new geometry kind. This contract samples two operations: a bounded size
deformation of one part about its centre, and the emergence of one object
between two validated outlines. Both reuse the [scalar spring
trajectories](44-spring-trajectories.md) and the [surface outline](81-surface-outline.md).
Neither schedules frames, resolves paint, reads a clock, acquires input or
selects dynamics. [Authored response calibration](80-material-response-calibration.md)
and the callers supply every coefficient; there are no defaults.

Each sample is a valid spec/81 outline. The [lit control body](88-lit-control-body.md),
[extruded contour](35-extruded-contour.md), [opaque surface IR](36-opaque-surface-ir.md)
and [focus indicator](37-focus-indicator-ir.md) apply to that outline per sample,
unchanged, and so do their guards; a sample is not accepted by this contract
alone, only its geometry is. No operation here names a renderer, toolkit,
compositor or platform, and the analytic size channels need no `deformation`
renderer capability: that capability stays reserved for nonaffine warp.

## Shared rules

**Family and policy.** Select the policy in this order: reduced motion `true`
gives `reducedMotion`; the Cast family gives `castImmediate`; Frost and
Elastomer give `spring`. Gel is rejected by both operations: its dynamics are
not calibrated, and curing a Gel transient into a persistent role is deferred
to its own decision. A rejected request publishes nothing.

**Samples.** Each channel is a spec/44 trajectory sampled from a retained
initial state, an explicit target and the common elapsed time `time` (seconds,
finite, nonnegative). Under `castImmediate` and `reducedMotion` a channel is
the spec/44 immediate endpoint: position equals the target, velocity is zero
and the sample is settled. All dynamics and states are validated even when the
policy ignores them. The complete unprojected scalar sample is published.

**Projection.** For painting geometry only, a channel position is projected to
its authored closed interval and the marker `lowerBound`, `upperBound` or `none`
is published, exactly as in [command motion](45-command-motion.md). Projection
is not a collision response or a settled signal. To retarget, resolve a new
request whose initial state is the returned **unprojected** state and whose
elapsed time is zero; never feed the projected value or a manufactured zero
velocity back. A state outside the interval, including a far one, is accepted
as an initial state.

**Content is rigid.** Content is a rectangle in the same physical coordinates
as the outline, never scaled, squashed, refracted, moved or resampled. It is
published exactly as authored.

**Direct manipulation never lags.** Only the channels named below are springs.
No operation here has a position channel: a position under direct manipulation,
such as a Slider thumb or a Toggle thumb while it is dragged, is placed by its
own contract without spring lag, and a request carrying any other channel is
rejected. The sampled travel of an unmanipulated Toggle thumb remains
[spec/56](56-toggle-thumb-travel.md).

**Reduced motion.** A consumer in reduced motion receives the `reducedMotion`
policy, the immediate target endpoint with zero velocity and the outline of that
endpoint, at every elapsed time including zero. The endpoint is whatever the
caller authored as the target; a caller that wants a rigid reduced-motion
result authors a rigid target. No intermediate transition is defined here. The
caller owes visible final-state feedback through the pigment, edge and depth
channels of the existing contracts. Re-enabling motion starts from the actual
current state.

**Economy quality.** A consumer may render the rigid rest outline instead of a
sample, because rest is a valid outline; the contract defines no quality input.

## Deformation

The [request](../schemas/surface-deformation-request.schema.json) supplies:

- `outline`, the rest [spec/81 request](81-surface-outline.md) with its rest
  size `W` by `H`, corner extents, direction, curvature and `minimumInset`;
- `content`, the rigid label box, in the same coordinates as the outline at rest;
- `sweepOffset`, the [key-light side offset](34-key-light.md) of the deepest
  silhouette the part can paint, a finite physical vector;
- `family`, `reducedMotion` and `time`;
- `bounds.widthScale` and `bounds.heightScale`, each an authored closed
  interval `[lower, upper]`;
- `channels.widthScale` and `channels.heightScale`, each with `dynamics`,
  `initial` and a `target`, all dimensionless; velocities use inverse seconds.

The scales multiply the rest size about the part centre: the sampled size is
`(W·sx, H·sy)` and its origin is `((W − W·sx)/2, (H − H·sy)/2)`, in rest
coordinates, so the centre never moves. Corner extents stay authored; the
sampled outline normalizes them at the sampled size by the bounded-corner factor
of [Tier 0 geometry](09-geometry.md) inside spec/81, so a smaller silhouette
shrinks all four corners uniformly and corner circularity is preserved.

**Bounds are authored and limited.** Each interval must satisfy
`0 < lower ≤ 1 ≤ upper`, so rest is always inside it, and each target must lie
in it. Each family also has a deformation limit `L`; evaluated in binary64, the
interval must satisfy `lower ≥ 1 − L` and `upper ≤ 1 + L`:

| Family | `L` | Meaning |
| --- | ---: | --- |
| Cast | 0 | rigid; the interval is exactly `[1, 1]` |
| Frost | 0.02 | small |
| Elastomer | 0.06 | tactile |
| Gel | | rejected |

The limits are reference authoring choices, not measured material properties,
and remain subject to visual acceptance. They bound what a family may declare;
the author's own interval may be narrower.

**Content stays in the uniform face.** At every sample the label box must lie
inside the spec/81 `contentBounds` of the sampled outline, translated by its
origin. Because `contentBounds` is a conservative rectangle at inset
`minimumInset`, an author sets `minimumInset` to the inset of the uniform face
the part needs; the exact uniform-face law of [command label
content](46-command-label-layout.md#lit-command-body-content) and the body
guards still apply to the actual sample. This bounds compression. The check is
not left to the sampled instant: the label box must lie inside the content
bounds at all four corners `(lower|upper) × (lower|upper)` of the declared
domain, which covers every sample inside it. The corners are not all equivalent, because a
smaller width also lowers the corner normalization of the vertical
extents, so the worst corner for one edge is not the smallest silhouette; a
public case contains a box that fits the smallest silhouette and not another
corner. The Python test compares the corner decision with a dense grid of the domain.

**Reservation.** Hit regions and reserved targets stay fixed and do not follow
the sample. The result's `reservation` is the smallest rectangle covering the
outline at the upper scales `(W·upperW, H·upperH)` about the centre and the same
outline translated by `sweepOffset`. Every sampled silhouette is inside it,
because each sampled rectangle is inside the largest one and a translated
rectangle's bounds are the union of its own and the original bounds. Layout owns
the [target](49-command-snapshot.md#stable-interaction-geometry); this contract
neither moves nor enlarges it. The focus ring follows the sampled silhouette and
keeps the [focus gap](37-focus-indicator-ir.md) exact, because it is resolved
from the sampled outline; the ring is not part of the reservation.

The [result](../schemas/surface-deformation-result.schema.json) contains the
policy, both complete scalar samples, both projection markers, the `origin`, the
complete spec/81 `outline` of the projected size, and the `reservation`.

## Emergence

Emergence is one object changing between two validated outlines. It is not a
family swap, a lifecycle or a product notification.

The [request](../schemas/surface-emergence-request.schema.json) supplies
`family`, `reducedMotion`, `time`, `emerged`, `sweepOffset` and two ends, `from`
and `to`. Each end has an `identity`, a spec/81 `outline`, a physical `origin` of
that outline in a shared frame, and a rigid `content` box in that frame. The
channels are `progress` and `opacity`, each with `dynamics` and `initial`. The
target of both is 1 when `emerged` is true and 0 otherwise.

**One identity and one accessible node.** Both identities must be equal and
nonempty; the result carries the single identity. Progress neither creates nor
destroys an accessible node, and the accessible semantics of
[command accessibility](47-command-accessibility.md) are resolved from the
current state, independently of progress. This contract resolves no semantics.

**Validated ends.** Both outlines must share direction and curvature. Each end
must resolve as a spec/81 outline and its own content must lie inside the
content bounds of its own outline, so the endpoints are always valid samples.

**Progress.** Project progress to `[0, 1]`, publish the marker and keep the
unprojected sample, as above; overshoot is therefore never extrapolated. With
the projected weight `s`, the sampled size, each logical corner extent, the
`minimumInset` and the origin are `a·(1 − s) + b·s` of the two ends. Every
interpolant is a convex combination of valid values, so the sample is a valid
outline unless spec/81 cannot represent it, in which case the request fails.
At `s = 0` and `s = 1` the outline and origin equal the ends exactly.

**Content.** Let `q` be the opacity channel projected to `[0, 1]`. The source
content has opacity `1 − q` and the destination content `q`. Content is
cross-faded in place and never scaled; it is published as `null` when its
opacity is exactly 0, so a settled, cancelled or immediate sample never carries
the other state's content. Visible content must lie inside the content bounds of
the sampled outline; a request that would show content outside the sampled face
fails rather than clipping or scaling it. An author delays a large destination
content by starting the opacity state below 0, which projects to 0 until the
spring crosses it. Opacity dynamics must not overshoot: with mass `m`, stiffness
`k` and damping `c`, `c² ≥ 4mk` is required, evaluated in binary64 and checked
even in the immediate policies. Opacity from rest then stays inside `[0, 1]`;
a retained state may still project.

**Cancellation.** Reversing `emerged` is a retarget from the retained unprojected
states of both channels. Samples stay continuous in outline, content opacity and
channel state, and a settled cancellation shows only the source content.

**Reservation.** The result's `reservation` is computed as for deformation
from the two ends' rest rectangles, and covers every progress in `[0, 1]`
because each sampled rectangle is a convex combination of the two.

**Not defined here.** Pigment or role change between the ends, including curing
a Gel transient into a persistent role, is not interpolated or selected by this
contract. Each sample's paint is resolved by its owning paint contract on the
sampled outline.

The [result](../schemas/surface-emergence-result.schema.json) contains the
policy, identity, both complete scalar samples, both markers, the `origin`, the
complete spec/81 `outline`, `fromContent` and `toContent` (each `null` or its
bounds and opacity), and the `reservation`.

## Failures and evidence

Reject unknown, missing and duplicate members, positional arrays, strings in
place of numbers, nonfinite or unrepresentable numbers (an underflowed bound
becomes zero and fails the positivity rule), unsupported versions, Gel, bounds
outside the family limit or not containing rest, targets outside their bounds,
negative or nonfinite time, invalid dynamics, content without positive area,
content outside the uniform face, mismatched identities, directions or
curvatures, overshooting opacity dynamics, and any outline spec/81 rejects.

The [deformation](../conformance/motion/surface-deformation-cases.json) and
[emergence](../conformance/motion/surface-emergence-cases.json) cases change a
[deformation](../conformance/motion/surface-deformation-request.json) or
[emergence](../conformance/motion/surface-emergence-request.json) baseline and
state the policy or a reason for failure. They cover all three policies, the
family limits on both sides, projection at both bounds with retained velocity,
retargeting, far and sub-bound initial states, asymmetric right-to-left quartic
outlines, a very large part, reduced motion at time zero and later,
endpoint continuity, delayed destination content, cancellation and every
rejection above.

[Independent arithmetic](../tools/surface_deformation_arithmetic.py) derives
every expected result without the reference: exact rational geometry, the
spec/81 content reservation, the spec/44 equation of the existing independent
checker, projection, interpolation, content visibility and reservation. The
[checker](../tools/check_surface_deformation_backend.py) compares the command
output with it, using spec/44 tolerance for scalar samples and `1e-12` absolute
or relative for geometry, and requires exit 1 with a diagnostic and no result
for every invalid request. Its [tests](../tools/test_surface_deformation_backend.py)
check the corner decision against a dense grid, reservation coverage, the fixed
centre, exact endpoints and that every compared member detects a change. Rust
tests additionally check each sampled outline against the public spec/81
command, centre and bounds across sampled times, retained unprojected state and
exact restart, immediate endpoints, cancellation, and the absence of a position
channel or content scale.

`resina-surface-deformation <path|->` and `resina-surface-emergence <path|->` read
one UTF-8 request up to 1 MiB; success exits 0 with one complete JSON result,
invalid input exits 1 with a diagnostic and no result, and usage errors exit 2.
Rust exposes `resolve_surface_deformation_source` and
`resolve_surface_emergence_source`.

## Limitations

This contract proves headless geometry, content and reservation. It does not
realize deformation in any backend, define a pigment law across emergence, a
Gel cure, nonaffine warp, native clocks or frame scheduling, or accept any
appearance: the family limits and the example dynamics are reference choices
and the fixture dynamics exercise equations only. No native, raster or visual
evidence for these samples exists yet.
