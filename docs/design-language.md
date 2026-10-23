# Resina design language

This is informative guidance for designers, component authors and backend
implementers. It introduces no new normative behavior. The
[blueprint](../RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes intended direction;
the versioned [specifications](../spec/), [definitions](../definitions/),
[schemas](../schemas/), [tokens](../tokens/) and [conformance cases](../conformance/)
define implemented behavior. An implemented candidate contract is not a claim of
Stable maturity, accepted production styling or complete backend qualification.

## Material grammar

Persistence is expressed through curing, direct interaction through elasticity,
and transience through viscosity. These relationships communicate what a surface
does. They do not require a literal physics simulation.

| Family | Design intention | Current contract and evidence |
| --- | --- | --- |
| Cast | Stable structure and persistent content; calm edges and restrained relief. | [Role assignments](../spec/05-materials.md), [opaque pigment](../spec/29-opaque-pigment.md) and [surface IR](../spec/36-opaque-surface-ir.md) provide a static dimensional representation. [Material scenes](../conformance/scenes/README.md) exercise opaque rest and focus. |
| Frost | Pigmented, diffusive chrome that separates content from its surroundings. | [Fallback](../spec/31-fallback.md) and [readability](../spec/26-frost-surface-readability.md) preserve the family when opaque. The same scenes prove its opaque representation, not rendered backdrop diffusion. [Diffusive paint](https://github.com/marcoallegretti/Resina-Design_System/issues/130) remains a separate contract. |
| Elastomer | Tactile controls, compression and prompt recovery. | Static pigment and geometry are exercised by the material scenes; [Command motion](../spec/45-command-motion.md) and [Toggle motion](../spec/57-toggle-part-motion.md) define sampled channels. The candidate [response calibration](../spec/80-material-response-calibration.md) supplies authored body, depth and travel dynamics, and the candidate [bounded deformation](../spec/96-surface-deformation.md) samples a bounded size deformation of the outline within an Elastomer limit; no deformation dynamics are calibrated and no backend realizes it yet. |
| Gel | Viscous, expressive transient feedback over stable structure. | [Role assignment](../spec/05-materials.md) limits Gel to transient surfaces and feedback. Its static catalog specimen is not a persistent control or evidence of liquid motion. The candidate [emergence](../spec/96-surface-deformation.md) contract rejects Gel until its dynamics are calibrated and its curing is decided. |

Components request semantic material roles; themes assign families. Family,
pigment, shape, elevation, state and optical treatment are separate choices.
Gel cannot be assigned to persistent surface or control roles. Avoid making every
surface translucent or every interaction elastic: material variation carries
hierarchy rather than decorative noise.

[Lens-family treatments](../spec/06-optics.md) are an independent axis, not a fifth
material. The current contract validates one ancestor chain and rejects more than
one Lens-family treatment on that chain. It does not define optical rendering or
fallback; those belong to the [treatment proposal](https://github.com/marcoallegretti/Resina-Design_System/issues/131).

## Shape, presence and hierarchy

Use calmer geometry for structural surfaces and softer geometry for small
interactive surfaces. Reserve organic expression for a meaningful tactile or
transient role. A universal capsule shape removes this hierarchy.

[Semantic shapes](../spec/09-geometry.md) name structural, soft, rounded, capsule
and organic intent. The authored [Tier 0 shape assignments](../definitions/tier0-shapes.json)
map them to bounded circular or square corners, including asymmetric organic
corners. Continuous curvature is blueprint intent, not a property promised by
that fallback.

[Elevation](../spec/08-elevation.md) communicates embedded, base, raised,
floating, overlay and modal relationships. The current
[surface IR](../spec/36-opaque-surface-ir.md) realizes nonnegative side-plane
extrusion and guarded edge/highlight bands with a uniform content region. It does
not define bevel relief, a concave well or an independent physical shadow model.
The candidate [continuous outlines](../spec/81-surface-outline.md),
[bounded surface relief](../spec/82-surface-relief.md),
[opaque relief face paint](../spec/83-opaque-relief-paint.md) and
[outline focus geometry](../spec/84-outline-focus-geometry.md) own flat, convex
and recessed relief separately from elevation and shadow. Do not treat negative extrusion as a recess or a shadow
as the meaning of elevation.

Depth and light should clarify containment and interaction without competing
with the content. Basic shape, pigment, edge and separation remain important
when advanced rendering is unavailable.

## Light and pigment

Keep one physical key direction within a visual context. The
[key-light contract](../spec/34-key-light.md) uses physical right/down axes and
does not mirror with RTL layout. Logical corners are mapped to physical corners
before their normals are evaluated. Its directional weights are a portable
perceptual convention, not calibrated reflectance or a three-dimensional light.
[Bounded material light](../spec/85-bounded-material-light.md) adds optional
ambient and interaction response to that key light, and
[material-lit relief paint](../spec/86-material-relief-paint.md) applies it to a
relief face. Command bodies and Toggle parts are [lit control bodies](../spec/88-lit-control-body.md):
the lit relief face projected with the side plane and guarded edge, with
interaction light admitted only while hovered or pressed. Light never replaces a
state cue, and the reserved content stays the exact body colour.

Semantic color communicates emphasis and status; material pigment expresses the
selected family's surface. [Opaque pigment profiles](../spec/29-opaque-pigment.md)
derive explicit side and highlight colors without making a backend's shader the
definition of the material.

The authored [Light and Dark themes](../spec/23-authored-themes.md) pair a warm
mineral base and a deep blue-black base with teal primary, blue secondary and
amber tertiary accents. They supply semantic assignments, not physical fonts,
component proportions or a universal production motion profile.
[Theme derivation](../spec/91-theme-derivation.md) uses them as templates for an
explicit seed: the seed's Oklch hue replaces the accent, selection and focus hues
at template lightness and chroma, and both themes must pass the
[legibility guard](../spec/23-authored-themes.md#legibility-guard).
[Environmental pigment adaptation](../spec/95-environmental-pigment.md) lets an
optional observation select Frost chrome tint and Cast surface warmth inside
authored ranges. It never rebinds a role, leaves accents and status colours to
their own contracts, and keeps the legibility guard.

## Content provides calm structure

[Typography roles](../spec/11-typography.md) express hierarchy independently of
physical font files. Backends supply real measurements and shaping. Preserve
normal text and semantic icons from deformation and optical distortion.

Allow labels to grow, wrap or reflow under translation and increased text scale.
[Command label layout](../spec/46-command-label-layout.md) accepts complete
producer measurements rather than guessing width. A fixed narrow text slot or
silently smaller text undermines both hierarchy and accessibility.

Symbolic icons should identify controls; full-color artwork can identify product
content. [Icon content](../spec/87-icon-content.md) describes an icon's
reference, bounds, direction and colour layers without carrying artwork, and
[Command content](../spec/90-command-content-accessibility.md) names an
icon-only Command from a required accessible label, never from the icon. Icon
size roles, placement, contrast, mapping lookup and rendering remain
[proposed work](https://github.com/marcoallegretti/Resina-Design_System/issues/135),
so there is no portable glyph or asset contract yet. Physical light direction
and an icon's logical mirroring policy are different concerns.

The [content design guide](content-design.md) covers how interface copy communicates:
which wording rules are already normative, which are advisory, and which wait for
a component contract. Its wording rules are informative.

## Motion and interaction

Motion expresses material response and continuity, then settles. The
[scalar spring](../spec/38-spring-reference.md) and
[retargetable trajectories](../spec/44-spring-trajectories.md) define deterministic
sampling and retained position/velocity. Command body mix/depth and Toggle thumb
travel are bounded sampled contracts; frame scheduling belongs to integration.

Their arithmetic fixtures do not establish accepted material calibration.
[Slider appearance](../spec/71-slider-appearance.md) makes the same distinction.
Do not substitute an unexplained animation constant for material meaning or
advertise a numerical fixture as production styling.

Reduced-motion spring sampling validates the input and returns the immediate
endpoint. This is the implemented refinement of the blueprint's qualitative
short-response guidance. Immediate pigment, edge and focus feedback remains;
feedback does not depend on overshoot or deformation.

Visual motion cannot move the interactive target under the user. The
[Command](../spec/49-command-snapshot.md), [Toggle](../spec/55-toggle-snapshot.md)
and [Slider](../spec/77-slider-snapshot.md) snapshots check coherent current paint,
layout, targets and semantic state. Direct manipulation and cancellation follow
their interaction contracts, not a decorative trajectory.

## Components, density and adaptation

[Component anatomy](../spec/78-component-anatomy.md) binds Command, Toggle and
Slider parts to semantic roles; themes change role values rather than
rebinding parts.
Command has explicit standard and primary emphasis. Toggle uses the selection
role for its checked track: forcing the authored primary accent would fail its
thumb-edge contrast guard. That is a readability refinement, not missing accent
styling. Disabled labels and component identification remain guarded.

Slider has value, interaction, layout, paint and snapshot contracts, plus
[track-segment allocation](../spec/79-slider-track-segments.md). The Slider track
and thumb are bound to neutral roles in this version; segment roles are
undecided
([#301](https://github.com/marcoallegretti/Resina-Design_System/issues/301)).
Shared production proportions belong to the
[component-profile proposal](https://github.com/marcoallegretti/Resina-Design_System/issues/134).
Diagnostic component dimensions are not accepted defaults. Tri-state
[Checkbox](https://github.com/marcoallegretti/Resina-Design_System/issues/136) and
single-selection [Segmented Control](https://github.com/marcoallegretti/Resina-Design_System/issues/137)
have separate semantic decisions; adjacent commands or a binary Toggle cannot
silently substitute for them.

[Density](../spec/17-density.md) is independent of device categories. The
[environment](../spec/18-environment.md) expresses geometry, input capabilities,
viewing, preferences and direction. It does not define phone or desktop modes.
Components own adaptation and retain minimum targets, readable content and clear
focus under narrower space, mixed input and increased text size.

## Accessibility and fallback preserve identity

Optional effects enhance material identity. They do not supply its only cue.
[Explicit capabilities](../spec/30-capabilities.md), quality policy and
accessibility preferences select the actual representation; renderer names and
descriptive tier numbers do not authorize effects.

Opaque Frost remains Frost. Reduced transparency or high contrast does not
replace its family with Cast. Focus remains identifiable without animation.
[Actual content and edge contrast](../spec/28-surface-readability.md),
[focus contrast](../spec/27-focus-indicator.md), measured text and
[minimum targets](../spec/15-targets.md) take priority over appearance. The theme
legibility guard alone does not prove readability over a rendered material or backdrop.

## Find the owning contract

| Appearance channel | Implemented boundary and evidence | Remaining contract or acceptance work |
| --- | --- | --- |
| Material and treatment | [Families](../spec/05-materials.md), [role vectors](../conformance/materials/role-assignment-vectors.json), [treatment stacks](../spec/06-optics.md) | [Frost paint](https://github.com/marcoallegretti/Resina-Design_System/issues/130), [optical realization](https://github.com/marcoallegretti/Resina-Design_System/issues/131) |
| Geometry and depth | [Opaque IR](../spec/36-opaque-surface-ir.md), [surface vectors](../conformance/ir/opaque-surface-cases.json), candidate [outlines](../spec/81-surface-outline.md) and [relief](../spec/82-surface-relief.md) with [face paint](../spec/83-opaque-relief-paint.md) and [focus geometry](../spec/84-outline-focus-geometry.md) | Acceptance of the candidate outline and relief contracts in [material identity](https://github.com/marcoallegretti/Resina-Design_System/issues/125) |
| Light and pigment | [Key light](../spec/34-key-light.md), [light vectors](../conformance/lighting/key-light-vectors.json), [authored themes](../spec/23-authored-themes.md), [seed derivation](../spec/91-theme-derivation.md), [environmental pigment](../spec/95-environmental-pigment.md) | [Material light response](https://github.com/marcoallegretti/Resina-Design_System/issues/129), [Environmental pigment rendering evidence](https://github.com/marcoallegretti/Resina-Design_System/issues/138) |
| Motion | [Trajectories](../spec/44-spring-trajectories.md), [trajectory cases](../conformance/motion/spring-trajectory-cases.json), sampled component paint, candidate [response calibration](../spec/80-material-response-calibration.md) | Acceptance of [bounded deformation and emergence](../spec/96-surface-deformation.md) in a backend ([#132](https://github.com/marcoallegretti/Resina-Design_System/issues/132)) |
| Anatomy, layout and hierarchy | [Role bindings](../spec/78-component-anatomy.md), measured labels and checked snapshots | [Profiles](https://github.com/marcoallegretti/Resina-Design_System/issues/134), [icons](https://github.com/marcoallegretti/Resina-Design_System/issues/135), distinct selection components |
| Visual proof and presentation | [Shared scenes](../conformance/scenes/README.md), [raster evidence](../conformance/raster/README.md), [native static Material Board](../lab/guido/README.md), [visual acceptance evidence](../conformance/scenes/visual-acceptance.md) | [Interactive UI Kit](https://github.com/marcoallegretti/Resina-Design_System/issues/141) |

The static material catalog compares four families in Light/Dark and opaque
rest/focus. Its scene checker verifies resolved meaning, geometry, contrast and
capture containment; CPU raster and native GPU tests verify their documented
paint boundaries. These are reproducible examples of current expression, not
accepted component proportions, calibrated motion, optical rendering, native
interaction or assistive-technology qualification. Follow the linked scene and
Material Board instructions to reproduce that bounded evidence.

## Documentation and implementation ownership

This guide stays in the existing documentation structure. A separate `language/`
directory adds no missing ownership boundary: design guidance links to contracts
instead of defining another token, profile or specification authority. A future
documentation reorganization should identify a concrete unmet reader need and
remain informative.

The Rust reference demonstrates contracts. [GUIdo](../backends/guido/resina-guido/README.md)
is the high-fidelity reference renderer; [Quickshell](../backends/quickshell/resina-qml/README.md),
[Slint](../backends/slint/resina-slint/README.md) and
[Web](../backends/web/resina-svg/README.md) provide independent, bounded mappings.
Their READMEs describe actual support; a rendered specimen does not certify a
complete component. The Lab presents shared results and diagnostics rather than
defining a second design system. Product layout, services and artwork remain
outside Resina Core.

Follow [delivery tracking](development/delivery.md) for integration and acceptance
status. Approval of a new semantic contract, acceptance of visual styling and
release-stage progression remain explicit decisions.
