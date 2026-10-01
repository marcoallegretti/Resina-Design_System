# Resina Design System
## Complete Specification, Architecture & Development Blueprint

**Project:** Resina Design System  
**Status:** Foundation / pre-1.0  
**Primary implementation language:** Rust  
**Normative dependency:** DTCG Design Tokens Format 2025.10  
**Reference rendering backend:** GUIdo  
**Independent Shell conformance backend:** Quickshell  
**Application backend:** Slint  
**Documentation / universal backend:** Web  
**Primary consumer:** Canopia  
**Primary target:** Linux, without Linux-specific semantics in Resina Core  

---

# 0. Mission

Resina is an independent, material-responsive, adaptive design system.

It defines how interfaces:

- look;
- communicate hierarchy;
- react to interaction;
- move;
- adapt to available space;
- adapt to input capabilities;
- adapt to accessibility preferences;
- degrade across renderer capabilities;
- communicate meaning;
- remain recognizable across different UI toolkits.

Resina is inspired by synthetic materials such as cured resin, elastomers and gels.

Its defining relationship is:

> **Persistence is expressed through curing.**

> **Interaction is expressed through elasticity.**

> **Transience is expressed through viscosity.**

Resina rejects flat design as its primary language.

It does not reject simplicity.

A Resina interface should feel:

- dimensional;
- tactile;
- coherent;
- mature;
- responsive;
- expressive;
- calm when idle.

It must never become:

- visually noisy;
- permanently animated;
- excessively translucent;
- toy-like;
- dependent on one expensive GPU effect.

---

# 1. Fundamental architectural rule

The most important sentence in this document is:

> **Resina is a specification, not a toolkit.**

No programming language, UI toolkit, rendering API, compositor or operating system defines Resina.

The following relationship is mandatory:

```text
Specification
    ≠
Reference implementation
    ≠
Rendering backend
    ≠
Platform integration
    ≠
Product
```

Therefore:

```text
Resina ≠ Rust
Resina ≠ GUIdo
Resina ≠ Quickshell
Resina ≠ Slint
Resina ≠ Web
Resina ≠ Wayland
Resina ≠ Canopia
```

---

# 2. System architecture

```text
                 ┌────────────────────────────┐
                 │    RESINA SPECIFICATION    │
                 │                            │
                 │ Philosophy                 │
                 │ Tokens                     │
                 │ Materials                  │
                 │ Color                      │
                 │ Geometry                   │
                 │ Motion                     │
                 │ Interaction                │
                 │ Components                 │
                 │ Adaptation                 │
                 │ Accessibility              │
                 │ Content                    │
                 │ Conformance                │
                 └──────────────┬─────────────┘
                                │
             ┌──────────────────┴──────────────────┐
             │                                     │
             ▼                                     ▼
      Human-readable                       Machine-readable
          specification                         model
             │                                     │
             └──────────────────┬──────────────────┘
                                ▼
                  ┌─────────────────────────┐
                  │ REFERENCE ENGINE — Rust │
                  │                         │
                  │ Parser                  │
                  │ Validator               │
                  │ Tokens                  │
                  │ Color                   │
                  │ Environment             │
                  │ Resolver                │
                  └─────────────┬───────────┘
                                │
                                ▼
                        ┌───────────────┐
                        │   RESINA IR   │
                        └───────┬───────┘
                                │
       ┌────────────────┬───────┼───────────┬────────────────┐
       ▼                ▼       ▼           ▼                ▼
    Headless          GUIdo  Quickshell    Slint             Web
       │                │       │           │                │
 Conformance       Reference Independent Application      Docs /
                  Rendering  Conformance   Backend       Playground
```

---

# 3. Backend classes

Backends are not all equal.

They exist for different purposes.

## Class A — Reference / Normative tooling

```text
resina-reference-rs
resina-headless
```

Purpose:

- implement normative algorithms;
- validate definitions;
- resolve scenarios;
- produce Resina IR;
- run conformance tests.

These are essential.

---

## Class B — Reference Rendering Backend

```text
resina-guido
```

Purpose:

> Demonstrate the highest-fidelity implementation of Resina.

GUIdo is allowed to use advanced capabilities such as:

- SDF geometry;
- GPU shaders;
- deformation;
- dynamic material lighting;
- backdrop effects;
- sophisticated motion;
- advanced masking.

GUIdo does not define the minimum capabilities of Resina.

---

# 4. Independent Shell Conformance Backend

```text
resina-quickshell
```

Purpose:

> Detect accidental architectural coupling between Resina and GUIdo.

It should implement enough of the core design system to prove that Resina semantics survive:

```text
Rust/wgpu/SDF
        versus
QtQuick/QML/Qt Scene Graph
```

Quickshell does not need to reproduce every Tier 3 rendering effect.

It must preserve semantics.

---

# 5. Application Backend

```text
resina-slint
```

Purpose:

> Prove that Resina also works for conventional application interfaces rather than only Shell UI.

Typical usage:

- Settings;
- utilities;
- system applications;
- installers;
- configuration tools.

---

# 6. Web Backend

```text
resina-web
```

Purpose:

- documentation;
- playground;
- interactive design-system reference;
- visual scenarios;
- portable conformance demonstrations;
- documentation site itself.

It may use:

```text
HTML
CSS
SVG
Web Animations
Canvas
WebGPU optionally
```

Advanced GPU rendering is optional.

A basic CSS/SVG implementation must remain useful.

---

# 7. Experimental and community backends

Not maintained as official Resina release targets initially:

```text
Flutter
GTK
Jetpack Compose
Qt Widgets
SwiftUI
React Native
other frameworks
```

These may become:

> **Resina Compatible Backends**

through the conformance program.

Flutter is therefore a future candidate, not part of the initial implementation burden.

---

# 8. Architectural laws

## R1 — Specification authority

No normative behaviour may exist only in Rust code.

Anything necessary to reimplement Resina MUST be described in the specification.

Where appropriate it MUST also have:

- schema;
- machine-readable definition;
- test vectors.

---

## R2 — Reimplementability

A developer should be able to implement Resina in another language using only:

```text
spec/
definitions/
schemas/
tokens/
conformance/
```

They MUST NOT need to reverse-engineer Rust.

---

## R3 — Toolkit independence

Normative Resina MUST NOT contain types from:

```text
GUIdo
Qt
Slint
Flutter
GTK
Web APIs
```

---

## R4 — Platform independence

Core Resina MUST NOT contain:

```text
wl_surface
layer-shell
exclusive-zone
X11 window IDs
Android Activity
Win32 HWND
Cocoa NSView
```

---

## R5 — Capability-based rendering

Forbidden:

```text
if backend == Guido
```

Correct:

```text
if capability.backdrop_blur
```

---

## R6 — Renderer independence

Shaders are implementations.

They are not material definitions.

---

## R7 — Product independence

