# Normalized scalar spring reference

This contract implements the scalar spring foundation required by blueprint §56. It defines an unforced response from normalized position 0 to endpoint 1. The caller supplies the coefficients and accessibility preference. It does not select material profiles, retarget a moving component, schedule frames, or define component motion IR. Cast, Frost, Elastomer and Gel motion parameters remain uncalibrated.

## Input and units

The [parameter schema](../schemas/spring-parameters.schema.json) requires version `0.1.0`, `mass` (m), `stiffness` (k), `damping` (c), `initialVelocity` (v₀), `positionThreshold` and `velocityThreshold`. All numbers MUST be finite. Mass, stiffness and both thresholds MUST be positive; damping MUST be nonnegative. Initial velocity MAY have either sign. No parameter has an implicit default.

Time is elapsed seconds from the initial condition and MUST be finite and nonnegative. Position is dimensionless; velocity and its threshold are normalized position per second. Mass uses any consistent mass unit M, stiffness uses M/s², and damping uses M/s. These are interface response parameters, not measured physical properties of a material. Multiplying m, k and c by the same positive factor preserves the response.

The [request schema](../schemas/spring-request.schema.json) requires `schemaVersion`, `spring`, `time` and `reducedMotion`. Reduced motion MUST come from the effective accessibility preference. Objects MUST reject unknown fields, missing fields, unsupported versions, and duplicate JSON members. Inputs MUST be validated before applying the fallback.

The request and its spring parameters MUST be JSON objects. Positional arrays
are invalid, including when reduced motion selects the immediate fallback.

## Equation and initial conditions

For ordinary motion, the response MUST solve

```text
m x″ + c x′ + k (x − 1) = 0
x(0) = 0
x′(0) = v₀
```

Define natural angular frequency ω = √(k/m), damping ratio ζ = c/(2√(mk)), normalized time u = ωt, initial normalized velocity q = v₀/ω, displacement y = x−1 and normalized velocity w = x′/ω. Derivatives below are with respect to u. Then y″+2ζy′+y=0, y(0)=−1, y′(0)=q.

For ζ < 1, set β=√(1−ζ²):

```text
y = exp(−ζu) [−cos(βu) + (q−ζ) sin(βu)/β]
w = exp(−ζu) [q cos(βu) + (1−ζq) sin(βu)/β]
```

For ζ = 1:

```text
y = exp(−u) [−1 + (q−1)u]
w = exp(−u) [q + (1−q)u]
```

For ζ > 1, β=√(ζ²−1), r₊=−ζ+β, r₋=−ζ−β, A=(q+r₋)/(r₊−r₋), B=−1−A:

```text
y = A exp(r₊u) + B exp(r₋u)
w = r₊ A exp(r₊u) + r₋ B exp(r₋u)
```

These mathematical formulas define the response, not an evaluation order. Implementations SHOULD use stable equivalent forms near critical damping and at large damping ratios, and scale products before exponential underflow. They MUST return a diagnostic error when required nonzero normalized quantities cannot be represented, or a required normalized quantity or returned value cannot be computed finitely; they MUST NOT substitute another damping regime or silently remove damping. Ordinary exponential decay to zero is permitted, including evaluation of an exponential argument tending to negative infinity. Unsnapped position and velocity MUST NOT be clamped: overshoot and initial movement away from the endpoint are valid.

The [MIT damped harmonic motion derivation](https://web.mit.edu/8.Math/www/lectures/lec4/1.4.2.pdf) supports the differential equation, characteristic roots and dissipating energy. The normalization, stopping rule and accessibility policy here are Resina decisions. Flutter's [spring reference implementation](https://github.com/flutter/flutter/blob/master/packages/flutter/lib/src/physics/spring_simulation.dart) is informative evidence for treating all damping regimes explicitly; its defaults and snapshot stopping rule are not part of this contract.

## Settling and endpoint

Compute the energy radius R=√(y²+w²). A sample is settled if and only if both R ≤ positionThreshold and ωR ≤ velocityThreshold. Implementations MUST evaluate both bounds without prematurely underflowing R before multiplying by ω.

Differentiating R² gives −4ζw² ≤ 0. Consequently, without retargeting or adding energy, future displacement magnitude is bounded by R and future physical velocity magnitude by ωR. This conservative rule protects against marking an oscillation settled at a momentary low velocity or endpoint crossing. It can settle later than a rule that checks only current position and velocity.

A settled sample MUST return position exactly 1 and velocity exactly 0. Otherwise it returns x=1+y and velocity=ωw; at time zero its position MUST be exactly 0. Stateless evaluation at any time MUST give the same response irrespective of previous samples or frame rate. Retargeting or an external impulse establishes a different problem and is outside this operation.

When `reducedMotion` is true, the representation MUST be `immediate`, position 1, velocity 0 and settled true, including at time zero. The caller MUST preserve final-state feedback such as changed pigment, edge or indicator; removing motion MUST NOT remove acknowledgement of the action. The ordinary representation is `spring`. Immediate resolution needs no representable spring frequency after input validation. Capability-based motion policy remains a separate contract.

## Results and conformance

The [result schema](../schemas/spring-result.schema.json) defines version, representation, position, velocity and settled status. No result may contain NaN or infinity. [Public vectors](../conformance/motion/spring-cases.json) cover all damping regimes, initial conditions, endpoint settling, reduced motion, validation and numerical diagnostics. Successful numeric samples MUST agree within `max(1e-9, 1e-9 × abs(expected))`; representation, version and settled status MUST agree exactly. The tolerance is a conformance comparison rule, not permission to settle early or an accuracy guarantee for every ill-conditioned binary64 input.

The Rust `resina-motion` crate is the canonical reference. `sample_spring` accepts validated model parameters; `resolve_spring_source` accepts strict JSON. The `resina-spring <path|->` command accepts one UTF-8 request of at most 1 MiB and writes one result. Exit status is 0 for success, 1 for a diagnostic failure with no result, or 2 for invalid command usage. `--help` succeeds. `tools/check_spring_backend.py` checks the same public protocol for independent implementations.

[Scalar spring trajectories](44-spring-trajectories.md) define explicit initial state and target changes as a separate contract using the same reference dynamics.
