# Component anatomy (candidate, 0.1.0)

Blueprint sections 25 and 98 make a component a contract over named parts that
request semantic material roles and semantic color roles, which themes resolve. This contract
binds the parts of the Command, Toggle and Slider components to those roles. Anatomy
never names a material family, a pigment, a token value, a renderer or a size.

## Ownership

The binding belongs to the component contract, not to a theme. A role is the
interface between them: the component states what each part means, and the
theme states what each role looks like. A theme that cannot keep a part legible
corrects the color it assigns to the role; it does not rebind the part. Letting
themes rebind parts would let the same component mean different things in
different themes. This follows the [token layering](03-tokens.md): component
tokens should exist only when a semantic token cannot express the component
contract, and these bindings are expressed by semantic roles.
[Material's component tokens](https://github.com/material-components/material-web/blob/main/docs/theming/README.md)
default to system roles in the same way, but also accept concrete values and
selector-scoped overrides of a part's role. Resina deliberately keeps the binding in
the contract so that a part's meaning stays stable across themes and products.

## Definitions

A [component anatomy](../schemas/component-anatomy.schema.json) document has
`schemaVersion` `0.1.0` and names its `component`. Every part is required and
every member is explicit; unknown members, missing parts and unsupported
versions MUST be rejected. No part is inferred from another part, a variant, a
theme or a backend.

A surface part names `materialRole`, `colorRole`, `shape` and `elevation`.
Material roles are limited to `control.passive`, `control.interactive` and
`control.primary`, so the [material assignment](05-materials.md) rule keeps Gel
out of persistent component parts. A producer builds each part's surface intent
from these roles, the part's actual complete state set and an untreated
treatment stack.

- **Command** defines `standard` and `primary` variants. Each has a `body`
  surface part with a `contentRole` for the label drawn on it, and an embedded
  `label` naming its `typographyRole`. The variant is the producer's explicit
  emphasis choice; it is never inferred from the label or the action.
- **Toggle** defines `track` and `thumb` surface parts, each with a
  `checkedColorRole` and a `contentRole`, and a `label` with `typographyRole`
  and `colorRole`. The label is drawn beside the parts and remains the
  component's intrinsic accessible name. [Part paint](54-toggle-part-paint.md) binds
  the checked role when the state set contains `checked`. The track owns
  navigation paint. Neither part draws content; the content role is the
  foreground part paint records for it. The label keeps one color in every
  state: the parts' disabled response and the
  [accessible semantics](52-toggle-accessibility.md), not a dimmed label,
  convey unavailability.

- **Slider** defines `track` and `thumb` surface parts and a `label` with
  `typographyRole` and `colorRole`. A Slider has no checked state, so its parts
  bind no `checkedColorRole`. Each part names `materialRole`, `colorRole`,
  `shape`, `elevation` and a `contentRole`, the color part paint records for
  content that neither part draws. The thumb owns navigation paint
  ([part paint](76-slider-part-paint.md)). The label is drawn beside the parts
  and is the component's intrinsic accessible name. It keeps one color in
  every state. This version binds no active or inactive track segment: see
  [segments](#slider-segments).

The authored [Command](../definitions/components/command.json),
[Toggle](../definitions/components/toggle.json) and
[Slider](../definitions/components/slider.json) definitions use a neutral
standard command and an accent primary command. The Toggle track turns from
`surface.low` to `selection` when checked. Material binds a
[selected switch track](https://github.com/material-components/material-web/blob/main/docs/components/switch.md)
to its primary accent instead. In both authored themes, neither outline
role reaches 3:1 against `accent.primary` (1.89:1 and 1.19:1 in Light), so a
thumb on such a track could not keep a [contrast-guarded edge](25-edge-contrast.md).
The selection role names the same state and keeps that edge. The Slider track is
an embedded `control.passive` well on `surface.low` and its thumb a raised
`control.interactive` capsule on `content.secondary`, the binding of an unchecked
Toggle thumb. This version binds no segment role: the active and inactive
segment roles are the subject of
[#301](https://github.com/marcoallegretti/Resina-Design_System/issues/301), where
the single `track` part is replaced by the segment parts. Binding no accent or
selection role here does not rule one out. The thumb color role is
evidence-neutral, as [below](#slider-segments); it follows the Toggle precedent.
The thumb's content
color roles are an authoring choice: the requirements below identify the thumb
by its edge, not by its body color.

## Legibility requirements

Producers MUST pass these thresholds to the existing operations, in every state
including disabled. WCAG exempts inactive components; Resina does not, so that
a disabled control stays identifiable.

- A Command body: content contrast 4.5:1 between its body and `contentRole`,
  the level [WCAG 2.2 SC 1.4.3](https://www.w3.org/TR/WCAG22/#contrast-minimum)
  sets for normal text.
- Every surface part: edge contrast 3:1, the level
  [SC 1.4.11](https://www.w3.org/TR/WCAG22/#non-text-contrast) sets for
  user-interface components. Edge contrast is the
  [outline selection](25-edge-contrast.md) against the part's actual adjacent
  color: the canvas for a Command body and a Toggle track, the track's resolved
  body for a Toggle thumb.
- A Slider thumb is wider than its track, so its edge borders both the canvas
  and the track's resolved body, and must reach 3:1 against each. The track
  edge is checked against the canvas. A Slider label has 4.5:1 against the
  canvas. The thumb's focus ring has the 3:1 of
  [part paint](76-slider-part-paint.md) against the same surroundings.
- A Toggle snapshot: minimum thumb-edge contrast 3:1 and minimum label contrast
  4.5:1 against the label's canvas, in the [snapshot](55-toggle-snapshot.md)
  operation.

A part without content is resolved with its `contentRole` and a minimum content
contrast of 1. That role then cannot cause a failure or a fallback, and nothing
is drawn in it, but the published part paint records it. A definition that cannot meet
these requirements in a theme is invalid for that theme. The producer MUST NOT
repair it by changing roles, capabilities or thresholds.

## Evidence

Rust tests resolve both authored definitions through Command paint and Toggle
part paint, in the Light and Dark themes and in each of the 27 Cast, Frost and
Elastomer assignments of the three control roles. They cover each Command and
Toggle phase with and without focus, and each Toggle phase with and without
selection. Command and Toggle parts are resolved as [lit control bodies](88-lit-control-body.md) at
the sizes of the illustrative layouts: a 96 by 48 px Command body, a 64.5 by
40.5 px Toggle track and a 16 by 16 px Toggle thumb. The environment is the public
[minimal-capability snapshot](../conformance/environment/valid-minimal-capabilities.json),
where Frost resolves to its opaque representation, and the canvas is the theme's
`surface.base`. On other containers, edge validity rests on the
[authored theme](23-authored-themes.md) outline guarantees. Opaque part paint cannot
resolve a translucent Frost body, so Frost assignments carry this evidence only
where Frost resolves opaque. A complete Toggle snapshot also depends on part
sizes and layout, which anatomy does not define; its composition is evidenced by
the snapshot contract.

The same tests establish four properties of the lit bodies for every authored
body, in both themes and under every family assignment, in that snapshot and in
the same snapshot with every condition that admits interaction light: high
contrast and reduced motion off, `dynamicLighting` on and balanced quality.
Light is never a state cue: state perceivability is measured on the unlit
body colour and final depth, and the full ambient and interaction channels,
admitted and suppressed, change nothing but `face.light`, so suppressing light
removes no cue. Light never reaches the content plateau: the content rectangle
keeps the exact body colour under every admitted light, and interaction light
appears in the resolved face only in the hover and pressed phases of an
environment that admits it. The disabled Command label sits at its dimming bound,
3:1 or 4.5:1 under high contrast, or is unchanged when its contrast is already
below that bound. Every shape of the bundled Tier 0 appearance carries each part's
[relief profile](88-lit-control-body.md#part-keyed-relief) in both authored
themes, so none reaches the `tau < 1` rejection. Label and thumb containment are
decided by their own [Command](46-command-label-layout.md) and
[Toggle](55-toggle-snapshot.md) fixtures.

The [visibility table](../conformance/materials/lit-control-visibility.json),
[schema](../schemas/lit-control-visibility.schema.json), records how visible the
static key is on these bodies. For each theme, family and bound colour role it
gives the body colour and the largest ΔE_OK between that body and the colours
painted where the signed key response toward the highlight and toward the side
peaks. Light is not a state cue, so the table is evidence of how much the light
contributes to identity, not a threshold. One half of the light is faint on most bodies, and which half depends on the
body. The Light `surface.high` Command body has a highlight half of at most 0.0011
ΔE_OK, far below any visible difference, so only its shading half shows (0.034
Cast, 0.048 Frost, 0.086 Elastomer). The Dark `surface.high` body shows its
highlight half (0.028 to 0.062) and its shading half is faint (0.0098 Cast, 0.014
Frost, 0.025 Elastomer). The Light unchecked and checked track has a highlight
half of 0.0028 to 0.011, the Light thumb a shading half of 0.0068 to 0.031, and the
Dark thumb a highlight half of 0.0011 to 0.0098. The values are those of the
[reference key light](34-key-light.md), `(−0.34, −0.94)`. The table is evidence about the
lit body as authored; it does not accept its appearance. The
[independent arithmetic](../tools/lit_control_visibility.py) locates each peak
with the lit body normals and reproduces every row; the Rust test paints the same
witness points with the resolver and compares their ΔE_OK. The table is
regenerated with `python tools/lit_control_visibility.py --generate -- <theme-resolve-command> -`
and checked with `python tools/lit_control_visibility.py`.

Slider parts are
[spec/36 bodies](76-slider-part-paint.md), not lit control bodies, at an
illustrative 160 by 8 px track and 20 by 20 px thumb. A Rust test resolves the
track and the thumb, the thumb with and without focus, in both themes under each
of the 28 family assignments (112 thumb configurations) against the canvas and
the track body. Resolution enforces the 3:1 edges, the label color is checked at
4.5:1, and a focused thumb's ring is checked at 3:1. Those resolutions use the identity rest
response. The state responses of the [public Slider fixture](71-slider-appearance.md)
are arithmetic test coefficients, not an authored profile, so the Slider's
hover, pressed, dragging, disabled and read-only responses are not evidenced
here.

<a id="slider-segments"></a>
The [Slider snapshot](77-slider-snapshot.md) checks the thumb edge against every
color the track paints, including the track's own exterior edge. The thumb edge
is chosen from the same outline roles. A test shows that in both themes and all
28 assignments the rest thumb is rejected against the whole track cover, with
one outline at 1:1 and the other at most 1.61:1 against the track's edge (the
test computes both ratios), and that it resolves against the canvas and the
track body. The cause lies in the check, not in the thumb's color role: when
each of `content.primary`, `content.secondary`, `surface.high`, `accent.primary`
and `selection` was substituted for the thumb, it passed against the track body
and failed against the whole cover. That substitution was a one-off check, not a
committed test.
Narrower adjacency needs the clearance geometry of
[track segments](79-slider-track-segments.md), whose roles belong here. They are
not bound in this version because no contract composes segment paint with the
track yet; the single `track` part is replaced by the segment parts in #301.

Schema cases reject missing parts, unknown members,
non-control material roles and members of the other components. The reference
model exposes typed anatomy and builds part surface intents; it introduces no
rendering dependency.
