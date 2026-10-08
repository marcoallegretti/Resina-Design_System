# Sampled command paint (candidate, schema 0.1.0)

This contract connects [scalar trajectories](44-spring-trajectories.md) to [opaque command paint](43-command-paint.md). It resolves a complete paint sample with retained motion state. It does not schedule frames or calibrate material coefficients. The blueprint's material motion values remain qualitative until calibrated.

## Ownership and inputs

The [request](../schemas/command-motion-request.schema.json) requires version 0.1.0, the complete surface request, commandAppearance, channels and elapsed time. The request MUST be a JSON object; positional request records are invalid. Each channel (`bodyMix`, `depthScale`) requires validated dynamics and an explicit initial position/velocity. Coefficients have no defaults. Time is seconds; both channels are dimensionless and their velocities use inverse seconds. Unknown, duplicate, missing, nonfinite and unsupported-version inputs MUST fail. Unbounded finite initial state is permitted, including when continuing an overshooting trajectory.

The channels container and each channel MUST be JSON objects. Positional arrays
are invalid before selecting any motion policy, including immediate policies.

Resolve the actual theme/environment once and bind the actual material family and current state set. The existing command phase rule chooses the authored target response; focus remains independent. Never derive enablement or invocation from paint. Select policy in this order:

1. Effective environment reducedMotion true: `reducedMotion`.
2. Bound family Cast: `castImmediate`.
3. Bound family Frost or Elastomer: `spring`.

Gel and unsupported command states remain diagnostic failures. Cast's immediate response follows the blueprint's material motion table. This policy introduces no device, toolkit, renderer or tier detection. Ordinary geometry, pigment and focus painting work with Tier 0 primitives; depth compression does not require the in-plane `deformation` capability. Reduced motion retains the visible endpoint pigment/edge/depth feedback.

## Sampling and bounded paint

For `spring`, sample each channel with spec44 using the authored target and common elapsed time. For either immediate policy, use spec44's immediate endpoint rule. All explicit inputs remain validated, but derived ordinary-motion arithmetic need not be representable for immediate policy. No default spring or previous state may be synthesized.

Scalar dynamics are unbounded, while the command paint response has constrained channels. Preserve the complete scalar samples unchanged. Project bodyMix to [-1, 1] and depthScale to [0, 1] for painting only. Publish a projection marker for each: `lowerBound` below the lower bound, `upperBound` above the upper bound, `none` at or between bounds. This explicit saturation is a Resina paint law, not a change to the scalar equation, a collision response, or a settled signal. It prevents negative extrusion and invalid pigment arithmetic without erasing velocity or hiding projection. It is continuous in position, but its derivative can change at a bound.

On retargeting, continue from the returned **unprojected** scalar states and reset elapsed time to zero. Do not feed projected paint response or a manufactured zero velocity back into the spring. Reduced motion instead returns actual endpoint states with zero velocity. Neither channel's settling nor projection may stop the other channel; scheduling may stop when both samples are settled. Focus and semantic state feedback use the current state immediately and are not delayed by body motion.

Apply the projected response to the original theme-bound pigment and semantic depth. Run all body, edge and focus contrast/geometry guards on this actual sample. Endpoint validity does not prove intermediate contrast. A failed guard MUST produce a diagnostic and no partial IR; do not silently alter the authored target, preference or capability set. Products must supply calibrated profiles and valid paths before exposing animation.

## Result and evidence

The [IR](../schemas/command-motion-ir.schema.json) contains policy, authored target, both scalar samples, projection markers and a complete `command` paint sample. The nested command phase describes the current target state; its response describes the rendered sample. Unlike static endpoint IR, a moving rest phase need not yet have the identity response. The static command operation/schema retains its endpoint invariant.

Target values, policy, phase, projections and metadata are exact. Scalar state follows spec44 tolerance. Projection and paint consistency are checked against the accepted backend scalar samples, rather than the oracle's rounded state. Paint follows the existing command geometry/pigment laws and contrast guards. [Cases](../conformance/ir/command-motion-cases.json) and [backend checker](../tools/check_command_motion_backend.py) cover all eligible families/phases, damping regimes, intermediate and settled samples, saturation with retained velocity, reduced motion and strict failure paths. Fixture dynamics exercise equations; they are not calibrated material definitions.

Rust exposes `resolve_command_motion` and `resolve_command_motion_source`. `resina-command-motion <path|->` uses the existing strict UTF-8 1 MiB protocol: success 0, diagnostic failure with no result 1, invalid usage 2. The optional PNG CPU command `resina-command-motion-raster` uses the existing viewport/sampling arguments and outputs a complete paint sample. CPU and GUIdo consume the returned complete paint without implementing motion policy or interpolation. Native clocks, frame delivery, labels, accessibility exposure, complete components and material calibration remain separate responsibilities.