If a feature only makes sense for Canopia, it probably belongs to Canopia rather than Core Resina.

---

# 9. Normative terminology

Normative documents use:

```text
MUST
MUST NOT
SHOULD
SHOULD NOT
MAY
```

Informative documents MUST clearly indicate when guidance is not normative.

---

# 10. Repository structure

Recommended structure:

```text
resina-design-system/
│
├── README.md
├── CONTRIBUTING.md
├── GOVERNANCE.md
├── CHANGELOG.md
├── SECURITY.md
│
├── LICENSES/
│
├── spec/
│   ├── 00-introduction.md
│   ├── 01-architecture.md
│   ├── 02-principles.md
│   ├── 03-tokens.md
│   ├── 04-color.md
│   ├── 05-materials.md
│   ├── 06-optics.md
│   ├── 07-light.md
│   ├── 08-elevation.md
│   ├── 09-geometry.md
│   ├── 10-spatial.md
│   ├── 11-typography.md
│   ├── 12-iconography.md
│   ├── 13-motion.md
│   ├── 14-interaction.md
│   ├── 15-state-composition.md
│   ├── 16-input.md
│   ├── 17-density.md
│   ├── 18-environment.md
│   ├── 19-adaptation.md
│   ├── 20-accessibility.md
│   ├── 21-localization.md
│   ├── 22-content-design.md
│   ├── 23-components.md
│   ├── 24-navigation.md
│   ├── 25-feedback.md
│   ├── 26-forms.md
│   ├── 27-shell-profile.md
│   ├── 28-couch-profile.md
│   ├── 29-ir.md
│   ├── 30-capabilities.md
│   ├── 31-fallback.md
│   ├── 32-conformance.md
│   ├── 33-versioning.md
│   └── 34-governance.md
│
├── tokens/
│   ├── foundation/
│   ├── semantic/
│   ├── themes/
│   └── experimental/
│
├── definitions/
│   ├── materials/
│   ├── treatments/
│   ├── motion/
│   ├── shapes/
│   ├── components/
│   ├── environments/
│   ├── density/
│   └── themes/
│
├── schemas/
│   ├── material.schema.json
│   ├── treatment.schema.json
│   ├── component.schema.json
│   ├── environment.schema.json
│   ├── capability.schema.json
│   ├── motion.schema.json
│   ├── scenario.schema.json
│   └── bundle.schema.json
│
├── reference/
│   └── rust/
│       ├── resina-model/
│       ├── resina-validate/
│       ├── resina-tokens/
│       ├── resina-color/
│       ├── resina-material/
│       ├── resina-motion/
│       ├── resina-environment/
│       ├── resina-resolver/
│       ├── resina-ir/
│       └── resina-ffi/
│
├── backends/
│   ├── headless/
│   ├── guido/
│   ├── quickshell/
│   ├── slint/
│   └── web/
│
├── conformance/
│   ├── vectors/
│   ├── scenarios/
│   ├── visual/
│   ├── behavioral/
│   ├── accessibility/
│   └── compatibility/
│
├── lab/
│   ├── guido/
│   └── web/
│
├── tooling/
│   ├── resina-cli/
│   ├── resina-compiler/
│   ├── resina-inspect/
│   ├── resina-codegen/
│   └── resina-docgen/
│
├── docs/
│   ├── examples/
│   ├── tutorials/
│   └── backend-authoring/
│
├── adr/
│
└── rfcs/
```

---

# 11. Standards strategy

Resina SHOULD use stable external standards where they fit.

## Normative external dependency

Use:

```text
DTCG Design Tokens Format 2025.10
```

for generic design tokens.

Examples:

- colour;
- dimensions;
- typography;
- duration;
- gradient;
- shadow;
- stroke.

Do not reinvent DTCG token syntax.

---

# 12. Resolver strategy

The current experimental DTCG Resolver draft is not normative for Resina.

Resina therefore owns its own resolution model for:

```text
theme
environment
accessibility
capabilities
quality
component state
```

The architecture SHOULD remain capable of adopting or mapping to future stable DTCG resolver standards.

---

# 13. Source-of-truth hierarchy

## Normative

```text
spec/
tokens/
definitions/
schemas/
conformance/
```

## Canonical implementation

```text
reference/rust/
```

## Implementation

```text
backends/
```

## Tooling

```text
tooling/
lab/
```

Generated artifacts are never authoritative.

---

# 14. Design-token architecture

Separate token layers.

## Foundation tokens

Raw scales:

```text
color.blue.500
space.4
radius.3
font.size.4
duration.2
```

Components SHOULD rarely consume these directly.

---

## Semantic tokens

Describe meaning:

```text
color.surface.base
color.content.primary
color.accent.primary

space.control.inline
space.panel.outer

shape.control
shape.container

motion.duration.short
```

---

## Component tokens

Use sparingly.

Example:

```text
button.padding.inline
button.icon.gap
```

Component tokens should usually reference semantic tokens.

---

# 15. Token aliasing

Prefer semantic references.

Bad:

```text
button.background = #5D75FF
```

Good:

```text
button.background = {color.control.primary.surface}
```

---

# 16. Resina Bundle

Authoring sources compile into a normalized bundle.

```text
ResinaBundle
├── metadata
├── tokens
├── themes
├── materials
├── treatments
├── motion
├── geometry
├── components
├── adaptation
├── accessibility
└── content guidance references
```

Required metadata:

```text
specVersion
schemaVersion
bundleVersion
tokenFormatVersion
```

---

# 17. Philosophy

Resina uses material behaviour as visual language.

The fundamental continuum is:

```text
Structural persistence
        ↓
      curing

Direct interaction
        ↓
    elasticity

Transient interaction
        ↓
     viscosity
```

The material metaphor provides consistency.

It is not a literal physics simulation.

---

# 18. No-flat principle

A meaningful surface MUST communicate presence using one or more of:

```text
edge
thickness
lighting
curvature
inset
elevation
pigment variation
material response
```

However, the result SHOULD remain visually restrained.

---

# 19. Material taxonomy

Resina begins with four primary material families.

---

## 19.1 Cast

Fully cured synthetic resin.

Characteristics:

```text
very high stiffness
very low deformation
controlled roughness
stable edges
high structural confidence
```

Typical roles:

```text
content backgrounds
persistent containers
settings surfaces
cards
structural layouts
```

---

## 19.2 Frost

Cured translucent/diffusive resin.

Characteristics:

```text
partial transmission
internal scattering
pigmented body
high stability
low elasticity
environmental response
```

Typical roles:

```text
panels
navigation chrome
docks
sidebars
launcher containers
floating structural chrome
```

Frost MUST remain recognizable without backdrop blur.

---

## 19.3 Elastomer

Solid silicone-like synthetic material.

Characteristics:

```text
high elasticity
moderate stiffness
strong compression response
quick recovery
tactile appearance
```

Typical roles:

