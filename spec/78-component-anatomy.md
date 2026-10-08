# Component anatomy (candidate, 0.1.0)

Blueprint sections 25 and 98 make a component a contract over named parts that
request semantic material roles and semantic color roles, which themes resolve. This contract
binds the parts of the Command and Toggle components to those roles. Anatomy
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

The authored [Command](../definitions/components/command.json) and
[Toggle](../definitions/components/toggle.json) definitions use a neutral
standard command and an accent primary command. The Toggle track turns from
`surface.low` to `selection` when checked. Material binds a
[selected switch track](https://github.com/material-components/material-web/blob/main/docs/components/switch.md)
to its primary accent instead. In both authored themes, neither outline
role reaches 3:1 against `accent.primary` (1.89:1 and 1.19:1 in Light), so a
thumb on such a track could not keep a [contrast-guarded edge](25-edge-contrast.md).
The selection role names the same state and keeps that edge. The thumb's content
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
selection. The environment is the public
[minimal-capability snapshot](../conformance/environment/valid-minimal-capabilities.json),
where Frost resolves to its opaque representation, and the canvas is the theme's
`surface.base`. On other containers, edge validity rests on the
[authored theme](23-authored-themes.md) outline guarantees. Opaque part paint cannot
resolve a translucent Frost body, so Frost assignments carry this evidence only
where Frost resolves opaque. A complete Toggle snapshot also depends on part
sizes and layout, which anatomy does not define; its composition is evidenced by
the snapshot contract. Schema cases reject missing parts, unknown members,
non-control material roles and members of the other component. The reference
model exposes typed anatomy and builds part surface intents; it introduces no
rendering dependency.
