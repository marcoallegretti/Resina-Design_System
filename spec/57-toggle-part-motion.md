# Sampled Toggle interaction paint (candidate, 0.1.0)

Blueprint sections 55–58, 59–62, 85–86, 98 and 180 require material response,
independent state feedback and reduced-motion adaptation. This contract connects
[scalar trajectories](44-spring-trajectories.md) to [Toggle part paint](54-toggle-part-paint.md).
It samples one track or thumb's interaction pigment and depth. It does not animate
selection color between roles, thumb travel, shape deformation or component semantics.

## Ownership and inputs

The [request](../schemas/toggle-part-motion-request.schema.json) requires the complete
static part inputs, explicit bodyMix/depthScale dynamics and initial states, and
finite nonnegative elapsed seconds. Channels are dimensionless; velocity uses
inverse seconds. They use the same response vocabulary and bounds as
[command motion](45-command-motion.md), without adopting ordinary-command state
restrictions. No spring coefficients, previous state or elapsed time have defaults.
Unknown, duplicate, missing, nonfinite and unsupported-version input fails.

The request MUST be a JSON object. Its `part` and `checkedColorRole` MUST be string names from their respective schemas, as in static part paint. Positional requests and object-encoded names are invalid.

Bind the actual selected or unchecked color role and material in the current
complete theme/environment. Preserve checked, hover, pressed, disabled and focused
independently. Disabled, pressed, hover, rest precedence chooses the authored
interaction target; checked chooses the current color role immediately and does
not change that precedence. Focus is immediate and belongs only to the track.
Paint does not authorize action or infer availability.

Policy precedence is actual reducedMotion, then immediate Cast, then spring for
Frost or Elastomer. Persistent Gel remains invalid. Use spec44's immediate endpoint
rule for either immediate policy, preserving visible target response. Do not change
reduced transparency, quality policy or capabilities to obtain success. Input
validation still applies to immediate policy. No renderer or platform detection
enters motion resolution.

## Sampling and validation

Sample both channels with spec44. Preserve each complete unprojected trajectory,
including velocity and settled status. For paint only, project bodyMix to [-1,1]
and depthScale to [0,1], recording lowerBound, upperBound or none for each as in
spec45. A projection is not a collision or evidence of settling. Continue from
unprojected returned states on retargeting; reset elapsed time to zero. Never
manufacture zero velocity or feed projected paint coefficients back into motion.

Apply the projected response to the actually selected theme-bound pigment and
semantic depth, then rerun body, adjacent-edge, content and track-navigation
geometry/contrast guards on this sample. A valid endpoint cannot certify a path.
On failure return a diagnostic and no partial IR. Do not hide a failure by changing
preferences, clipping paint, omitting focus or weakening contrast minima.

The [result](../schemas/toggle-part-motion-ir.schema.json) contains policy, current
authored target, both scalar trajectories/projections and complete partPaint. The
nested phase is the current target phase; its response is the actual rendered
sample. A moving rest phase may therefore have a nonidentity response. Static
part paint retains its exact rest identity requirement. Both shapes retain checked
state agreement, full supported states and track-only focus ownership.

## Component composition

Track and thumb may have independent dynamics, but each frame must use coherent
current signals, theme, environment, label and semantics. Resolve the current
track sample first; use its actual pigment as thumb adjacency/backdrop evidence.
Then resolve the thumb sample and assemble the [complete snapshot](55-toggle-snapshot.md).
Revalidate actual thumb/track contrast, both endpoint footprints, label exclusion,
track navigation and the existing stable target on every sample. Do not reserve
only the settled geometry or resize the target with feedback. If applying
[thumb travel](56-toggle-thumb-travel.md), validate that placement against this
frame's complete snapshot and retain its separate raw trajectory for retargeting.

Publish complete native content together after all preparation succeeds. Neither
an isolated part nor its sampled state proves full component conformance. Native
clocks, scheduling, events, assistive technology, calibrated material coefficients
and complete component styling remain separate requirements. The reference's
CommandMotionChannels/Policy/Projection types are shared body-response vocabulary;
Toggle state and selection ownership remain in the Toggle operation.

## Evidence and protocol

[Public cases](../conformance/ir/toggle-part-motion-cases.json) and the
[request fixture](../conformance/ir/toggle-part-motion-request.json) cover both
parts, selection, all interaction phases, retained projection velocity, damping
regimes, reduced motion and diagnostic failures. These explicit dynamics exercise
arithmetic and are not calibrated material defaults. Rust tests cover every
nonempty supported state set across all three persistent families and both parts,
exact settled/static equivalence, typed/source equivalence and retargeting.

`resina-toggle-part-motion <path|->` uses strict UTF-8 JSON up to 1 MiB. Success
exits 0 with complete JSON and no diagnostic; invalid input exits 1 with a
diagnostic and no output; usage exits 2. The [independent checker](../tools/check_toggle_part_motion_backend.py)
checks equations, exact target/policy/projection metadata, selected-role pigment,
contrast, depth and navigation, including ordinary/reduced motion for each family
and part. Linux and Windows CI run that checker.

Required GUIdo readback checks all opaque prepared pixels in 72 existing static
frames plus 216 sampled frames across both parts, selections, focus states, three
families, three times and three device scales. Tests explicitly use ordinary
motion and verify intermediate spring responses differ from both endpoints.
Captures are isolated diagnostic parts, not complete Toggle UI release evidence.

Joint native composition conformance adds 180 complete frames across Light/Dark,
Cast/Frost/Elastomer, LTR/RTL and both selections. It samples independent track
and thumb response dynamics, supplies the current track pigment to thumb
resolution, validates the coherent snapshot and applies checked thumb travel.
Tests cover initial/intermediate/settled press response, raw-state retargeting to
rest with selection reversal, immediate disabled semantics and focus removal,
reduced-motion endpoints for every channel, and wrapped labels at textScale 2.
The reserved target remains unchanged. A thumb resolved against a stale static
track pigment must fail snapshot adjacency validation for a moving sample.

The existing complete pixel-grid and full-label checks apply to these frames at
fractional device scale. The 180 frames supplement the 428 existing static/travel
frames. Retargeting preserves raw part/travel state and physical geometry at time
zero under unchanged allocation. Captures remain diagnostic component proportions;
this evidence does not publish native assistive technology or event routing,
calibrate motion coefficients, or certify complete component styling.