```text
buttons
toggles
slider thumbs
interactive chips
touch targets
controller-focused actions
```

---

## 19.4 Gel

Soft and viscous synthetic material.

Characteristics:

```text
high deformation
higher inertia
high damping
shape morphing
slower relaxation
```

Typical roles:

```text
drag feedback
selection
temporary affordances
expressive transient UI
```

Gel MUST NOT become default structural UI.

---

# 20. Material dimensions

A material definition contains:

```text
optics
geometry
pigment
illumination
mechanics
interaction
fallback
```

---

# 21. Optical properties

Possible normalized parameters:

```text
transmission
scatter
roughness
absorption
internalBrightness
backgroundInfluence
```

Values MUST have documented perceptual meaning.

---

# 22. Geometry properties

Possible material geometry parameters:

```text
thickness
edgeDepth
bevel
curvatureBias
surfaceTension
```

These MUST remain renderer-independent.

---

# 23. Mechanical properties

```text
stiffness
elasticity
viscosity
damping
inertia
```

These represent interface behaviour, not laboratory measurements.

---

# 24. Optical treatments

Treatments operate independently of material family.

Initial treatments:

```text
None
Lens
FocusLens
HighlightLens
```

Example:

```text
Frost + Lens
Elastomer + FocusLens
Cast + HighlightLens
```

---

# 25. Material roles

Components SHOULD request semantic material roles.

```text
surface.base
surface.content
surface.chrome
surface.raised
surface.transient

control.passive
control.interactive
control.primary

feedback.focus
feedback.selection
feedback.drag
```

Themes resolve roles into material families.

---

# 26. Composition rules

Initial rules:

```text
Cast over Cast
    allowed

Elastomer over Cast
    encouraged

Elastomer over Frost
    allowed

Gel over stable material
    selective

Frost nested repeatedly
    discouraged

Gel inside Gel
    discouraged

Lens over Lens
    invalid
```

---

# 27. Lighting system

Resina defines a coherent lighting environment:

```text
LightEnvironment
├── Key
├── Ambient
└── Interaction
```

---

# 28. Key light

Provides stable dimensional direction.

Components in the same visual context SHOULD appear physically consistent.

---

# 29. Ambient light

May react to:

```text
theme
wallpaper
surrounding surfaces
environment
```

Ambient influence MUST NOT destroy readability.

---

# 30. Interaction light

May appear during:

```text
hover
touch contact
focus
drag
press
```

Interaction lighting MUST settle after the interaction ends.

---

# 31. Elevation system

Semantic elevation:

```text
embedded
base
raised
floating
overlay
modal
```

Elevation may influence:

```text
shadow
edge
highlight
thickness
background separation
```

It is not synonymous with `box-shadow`.

---

# 32. Spatial system

Resina needs a formal spatial scale.

Recommended conceptual foundation:

```text
space.0
space.1
space.2
space.3
space.4
space.5
space.6
space.7
space.8
```

Do not finalize numerical values until component prototyping.

The scale MUST support:

- dense desktop UI;
- touch;
- couch;
- increased text scale.

---

# 33. Spatial semantic roles

Examples:

```text
space.control.inline
space.control.block

space.container.inner
space.container.outer

space.section
space.group
space.page

space.touch.minimum
```

---

# 34. Minimum target policy

Resina component contracts define semantic minimum targets.

Separate:

```text
visual size
```

from:

```text
interactive hit region
```

A visually small control MAY have a larger touch hit target.

---

# 35. Safe areas

Layouts MUST support safe-area insets.

Applicable to:

- handheld displays;
- rounded screens;
- display cut-outs;
- software navigation;
- television overscan-like constraints.

No specific OS API belongs in the Resina specification.

---

# 36. Shape system

Canonical semantic shapes:

```text
Structural
Soft
Rounded
Capsule
Organic
```

These are intents rather than fixed radii.

The reference model may define normalized curvature descriptors.

---

# 37. Superellipse language

Resina SHOULD favour continuous/superellipse-like curvature for major surfaces.

Renderer implementations MAY approximate it when exact geometry is unavailable.

Approximation MUST preserve the intended visual softness.

---

# 38. Shape hierarchy

General tendency:

```text
large structural surface
    → calmer curvature

small interactive control
    → softer curvature

highly tactile transient element
    → potentially organic curvature
```

Avoid universal pill UI.

---

# 39. Color system

Resina separates:

```text
palette generation
semantic colour
material pigmentation
```

---

# 40. Semantic colour families

Initial semantic system:

```text
accent.primary
accent.secondary
accent.tertiary

surface.base
surface.low
surface.high
surface.chrome

content.primary
content.secondary
content.muted
content.inverse

outline
outline.strong

status.error
status.warning
status.success
status.info

focus
selection
```

---

# 41. Dynamic colour

Theme sources MAY include:

```text
wallpaper
user seed
application seed
system seed
fixed theme
```

Pipeline:

```text
Source
 ↓
Candidate extraction
 ↓
Quantization
 ↓
Scoring
 ↓
Seed
 ↓
Palette generation
 ↓
Semantic assignment
 ↓
Material pigment derivation
```

---

# 42. Color-space strategy

Use perceptual spaces where appropriate.

Potential architecture:

```text
HCT-like strategy
    for seed → tonal palette

OKLab / OKLCH
    for interpolation and animation
```

The exact algorithm MUST become a documented ADR and conformance-tested implementation.

Do not leave dynamic colour behaviour as an implementation accident.

---

# 43. Material pigment

Materials derive several values from one semantic colour.

Example:

```text
accent.primary
    ↓
Elastomer
    ↓
body
edge
highlight
shadow affinity
ambient affinity
```

---

# 44. Legibility Guard

A dynamic-material interface cannot rely solely on predefined static contrast pairs.

The resolver SHOULD support a Legibility Guard.

Potential corrections:

```text
increase opacity
reduce environmental influence
adjust foreground
strengthen edge
change pigment tone
reduce treatment
add content backing
```

Readability has priority.

---

# 45. Typography

Typography provides calm structure against expressive materials.

Resina SHOULD NOT visually deform normal text.

No:

```text
jelly body text
refraction through text
moving highlight inside labels
```

---

# 46. Typography roles

Recommended semantic hierarchy:

```text
Display
Title Large
Title
Heading
Body Large
Body
Label
Caption
Numeric
Code
```

---

# 47. Typography specification

Each role defines:

```text
font family role
size
weight
line height
tracking
minimum scale behaviour
```

Do not hardcode one physical font into the semantic system.

---

# 48. Font-family roles

Potential roles:

```text
sans
display
monospace
numeric
```

Theme/platform backend maps roles to actual available fonts.

---

# 49. Text scaling

Resina MUST support increased text scale.

Components MUST define:

```text
wrap
grow
reflow
truncate only when necessary
```

Do not respond to larger text by silently shrinking text.

---

# 50. Numeric typography

Numeric surfaces may use:

