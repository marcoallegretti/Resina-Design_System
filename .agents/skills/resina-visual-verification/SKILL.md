---
name: resina-visual-verification
description: Use for Resina material appearance, text, motion, deterministic raster/GPU evidence and Lab image review.
---

# Visual verification

Read [AGENTS.md](../../../AGENTS.md), the affected appearance/geometry/typography
contracts, [shared scenes](../../../conformance/scenes/README.md),
[raster evidence](../../../conformance/raster/README.md) and the owning backend
README. [The Material Board](../../../lab/guido/README.md) is a static native
catalog consumer, not proof of complete interactive components.

Select repeatable scenarios before implementation. Cover the relevant materials,
Light/Dark themes, current interaction/focus, RTL, wrapping, device scale, text
scale, reduced motion/transparency and actual lower-capability fallback. Select
combinations justified by the changed behavior rather than claiming every possible
combination from a small fixture.

Compare deterministic semantic/geometry results, placed pixel grids and native
GPU readback as appropriate. Inspect the images themselves for hierarchy,
spacing, typography, material coherence, clipping, contrast and focus. Tier 0
must retain Resina identity. Arithmetic fixtures and structural black/white
component scenes are not accepted material styling or calibrated motion.

Use explicit fonts and actual device/text scales. Preserve scenes' authored theme
and environment. For a native capture, follow the backend README's capture
variable or Lab CLI and save outputs under ignored `target/` paths. Existing
Material Board output is not overwritten; choose a new filename for each capture.

For motion, verify retargeting continuity, raw retained state, settled behavior,
immediate preference fallback and guarded intermediate paint. Native scheduling
and platform interaction need separate evidence where claimed.

Explain expected-image changes against the intended contract. Never re-bless
evidence merely to remove a failure. Record the scenario, scales, fonts, actual
native environment, observed result and any boundary left unverified.
