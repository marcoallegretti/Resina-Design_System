# Checked Toggle thumb travel (candidate, 0.1.0)

Blueprint sections 55–57, 85–86, 98, 113 and 180 require material response,
reduced-motion adaptation, stable interaction geometry and headless resolution.
This contract samples horizontal thumb travel on a complete checked
[Toggle snapshot](55-toggle-snapshot.md). It changes placement only; it does not
interpolate pigment, interaction response, shape or component semantics.

## Inputs and ownership

Use a current coherent snapshot and explicit validated [scalar dynamics and
initial state](44-spring-trajectories.md), plus nonnegative finite elapsed seconds.
No coefficients, previous position or velocity are synthesized. The scalar channel
is dimensionless: off is 0, on is 1, velocity is inverse seconds. The target is
1 exactly when current checked is true; it is independent of hover, press,
availability and actual focus. The semantic state and static checked-role paint
are current immediately, even while the thumb travels from its previous position.

Use the thumb's actually resolved material family. The snapshot retains the
reduced-motion preference used for its resolution. Policy precedence is reduced
motion, then immediate Cast, then spring for Frost or Elastomer. Gel is not a
persistent Toggle part. No toolkit, platform, device or renderer detection enters
this policy. Both immediate policies use the validated scalar endpoint rule:
position equals target, velocity is zero and the sample is settled. Invalid time
still fails. Reduced transparency and rendering capabilities remain owned by the
snapshot's existing part paint; no preference is altered to obtain success.

## Bounded placement and retained motion

Sample the trajectory under spec44. Retain its complete unprojected state and
settling result. For placement only, project progress to [0,1]. Record
`offEndpoint` below 0, `onEndpoint` above 1, and `none` at or inside the interval.
This is explicit saturation, not a spring collision or a settled signal.

Let off and on be the snapshot's allocated endpoints. For progress 0 or 1,
use the corresponding endpoint exactly. Otherwise the physical x origin is
`off.x + (on.x - off.x) * progress`; y and size remain the off allocation's values.
Logical direction already belongs to the endpoint allocation, so RTL changes
travel direction without mirroring lighting. Nonfinite placement or an interior
progress swallowed into either endpoint by binary64 arithmetic MUST fail.

Recheck the complete placed thumb silhouette against the actual uniform convex
track content region, stable reserved target and complete external label slot.
Use the same conservative full-footprint-box certificate as the static snapshot.
A failed guard returns no travel result. Contrast remains the snapshot's checked
thumb/track and external-label contrast: placement remains within the same
uniform backdrop, and this operation changes no paint. Endpoint certification
alone MUST NOT replace actual sampled-placement checks.

The result retains the original complete snapshot, policy, full scalar trajectory,
projection and checked thumb bounds. It does not mutate the snapshot's selected
static endpoint, target, semantics, track navigation or label. Consumers render
this checked placement instead of deriving a second toolkit interpolation.
Publish complete track/thumb/label content together after preparation succeeds.

On retargeting, resolve a fresh snapshot with current signals, continue from the
unprojected returned scalar state, and reset elapsed time to zero. Never feed the
projected coordinate or fabricated zero velocity into the spring. Keep the same
allocation to preserve physical position/velocity continuity; layout or context
changes require explicit owner reconciliation and new snapshot validation. Replace
snapshots when preferences, eligibility, layout, typography or paint inputs change.
A travel snapshot is a rendering certificate, not action authorization.

## Evidence and implementation boundary

[Public vectors](../conformance/motion/toggle-travel-cases.json) use explicit
critical damping, with on position `1 - (1+10t) exp(-10t)` and off position
`(1+10t) exp(-10t)` from rest. They cover intermediate values, retained overshoot
velocity, midtravel state and settled endpoints. Their dynamics exercise equations;
they are not calibrated material defaults. The [case schema](../schemas/toggle-travel-cases.schema.json)
and repository schema gate validate complete records. Rust tests consume these
vectors in both directions and check immediate policies, invalid time, current
semantics, stable targets and retarget state. Scalar tolerance follows spec44;
physical geometry follows the existing binary64 conformance budget.

The Rust reference exposes `resolve_toggle_travel` and an immutable
`ToggleTravelSnapshot` view of checked component data. Like static snapshot
assembly, this is a typed operation; it introduces no serialized component
request or partial output protocol. GUIdo's `prepare_toggle_travel_content` consumes
the checked thumb bounds and the complete original snapshot. Required GPU readback
covers 32 travel frames in actual Light/Dark Elastomer scenes, both directions,
both selections, intermediate and settled states at fractional device scale.
Native clocks/frame scheduling, continuous interaction/paint response motion,
material calibration, keyboard delivery and native accessibility remain separate
requirements. No interactive product UI or complete Toggle conformance is claimed.

The ordinary-travel GPU fixture explicitly sets reducedMotion false before
resolving paint and snapshots, and asserts spring policy, opposite initial
placement, intermediate non-endpoint placement and exact settled placement.
Reduced-motion policy is tested separately in the portable snapshot matrix.