```text
tabular figures
monospaced digits
```

where useful.

Examples:

- timers;
- percentages;
- media duration;
- clocks;
- performance displays.

---

# 51. Iconography

Resina requires a formal symbolic icon system.

It should define:

```text
canonical grid
optical sizing
stroke/fill policy
semantic size roles
state behaviour
directional behaviour
RTL behaviour
badge behaviour
```

---

# 52. Icon size roles

Conceptually:

```text
icon.compact
icon.control
icon.navigation
icon.large
icon.hero
```

Exact dimensions belong to tokens.

---

# 53. Symbolic vs full-color

Use symbolic icons for most interface controls.

Use full-color icons for:

- applications;
- devices where identity matters;
- rich media;
- categories where colour is semantically useful.

---

# 54. Directional icons

Icons with directionality MUST declare whether they mirror under RTL.

Examples that often mirror:

```text
back
forward
previous
next
```

Examples that generally do not:

```text
play
volume
clock
download
```

This behaviour belongs to icon metadata.

---

# 55. Motion philosophy

Motion expresses material and continuity.

Do not design motion as:

```text
all transitions = 200 ms ease-out
```

Material personality determines motion.

---

# 56. Spring model

Physically inspired model:

```text
mass
stiffness
damping
initial velocity
settling threshold
```

Normative algorithms MUST define:

- equation;
- normalization;
- initial state;
- stopping criterion;
- acceptable numerical tolerance.

---

# 57. Material motion

Baseline:

| Material | Stability | Compression | Overshoot | Settling |
|---|---:|---:|---:|---:|
| Cast | Very high | Minimal | None | Immediate |
| Frost | High | Very low | Minimal | Fast |
| Elastomer | Medium | High | Controlled | Fast |
| Gel | Low | High | Restrained | Slow |

Values are qualitative until calibrated.

---

# 58. Spatial motion

Not every movement is material deformation.

Resina distinguishes:

```text
Material Motion
Spatial Motion
Navigation Motion
State Transition
```

Example:

- pressing a button → material;
- opening a drawer → spatial;
- switching page → navigation;
- disabling control → state.

---

# 59. Interaction state model

Canonical states:

```text
rest
hover
focused
pressed
active
selected
checked
disabled
busy
dragging
error
warning
success
```

Not every component supports every state.

---

# 60. State composition

States may coexist.

Example:

```text
focused + selected
hovered + error
focused + checked
pressed + selected
```

Resina MUST define composition rules rather than assuming one state replaces all others.

---

# 61. State priority

Recommended conceptual layers:

```text
Availability
    disabled / enabled

Validation
    error / warning / success

Selection
    selected / checked / active

Interaction
    pressed / dragging / hover

Navigation
    focused

Base
    rest
```

Visual state is composed across these layers.

This is preferable to one simplistic priority list.

---

# 62. Example state composition

```text
Button
enabled
selected
focused
hovered
```

may resolve as:

```text
selected pigment
+
focus edge/treatment
+
hover interaction light
```

instead of forcing one state to completely replace the others.

---

# 63. Input modalities

Supported abstract capabilities:

```text
fine pointer
coarse pointer
hover
direct touch
stylus
keyboard
directional navigation
gamepad
voice/action input where applicable
```

Multiple capabilities MAY coexist.

---

# 64. Pointer interaction

Fine pointer:

- precise;
- smaller hover region acceptable;
- subtle deformation;
- subtle interaction light.

---

# 65. Touch interaction

Direct touch:

- larger targets;
- stronger compression;
- no hover dependency;
- spacing adapted to coarse input.

---

# 66. Stylus

Stylus:

- precise;
- minimal coarse-target inflation;
- potentially pressure-aware in future;
- should not inherit heavy finger deformation automatically.

---

# 67. Keyboard

Keyboard interaction requires:

- deterministic focus;
- visible focus;
- predictable tab order;
- shortcuts where appropriate;
- no pointer dependency.

---

# 68. Directional navigation

Required for:

- gamepad;
- TV/couch;
- accessibility;
- remote-like input.

Spatial focus MUST be deterministic.

---

# 69. Density system

Resina formalizes information density separately from device type.

Initial modes:

```text
Compact
Standard
Comfortable
Immersive
```

---

# 70. Compact density

Appropriate for:

- precise pointer;
- dense information;
- desktop workflows.

May reduce:

```text
spacing
row height
control height
label redundancy
```

without compromising accessibility minimums.

---

# 71. Comfortable density

Appropriate for:

- mixed input;
- touch-capable laptops;
- general-purpose relaxed UI.

---

# 72. Immersive density

Appropriate for:

- couch;
- gamepad;
- far viewing;
- high-focus interfaces.

It may increase:

```text
target size
text size
spacing
focus strength
```

and reduce simultaneous information.

---

# 73. Environment Model

Resina uses capabilities and geometry rather than device categories.

```text
EnvironmentSnapshot
{
    geometry
    scale
    textScale

    inputCapabilities

    viewingProfile
    densityPreference

    accessibilityPreferences

    locale
    layoutDirection

    rendererCapabilities
    qualityPolicy
}
```

---

# 74. Geometry

Include:

```text
width
height
aspect ratio
orientation
safe area
```

---

# 75. Viewing profile

Possible profiles:

```text
Near
Desk
Couch
Unknown
```

Do not infer Couch only because a gamepad exists.

---

# 76. No device modes

Core Resina MUST NOT expose:

```text
DesktopMode
PhoneMode
TabletMode
GameMode
```

Products MAY expose experience presets.

Resina sees the resulting environment.

---

# 77. Adaptation ownership

Components own most adaptive behaviour.

Do not use one global breakpoint that transforms the entire UI.

---

# 78. Adaptive representations

Components MAY define:

```text
compact
standard
expanded
immersive
```

Example:

```text
Volume
    compact → icon
    standard → icon + percentage
    expanded → slider + device
    immersive → controller-friendly large control
```

---

# 79. Breakpoints

Breakpoints describe available space.

Good:

```text
availableInlineSize < threshold
```

Bad:

```text
isMobile == true
```

---

# 80. Hysteresis

Automatic adaptation SHOULD use hysteresis where continuous resizing could cause layout thrashing.

---

# 81. Localization

Localization support is normative.

Components MUST be designed for:

```text
long translated strings
RTL
CJK
Arabic
Indic scripts
variable word length
plural forms
localized dates
localized numbers
```

---

# 82. RTL

Components must distinguish:

```text
physical direction
logical direction
```

Prefer:

```text
start / end
```

over:

```text
left / right
```

for layout semantics where direction should mirror.

---

# 83. Text expansion

Reference scenarios SHOULD test approximately:

```text
100%
150%
200%
```

string-length expansion.

Controls should not assume English label length.

---

# 84. Localization-safe anatomy

Bad component anatomy:

```text
fixed-width text slot
```

Preferred:

