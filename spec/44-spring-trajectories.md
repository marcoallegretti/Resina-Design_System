# Scalar spring trajectories (candidate, schema 0.1.0)

This contract extends the [normalized spring foundation](38-spring-reference.md) to explicit initial position, velocity and target. It supports continuous target changes in a moving scalar response. Resolution remains headless and independent of frame scheduling, material calibration, rendering and product events.

## Inputs and units

The [dynamics](../schemas/spring-dynamics.schema.json) require version 0.1.0, mass, stiffness, damping, positionThreshold and velocityThreshold, with the same coefficient validation as the normalized contract. Initial velocity belongs to the [initial state](../schemas/spring-state.schema.json), alongside position. Every state channel and target MUST be finite. The [request](../schemas/spring-trajectory-request.schema.json) requires version, dynamics, initial state, target, elapsed time and reducedMotion without defaults. Unknown or duplicate members, absent fields and unsupported versions MUST fail.

Choose one scalar coordinate unit L consistently. Position, target and positionThreshold use L; velocity and velocityThreshold use L/s. Elapsed time uses seconds and MUST be finite and nonnegative. Mass uses a consistent unit M, stiffness M/s² and damping M/s. These are authored response parameters, not measured material properties. Family-specific coefficients remain qualitative until calibrated; this contract invents no defaults.

The request, dynamics and initial state MUST be JSON objects. Positional arrays
are invalid before either ordinary motion or the immediate fallback is selected.

## Equation and normalization

For ordinary motion solve:

```text
m x″ + c x′ + k (x − target) = 0
x(0) = initial.position
x′(0) = initial.velocity
```

The [MIT damped harmonic motion derivation](https://web.mit.edu/8.Math/www/lectures/lec4/1.4.2.pdf) supports the unforced damped equation and characteristic roots. Translating the equilibrium to an explicit target and the stopping/accessibility policies below are Resina decisions.

Use the normalized contract's frequency ω, damping ratio ζ and time u. Set y₀ = initial.position − target and q = initial.velocity / ω. Solve y″ + 2ζy′ + y = 0 with y(0) = y₀ and y′(0) = q. For ζ < 1, β = √(1 − ζ²):

```text
y = exp(−ζu) [y₀ cos(βu) + (q + ζy₀) sin(βu)/β]
w = exp(−ζu) [q cos(βu) − (y₀ + ζq) sin(βu)/β]
```

For ζ = 1:

```text
y = exp(−u) [y₀ + (q + y₀)u]
w = exp(−u) [q − (q + y₀)u]
```

For ζ > 1 use the normalized contract's roots r₊ and r₋. Set A = (q − r₋y₀)/(r₊ − r₋), B = y₀ − A:

```text
y = A exp(r₊u) + B exp(r₋u)
w = r₊ A exp(r₊u) + r₋ B exp(r₋u)
```

Unsnapped output is x = target + y and velocity ωw. Do not divide by target − initial.position: a state already at the target can still have nonzero velocity and must move normally. Overshoot and negative coordinates are valid; results MUST NOT be clamped. Stable equivalent evaluation is permitted and required near critical damping and at extreme ranges. The normalized contract's numeric diagnostics apply. Avoid intermediate product overflow/underflow when the final exponentially scaled contribution is representable. A nonfinite required subtraction, normalization, energy bound or result MUST fail diagnostically, without changing the equation or dropping motion.

## Settling, retargeting and accessibility

Use the same conservative energy-radius rule: R = hypot(y, w). Settle if and only if R ≤ positionThreshold and ωR ≤ velocityThreshold. Preserve both bounds without prematurely underflowing before multiplication by ω. For a fixed target and nonnegative damping, the normalized energy derivative remains −4ζw². Settled output MUST be exactly the target with zero velocity.

At elapsed time zero an ordinary, unsettled result MUST preserve the exact initial position and velocity. To change targets at a known elapsed time, sample the existing trajectory at that time, use its returned state as the new initial state, supply the new target, and reset elapsed time to zero. This preserves position and velocity unless the new state is already within both stopping bounds. Acceleration may change because the restoring force changes. Energy need not decrease across a target change. Supplying zero velocity on every target change erases continuity and is not this retargeting procedure.

Evaluate each trajectory from its fixed initial state and elapsed time. Output MUST be deterministic and independent of frame cadence. Resampling an intermediate state with the same target gives the same continuous solution within conformance tolerance, until endpoint snapping changes the initial state. The sampler does not read a clock or schedule frames.

When the effective reducedMotion preference is true, validate all inputs and return representation `immediate`, the exact target, zero velocity and settled true, including at time zero. Derived ordinary-motion quantities need not be representable in this fallback. Invalid time, target, state or dynamics still fail. Re-enabling motion starts from the actual current state; do not restore an obsolete pre-fallback velocity. Components still owe visible final-state feedback.

## Results and conformance

The [result](../schemas/spring-trajectory-result.schema.json) contains schemaVersion, representation, target, state and settled. Representation is `spring` or `immediate`. No output may contain a nonfinite number. State/target equality for settled results is checked semantically by conformance; standard JSON Schema cannot compare arbitrary sibling numeric values.

[Public vectors](../conformance/motion/spring-trajectory-cases.json) cover every damping regime, signed targets, velocity at equilibrium, exact initial conditions, settling, reduced motion and diagnostic failures. Numeric state samples use `max(1e-9, 1e-9 × abs(expected))`; target and position of a settled result MUST also agree exactly. Metadata agrees exactly. Independent Runge–Kutta tests check the equation; retarget tests check continuity, same-target restarts and conservative stopping.

The Rust reference exposes `sample_spring_trajectory` with validated dynamics/state and explicit target/time/preference, and `resolve_spring_trajectory_source` for strict JSON. `resina-spring-trajectory <path|->` uses the existing UTF-8, 1 MiB and atomic output protocol: success 0, diagnostic failure with no result 1, invalid usage 2; `--help` succeeds. [The backend checker](../tools/check_spring_trajectory_backend.py) verifies public cases for independent implementations. The original normalized contract retains its semantics and shares this reference solver.

Material response trajectories, capability-based motion policy, deformation, bounded paint channels and native scheduling remain separate contracts. This scalar sampler alone does not constitute a complete animated component.