```text
content-sized label
with explicit max/reflow policy
```

---

# 85. Accessibility

Accessibility is part of component definition.

Not backend polish.

Every interactive component defines:

```text
role
name
description
value
state
actions
focusability
relationships
```

---

# 86. Reduced motion

Transform:

```text
large deformation
long spring
overshoot
morphing
```

into:

```text
short response
small deformation
critically damped transition
pigment/edge change
```

Do not remove all feedback.

---

# 87. Reduced transparency

Frost becomes:

```text
opaque / near-opaque dimensional Frost representation
```

not a generic flat rectangle.

---

# 88. High contrast

May strengthen:

```text
foreground separation
edge
focus
outline
material opacity
```

and reduce ambiguous shadows.

---

# 89. Color independence

Meaning MUST NOT depend exclusively on hue.

Examples:

```text
error
success
focus
selection
```

should combine colour with:

- icon;
- shape;
- outline;
- label;
- position;
- material treatment.

---

# 90. Focus

Focus is a first-class semantic state.

Possible visual responses:

```text
edge strengthening
elevation
FocusLens
controlled inflation
pigment change
highlight
```

Focus MUST remain obvious without animation.

---

# 91. Content design

Resina defines how interface language communicates.

It does not only define visuals.

---

# 92. Content principles

Interface copy SHOULD be:

- concise;
- direct;
- specific;
- action-oriented;
- non-technical unless the context is technical;
- non-judgmental.

---

# 93. Button labels

Prefer action verbs.

Good:

```text
Save
Connect
Remove
Restart
Try again
```

Avoid:

```text
OK
Proceed
Do it
```

when a more specific action exists.

---

# 94. Error language

Errors SHOULD explain:

```text
what failed
what remains safe
what the user can do
```

Bad:

```text
Error 0x35
```

Better:

```text
Bluetooth couldn't connect to the device.
Try again or remove the device and pair it again.
```

Technical detail MAY be available through expandable diagnostics.

---

# 95. Destructive actions

Use explicit verbs:

```text
Delete file
Remove account
Forget network
```

rather than generic:

```text
Confirm
Yes
```

---

# 96. Progress language

Prefer meaningful progress:

```text
Installing…
Connecting…
Restarting…
```

Avoid inventing percentage values when actual progress is unknown.

---

# 97. Empty states

An empty state should:

1. explain the state;
2. optionally explain why;
3. offer a useful action where applicable.

---

# 98. Component model

A Resina component is a **contract**.

It defines:

```text
anatomy
variants
states
semantic roles
material roles
content rules
interaction
focus
adaptation
accessibility
localization
fallback
```

It does NOT define one specific widget implementation.

---

# 99. Core components

Foundation:

```text
Surface
Container
Divider
ScrollArea
FocusSurface
```

Actions:

```text
Button
IconButton
SplitButton
```

Selection:

```text
Checkbox
Radio
Toggle
SegmentedControl
Chip
```

Input:

```text
TextField
SearchField
Slider
RangeSlider
Stepper
ComboBox
```

---

# 100. Navigation components

```text
NavigationItem
NavigationGroup
NavigationRail
Tabs
Pager
Breadcrumb
```

Profiles may define additional navigation components.

---

# 101. Feedback components

```text
Progress
Spinner
Toast
Tooltip
Dialog
Popover
Banner
Notification
```

---

# 102. Data components

Later phases MAY include:

```text
List
Tree
Table
Grid
DataCard
```

Do not rush complex data components before fundamentals stabilize.

---

# 103. Shell Profile

Shell-specific design concepts belong in a profile rather than Core.

```text
Panel
Applet
TaskItem
WorkspaceItem
LauncherItem
SystemStatus
QuickSetting
Drawer
OSD
```

These remain platform-neutral.

---

# 104. Couch / Game Profile

Adds guidance for:

```text
directional focus
large selectors
GameTile
Carousel
ActionHint
ControllerPrompt
FocusGroup
```

It does not create a separate material system.

---

# 105. Resina IR

Resina Intermediate Representation is the resolved design intent exchanged between resolver and renderer.

It is similar conceptually to computed style.

It is NOT:

```text
scene graph
widget tree
GPU command buffer
layout engine
event system
```

---

# 106. Resolution pipeline

```text
Component Intent
       +
Variant
       +
State
       +
Theme
       +
Environment
       +
Accessibility
       +
Renderer Capabilities
       +
Quality Policy
       │
       ▼
  Resina Resolver
       │
       ▼
    Resina IR
       │
       ▼
Rendering Backend
```

---

# 107. Example Resina IR

```text
ResolvedButton
{
    representation: Standard

    surface:
        material: Elastomer
        treatment: None

    pigment:
        body
        edge
        highlight

    geometry:
        shape: Soft
        elevation: Raised

    mechanics:
        stiffness
        damping
        compression

    motion:
        profile: ElastomerPress

    content:
        foreground

    accessibility:
        role: Button
        enabled: true
}
```

No renderer-specific object exists here.

---

# 108. IR design rules

IR SHOULD be:

- serializable;
- stable enough for debugging;
- inspectable;
- language-neutral in concept;
- small enough not to become a GUI framework.

---

# 109. Determinism

Given identical:

```text
bundle
intent
state
theme
environment
capabilities
quality
```

the resolver SHOULD produce semantically equivalent IR.

---

# 110. Renderer capability model

A backend advertises capabilities.

Example:

```text
RendererCapabilities
{
    gradients
    innerShadow
    advancedShadow
    sdfShapes

    backdropEffect
    backdropBlur
    shapedBackdrop

    dynamicLighting
    deformation
    masks
    customShader

    wideGamut
    hdr
}
```

---

# 111. Capability levels

Capabilities MAY eventually use levels.

Example:

```text
deformation:
    none
    transform
    shape
    advanced
```

This is more expressive than boolean support.

---

# 112. Quality policy

Separate capabilities from requested quality.

```text
Economy
Balanced
Full
```

A renderer may technically support Tier 3 but choose lower-cost effects in Economy mode.

---

# 113. Rendering tiers

## Tier 0 — Identity

Required.

```text
pigment
shape
edge
hierarchy
basic depth
state
```

---

## Tier 1 — Dimensional

```text
gradients
inner/outer shadow
thickness
basic deformation
```

---

## Tier 2 — Material

```text
backdrop effects
ambient response
advanced geometry
dynamic highlights
richer motion
```

---

## Tier 3 — Physical

```text
material morphing
advanced deformation
complex masks
local interaction lighting
optical distortion
material merging
```

Tier classification is descriptive.

Capability resolution remains authoritative.

---

# 114. Fallback contract

Every advanced feature MUST have a valid fallback.

Example:

```text
Frost + shaped backdrop
        ↓
Frost + regular backdrop
        ↓
translucent pigmented Frost
        ↓
opaque dimensional Frost
```

---

# 115. Identity requirement

If all advanced effects are disabled:

> Resina MUST still look like Resina.

Identity must survive through:

```text
material hierarchy
shape
edge
pigment
elevation
spacing
typography
interaction state
```

---

# 116. Headless backend

The Headless backend is mandatory.

It:

- resolves scenarios;
- serializes IR;
- validates expected output;
- performs no rendering.

Example:

```bash
resina resolve button-primary-pressed.json
```

produces:

```json
{
    "material": "elastomer",
    "state": "pressed",
    "elevation": "raised"
}
```

---

# 117. GUIdo backend

GUIdo is the high-fidelity reference implementation.

It may use:

```text
wgpu
SDF
custom shaders
advanced springs
material deformation
backdrop capabilities
```

Mapping:

```text
Resina IR
   ↓
resina-guido
   ↓
GUIdo primitives
```

---

# 118. GUIdo upstream policy

Generic primitives should be contributed upstream.

Good candidates:

```text
inner shadow
generic deformation
SDF masking
directional focus
accessibility primitives
renderer instrumentation
generic effects
```

Resina semantics remain outside GUIdo.

---

# 119. Quickshell backend

Purpose:

> Catch GUIdo-specific assumptions.

It implements the same semantic subset using QtQuick/QML.

It does not need advanced visual parity.

Before a foundational concept becomes stable ask:

> Can Quickshell represent this without changing what it means?

---

# 120. Slint backend

Purpose:

> Verify conventional application use.

It prioritizes:

```text
components
forms
navigation
settings
dialogs
accessibility
```

rather than maximum optical effects.

---

# 121. Web backend

The Web backend powers:

```text
documentation
component gallery
playground
interactive token viewer
scenario browser
```

It should prefer broadly available web primitives first.

WebGPU enhancements are optional.

---

# 122. Resina Compatible program

Third-party implementations can declare compatibility.

Conformance is graded.

---

# 123. Level 1 — Semantic

Required:

```text
tokens
semantic colors
material roles
geometry
component states
content semantics
```

---

# 124. Level 2 — Behavioral

Adds:

```text
interaction
motion
focus
adaptation
reduced motion
```

---

# 125. Level 3 — Material

Adds:

```text
dimensional hierarchy
material-specific interaction
deformation
lighting response
```

---

# 126. Level 4 — Enhanced

Adds selected advanced capabilities:

```text
backdrop
advanced optical treatment
advanced deformation
dynamic lighting
```

A backend need not reach Level 4 to be genuinely Resina-compatible.

---

# 127. Conformance vectors

Normative vectors cover:

```text
tokens
color
material resolution
motion
state composition
environment
adaptation
accessibility
fallback
components
```

---

# 128. Scenario manifests

Visual and behavioural backends consume shared scenarios.

Example:

```text
scenario:
    button-primary-focused

theme:
    dark

environment:
    width: 1280
    finePointer: true
    keyboard: true

state:
    focused
```

---

# 129. Numerical tolerance

Cross-language implementations do not need bit-identical floating-point output.

Define tolerances for:

```text
color
motion
interpolation
geometry
```

---

# 130. Visual conformance

Visual conformance combines:

```text
geometry assertions
semantic assertions
reference scenes
perceptual image comparison
```

Do not rely solely on exact pixel matching.

---

# 131. Resina Lab

Resina Lab is mandatory.

The principal high-fidelity Lab SHOULD use GUIdo.

The Web implementation provides an additional portable Lab.

---

# 132. Lab controls

Expose:

```text
theme
accent

material
treatment
material parameters

component
variant
state

viewport
orientation
density

input capabilities

accessibility

renderer capability simulation
quality
```

---

# 133. Material Board

Each material against:

```text
light background
dark background
complex wallpaper
high-chroma wallpaper
low-contrast wallpaper
```

and states:

```text
rest
hover
focus
press
drag
release
```

where applicable.

---

# 134. Component Board

Every normative component state must be visible.

State combinations must be inspectable.

---

# 135. Adaptation Board

Shared content under:

```text
narrow portrait
wide portrait
desktop pointer
mixed touch + pointer
large text
couch
```

These are test scenarios, not device classes.

---

# 136. Localization Board

Test:

```text
English
German-like expansion
RTL
CJK
200% text expansion
```

Use synthetic strings if translations are unavailable.

---

# 137. Accessibility Board

```text
high contrast
reduced motion
reduced transparency
large text
keyboard only
directional navigation
```

---

# 138. Inspector

Developer inspection should expose:

```text
intent
tokens
semantic role
material
pigment
state composition
environment
capabilities
fallback
IR
```

This is essential.

---

# 139. CLI

Potential commands:

```text
resina validate
resina compile
resina resolve
resina inspect
resina test
resina conformance
resina codegen
```

---

# 140. Rust reference crates

Recommended:

```text
resina-model
resina-validate
resina-tokens
resina-color
resina-material
resina-motion
resina-environment
resina-resolver
resina-ir
resina-ffi
```

Avoid one giant ambiguous `resina-core`.

---

# 141. resina-model

Contains platform-neutral types.

MUST NOT depend on:

```text
GUIdo
Qt
Slint
wgpu
Wayland
```

---

# 142. resina-validate

Validates:

```text
schemas
references
cycles
state definitions
fallback graphs
component contracts
```

Errors must be actionable.

---

# 143. resina-color

Implements canonical:

```text
colour conversion
palette generation
pigment derivation
legibility guard
```

Every normative algorithm gets vectors.

---

# 144. resina-material

Responsible for:

```text
material definitions
material roles
treatments
fallback
parameter normalization
```

It does not render them.

---

# 145. resina-motion

Responsible for:

```text
motion profiles
spring reference calculations
reduced-motion transforms
```

It is not an animation engine.

---

# 146. resina-environment

Defines normalized environment.

Platform code gathers raw information and translates it into Resina environment.

---

# 147. resina-resolver

Canonical decision engine.

Input:

```text
intent
state
theme
environment
capabilities
quality
```

Output:

```text
Resina IR
```

---

# 148. FFI

`resina-ffi` MAY expose a stable C ABI.

Useful for:

```text
Qt/C++
other native languages
```

This is convenience.

It is not a compatibility requirement.

---

# 149. Code generation

Potential generated targets:

```text
Rust
C++
TypeScript
Dart
```

Useful for:

```text
token constants
enums
schemas
component metadata
```

Generated output is never source of truth.

---

# 150. Documentation architecture

Documentation has three audiences.

## Designers

Need:

```text
principles
materials
color
motion
components
content
accessibility
```

## Backend implementers

Need:

```text
IR
capabilities
fallback
conformance
algorithms
```

## Product developers

Need:

```text
component APIs
semantic roles
themes
adaptation
examples
```

---

# 151. Web documentation

The web backend should power a live documentation site.

Each component page may show:

```text
anatomy
usage
states
do/don't
adaptive modes
accessibility
live playground
IR inspector
backend support matrix
```

---

# 152. Performance principles

Resina should settle when idle.

Rules:

```text
no permanent decorative loops
springs stop when settled
hidden effects stop
static material results may cache
large offscreen passes are bounded
```

---

# 153. Performance is backend-owned but spec-aware

Resina defines degradability.

Backend decides implementation optimization.

Performance characteristics MUST NOT silently change semantic meaning.

---

# 154. Security considerations

Resina itself is not a security framework.

However component specifications must avoid patterns that encourage insecure UX.

Examples:

- dangerous actions should be explicit;
- password fields must not expose contents through decorative effects;
- security state must not rely solely on colour;
- deceptive modal layering should be avoided.

Platform security belongs elsewhere.

---

# 155. Governance

Resina needs explicit maturity states.

```text
Experimental
Candidate
Stable
Deprecated
Removed
```

These may apply to:

```text
tokens
materials
components
IR fields
schemas
algorithms
```

---

# 156. Experimental

Can change freely.

Must not be required by stable backend compatibility.

---

# 157. Candidate

Semantics mostly defined.

Conformance vectors exist.

Breaking changes still possible before Stable.

---

# 158. Stable

Subject to compatibility policy.

Breaking semantic changes require major-version process.

---

# 159. Deprecated

Still supported for a documented transition period.

Replacement guidance required.

---

# 160. RFC process

Major additions SHOULD begin with an RFC.

Examples:

```text
new material family
new state dimension
major IR addition
new component family
new adaptation axis
```

RFC contains:

```text
problem
goals
non-goals
proposal
alternatives
compatibility
accessibility
performance
migration
```

---

# 161. ADR process

Architecture decisions record finalized technical choices.

Initial ADRs:

```text
ADR-0001 Specification-first architecture
ADR-0002 DTCG Format adoption
ADR-0003 Resina-specific resolver
ADR-0004 Material taxonomy
ADR-0005 Lens as treatment
ADR-0006 Resina IR boundary
ADR-0007 Dynamic colour model
ADR-0008 Motion model
ADR-0009 Environment Model
ADR-0010 Capability model
ADR-0011 Density system
ADR-0012 State composition
ADR-0013 GUIdo backend
ADR-0014 Quickshell backend
ADR-0015 Slint backend
ADR-0016 Web backend
ADR-0017 Compatible program
```

---

# 162. Versioning

Track separately:

```text
Resina Spec
Schema
Bundle
IR
Token package
Rust crates
Backend versions
```

Do not force them all to use one version number.

---

# 163. SemVer philosophy

Before 1.0:

breaking experimentation is allowed but documented.

After 1.0:

- major → semantic break;
- minor → backwards-compatible additions;
- patch → fixes/clarifications.

---

# 164. Schema migration

Persisted machine-readable definitions MUST identify schema version.

Migration tooling SHOULD exist for supported versions.

---

# 165. Backend compatibility matrix

Documentation should publish:

| Capability | GUIdo | Quickshell | Slint | Web |
|---|---:|---:|---:|---:|
| Semantic | ✓ | ✓ | ✓ | ✓ |
| Motion | ✓ | ✓ | ✓ | ✓ |
| Material deformation | ✓ | partial | partial | partial |
| Backdrop | platform | platform | optional | CSS-dependent |
| Advanced shader | ✓ | possible | renderer-dependent | optional |

Exact matrix evolves with implementation.

---

# 166. Development workflow

For every new feature:

```text
1. Identify design problem
2. Determine whether it belongs to Resina
3. Write intent
4. Define semantics
5. Update specification
6. Define machine model
7. Add schema
8. Add conformance vectors
9. Implement Rust resolver
10. Inspect IR
11. Implement GUIdo representation
12. Validate in Lab
13. Perform Quickshell plausibility test
14. Validate accessibility/localization
15. Stabilize
```

---

# 167. Backend Independence Gate

Before foundational semantics become Stable ask:

```text
Can GUIdo represent it?
Can Quickshell represent it?
Can Web approximate it?
Can Slint retain its meaning?
```

Not every backend requires implementation immediately.

But the concept must not inherently depend on one renderer.

---

# 168. Language Independence Gate

Ask:

> Could someone implement this from the specification in C++ without reading Rust?

If not:

the specification is incomplete.

---

# 169. Accessibility Gate

Before Stable status:

```text
keyboard
screen reader semantics
large text
high contrast
reduced motion
reduced transparency
```

must have defined behaviour.

---

# 170. Localization Gate

Before Stable status:

```text
RTL
text expansion
wrapping
long labels
logical direction
```

must be tested.

---

# 171. Identity Gate

Disable:

```text
blur
transparency
custom shaders
advanced deformation
```

If the result stops looking like Resina:

the feature depends too heavily on expensive rendering.

---

# 172. Anti-patterns

## Backend leakage

Bad:

```text
material.guidoBlur = 30
```

---

## Framework leakage

Bad:

```text
component.qmlImplicitWidth
```

---

## Platform leakage

Bad:

```text
Panel.exclusiveZone
```

in Core Resina.

---

## Material-as-shader

Bad:

```text
Frost = frost.wgsl
```

---

## Device assumptions

Bad:

```text
if phone
```

---

## Universal translucency

Not every surface should expose the background.

---

## Universal jelly

Not every interaction should wobble.

---

## Arbitrary physics vocabulary

Do not label unexplained animation constants "viscosity".

---

## Flat fallback

Disabling blur must not collapse the design to plain rectangles.

---

## Pixel-parity obsession

Different renderers may express the same material with different techniques.

---

## Backend explosion

Do not add official backend support merely to prove agnosticism.

---

## Spec by implementation

Do not document behaviour by saying:

> See the Rust source.

---

# 173. Phase 0 — Foundation

Create:

```text
spec/
tokens/
definitions/
schemas/
reference/
conformance/
adr/
```

Deliver:

- architecture document;
- repository laws;
- DTCG adoption;
- validation CI;
- initial schemas.

---

# 174. Phase 1 — Design foundations

Define:

```text
spacing
geometry
shape
elevation
semantic color
typography
density
states
```

Exit criterion:

> Foundation tokens resolve without GUI dependencies.

---

# 175. Phase 2 — Material system

Define:

```text
Cast
Frost
Elastomer
Gel
Lens treatments
```

Deliver:

- material schemas;
- definitions;
- fallback model;
- basic motion personalities.

---

# 176. Phase 3 — Headless reference engine

Implement:

```text
model
validator
tokens
environment
resolver
IR
```

First critical milestone:

```text
scenario.json
      ↓
reference engine
      ↓
Resina IR
```

completely headless.

---

# 177. Phase 4 — GUIdo vertical slice

Implement:

```text
Surface
Button
Toggle
Slider
Panel
FocusSurface
```

across:

```text
Cast
Frost
Elastomer
Gel
```

Goal:

validate real material expression.

---

# 178. Phase 5 — Resina Lab

Implement:

```text
Material Board
Component Board
State Board
Capability Board
Accessibility Board
```

The Lab becomes mandatory for visual changes.

---

# 179. Phase 6 — Quickshell validation

Implement only the core subset necessary to expose architectural leakage.

Initial:

```text
Surface
Button
Toggle
Slider
Panel
```

Do not attempt complete parity.

---

# 180. Phase 7 — Motion

Calibrate material motion.

Define spring vectors.

Test:

```text
pointer
touch
focus
controller
reduced motion
```

---

# 181. Phase 8 — Dynamic colour

Implement:

```text
seed extraction
palette generation
semantic mapping
pigment derivation
Legibility Guard
```

Use a hostile wallpaper corpus.

---

# 182. Phase 9 — Typography, icons and localization

Stabilize:

```text
typographic scale
text metrics
icon sizing
RTL
text expansion
CJK
large text
```

---

# 183. Phase 10 — Adaptation

Implement:

```text
density
geometry adaptation
input capabilities
viewing profiles
adaptive representations
```

Prove no device categories are required.

---

# 184. Phase 11 — Accessibility

Complete:

```text
semantic roles
keyboard
focus
directional navigation
reduced motion
reduced transparency
high contrast
large text
```

---

# 185. Phase 12 — Web backend

Build:

```text
documentation renderer
playground
scenario viewer
IR inspector
component gallery
```

This should become the public Resina documentation.

---

# 186. Phase 13 — Slint backend

Implement application-oriented subset.

Use identical:

```text
tokens
IR semantics
component contracts
```

---

# 187. Phase 14 — Profiles

Stabilize:

```text
Shell Profile
Couch/Game Profile
```

Potential future profiles MAY include:

```text
Automotive
Embedded
Large Display
```

only with real users/use-cases.

---

# 188. Phase 15 — Component expansion

Only after foundations stabilize:

```text
advanced forms
navigation
data display
menus
dialogs
notifications
lists
tables
trees
```

---

# 189. Phase 16 — Compatible program

Publish:

```text
conformance CLI
backend authoring guide
compatibility levels
test vectors
certification wording
```

Allow community backends such as Flutter.

---

# 190. Resina 0.1

Architecture proof.

Requires:

1. specification structure;
2. DTCG tokens;
3. material schemas;
4. four material families;
5. Environment Model;
6. capability model;
7. Resina IR;
8. Rust resolver;
9. Headless backend;
10. GUIdo vertical slice;
11. basic conformance vectors.

---

# 191. Resina 0.2

Material-language proof.

Requires:

```text
Resina Lab
motion
fallback
core component states
basic accessibility
Quickshell conformance prototype
```

---

# 192. Resina 0.3

Foundation-completeness milestone.

Requires:

```text
typography
spatial system
iconography
density
state composition
localization rules
content design
```

---

# 193. Resina 0.5

Usability milestone.

Requires:

```text
dynamic color
adaptation
full accessibility modes
core component library
Shell Profile
Web documentation
meaningful Quickshell coverage
```

---

# 194. Resina 0.7

Portability milestone.

Requires:

```text
Slint backend
stable IR candidate
stable component contracts
stable conformance tooling
backend authoring documentation
```

---

# 195. Resina 0.8

Architecture freeze candidate.

Requirements:

```text
stable material taxonomy
stable semantic token model
stable Environment Model
stable capability model
stable IR
governance operational
migration tooling
```

---

# 196. Resina 0.9

1.0 qualification.

Focus:

```text
backwards compatibility
performance
documentation
accessibility
localization
conformance
community backend experiment
```

A Flutter proof-of-concept MAY make sense here as an external test, but it is not required.

---

# 197. Resina 1.0 completion criteria

Resina 1.0 requires:

1. The specification can be implemented without reading Rust.
2. Core token format is standards-based.
3. Material definitions are stable.
4. Dynamic colour algorithms are specified.
5. Typography is specified.
6. Spatial system is specified.
7. Iconography is specified.
8. Motion model is specified.
9. State composition is specified.
10. Environment Model is stable.
11. Density model is stable.
12. Core components are stable.
13. Accessibility behaviour is stable.
14. Localization behaviour is stable.
15. Content Design guidance exists.
16. Resina IR is stable.
17. Capability negotiation is stable.
18. Fallback rules are complete.
19. Headless implementation passes normative vectors.
20. GUIdo provides production-quality high-fidelity rendering.
21. Quickshell proves renderer independence.
22. Slint proves application portability.
23. Web provides living documentation.
24. Tier 0 remains recognizably Resina.
25. Reduced transparency preserves identity.
26. Reduced motion preserves feedback.
27. High contrast remains coherent.
28. Large text works.
29. RTL works.
30. Multiple languages and text expansion are covered.
31. Conformance levels are published.
32. Backend authoring is documented.
33. Schemas are versioned.
34. Migration strategy exists.
35. Governance is documented.
36. Performance has been measured.
37. No mandatory feature depends on one rendering backend.

---

# 198. Final portability test

For every new feature ask:

> Could someone implement this in another language and renderer from the specification alone?

If no because it requires GUIdo:

wrong layer.

If no because it requires Rust:

incomplete specification.

If no because it requires Canopia:

probably product-specific.

If no because no fallback exists:

incomplete Resina feature.

---

# 199. Final identity test

Render the same component with:

```text
no blur
no transparency
no custom shaders
no advanced deformation
reduced motion
```

It should still communicate:

```text
material role
hierarchy
state
interaction
Resina geometry
Resina typography
Resina spacing
```

If it does:

Resina has a real design language.

If it does not:

Resina is still only an effects system.

---

# 200. Final rule

Resina should be able to become richer as rendering technology improves without changing what its components fundamentally mean.

The specification defines:

> **what the interface communicates.**

The resolver defines:

> **how design intent is transformed into resolved intent.**

The backend defines:

> **how that intent is rendered with available technology.**

The product defines:

> **how those components become an experience.**

Therefore:

```text
Resina Specification
      ↓
Resolved Design Intent
      ↓
Renderer
      ↓
Product
```

No layer should absorb the responsibilities of the layer above it.

---

# 201. Identity statement

Resina is:

> **A material-responsive design system in which hierarchy, interaction and motion are expressed through synthetic materials with different degrees of curing, elasticity, optical response and viscosity, while layout and component behaviour adapt to geometry, input capabilities, accessibility preferences and rendering capabilities rather than device categories.**

Its identity must remain recognizable:

- on a powerful GPU;
- on a low-power machine;
- without blur;
- without transparency;
- with reduced motion;
- with high contrast;
- with touch;
- with pointer;
- with keyboard;
- with controller;
- in GUIdo;
- in QtQuick;
- in Slint;
- in the Web;
- and in implementations that do not exist yet.

That is the definition of Resina being truly independent.