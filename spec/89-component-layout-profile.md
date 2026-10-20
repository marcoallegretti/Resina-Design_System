# Component layout profiles (candidate, 0.1.0)

Blueprint sections 34, 69 to 72, 77, 83 and 98 make components own their
adaptation to explicit density, text scale, input and layout direction, with
compact density reducing control height without lowering accessibility minimums. A component layout profile is an authored Resina
definition. It binds a component's content insets to [spatial roles](10-spatial.md)
and its visual size limits to component [tokens](03-tokens.md) for every
[density mode](17-density.md).
Resolving a profile produces the inputs of an existing component layout
contract. It does not measure content, place a component, or choose a density.

## Profile

A [profile](../schemas/component-layout-profile.schema.json) has
`schemaVersion` `0.1.0`, a `component`, a `padding` binding and `extents`. All
members are required; missing, unknown and duplicate members, positional
records and unsupported versions are invalid.

Version 0.1.0 defines `command`. Its `padding.start` and `padding.end` MUST name
`space.control.inset.inline`, and `padding.top` and `padding.bottom` MUST name
`space.control.inset.block`. The content-gap roles `space.control.inline` and
`space.control.block` are not insets and are rejected, as is any other role or
a role on the wrong axis. Logical start and end follow the resolved layout
direction; they are not physical left and right.

`extents` has exactly the members `compact`, `standard`, `comfortable` and
`immersive`. Each holds a `minimumSize` and a `maximumSize`, and each of those
names a nonempty DTCG token path for its `width` and its `height`. A profile
carries no bare size value: extents are component tokens in the token package,
because no semantic role names a component extent. They are authored for every
mode, and a profile never derives one mode's extents from
another. These are visual body limits. They are not hit regions.

Each referenced token MUST resolve to a DTCG `dimension` in `px` with a finite,
positive value, and in every mode the resolved minimum MUST NOT exceed the
resolved maximum on either axis. Resolution checks every mode in the order
`compact`, `standard`, `comfortable`, `immersive`, and within a mode the
minimum width, minimum height, maximum width and maximum height, whichever mode
the environment selects; it reports the first failure. A missing token, a path
naming a group, another type, another unit and a value that is not positive
are each rejected for that reason.

The [Command profile](../definitions/layout/command.json) names the
`command.extent.<mode>.minimum` and `command.extent.<mode>.maximum` width and
height tokens of the [Command extent tokens](../tokens/components/command.json).
For compact, standard, comfortable and immersive these author minimum sizes
64×32, 96×40, 112×48 and 144×64 and maximum sizes 240×160, 320×200, 360×240 and
480×320. Each mode is larger than the previous one on every axis. The
[profile cases](../conformance/layout/command-profile-cases.json) accept the
authored profile and reject each invalid form, including a bare pixel extent,
for its own reason.

## Command resolution

The [request](../schemas/command-layout-request.schema.json) supplies version
`0.1.0`, a profile, one DTCG token document `tokens` holding the profile's
extent tokens, the theme's complete [spatial binding](../schemas/spatial-binding.schema.json)
with every density mapping of the inset roles, one validated
[environment snapshot](18-environment.md) and a finite, positive
`availableSize`: the space the owner can offer the command body after ancestor
clipping and safe areas. Nothing is inferred when a member is missing.

1. Resolve `tokens` as a complete DTCG document, with duplicate members
   rejected at every depth, and resolve and check every mode's extents as
   described above. A token document that fails resolution fails the request.
   Then select the extent and both inset values for the environment's
   explicit `densityPreference`. An inset role's value MUST use the unit `px`;
   another unit fails, because no font-relative conversion is defined here.
2. `minimumSize` is the selected minimum. Text scale does not change it.
3. Each axis of `maximumSize` is the smaller of the available extent and the
   selected maximum multiplied by `max(1, textScale)`. The product uses
   IEEE 754 binary64 round-to-nearest and MUST be finite; a text scale whose
   product overflows fails resolution rather than publishing an unbounded
   limit. Larger text therefore
   widens and heightens the limit in proportion, so a label that fits at 100%
   is not rejected at 200% only because the limit stayed fixed. Smaller text
   does not shrink the authored limit.
4. If the available extent is smaller than the selected minimum on either axis,
   resolution fails. A minimum is never reduced to fit a container.
5. `padding` maps start and end to the inline inset and top and bottom to the
   block inset. The inline and block padding aggregates use the
   [complete label layout](46-command-label-layout.md) conservative sum. If an
   aggregate is not less than the maximum on its axis, resolution fails: the
   insets leave no label space.
6. `targetMinimum` is the [minimum interactive target](15-targets.md) for the
   environment's input capabilities: 24×24, or 48×48 with `coarsePointer` or
   `directTouch`. Density, text scale and the profile never change it.

The [result](../schemas/command-layout-ir.schema.json) records version,
density, layout direction, `minimumSize`, `maximumSize`, `padding` and
`targetMinimum`. `minimumSize`, `maximumSize`, `padding` and layout direction
are exactly the size, padding and direction inputs of complete Command label
layout; label layout then reflows long labels within the offered width or
fails when the complete label exceeds the maximum height. `targetMinimum` is
the `componentMinimum` of the [hit region](40-hit-region-ir.md); the visual
body may be smaller than that region and is never enlarged to reach it.

## Conformance

The [layout cases](../conformance/layout/command-layout-cases.json) use the
authored Command profile, its extent tokens and the spatial binding shared by
the authored light and dark themes. That binding gives compact 16 px inline and
8 px block insets: compact lowers the body height but keeps the standard inline
inset. With the 8 px block inset, a 12 px inline inset would cross the lit
body's corner shoulder once the corner radius reaches 24 px. They cover every density mode at text
scales 1 and 2, fine and coarse input, and both layout directions. Every mode
keeps the 24-unit fine and 48-unit coarse target, including compact, whose
visual minimum height is 32, and direct touch without a coarse pointer selects
the 48-unit target.
Further cases clamp the maximum to a narrow container, accept a container equal
to the minimum, reject a container below the profile minimum, reject a text
scale whose maximum overflows, reject insets that leave no label space, reject
a font-relative inset, resolve an aliased extent token, reject missing,
mistyped, non-px and nonpositive extent tokens, an unresolved alias and a
minimum above its maximum in a selected or unselected mode, and reject invalid
profiles and bindings. Schema checks
recompute every expected result with exact rational arithmetic, independently
of Rust. Rust integration tests compile both authored themes, confirm that
their binding, the authored profile and its extent tokens are the ones these
cases use, and pass resolved policies to
complete label layout with supplied advances, not a font, to show reflow and
explicit rejection. For every density, text scales 1, 1.5 and 2, one and two
supplied label lines and both layout directions, the resolved policy and label
layout place the label inside the
[lit command body content](46-command-label-layout.md#lit-command-body-content)
of a rounded Tier 0 Command body with the reference control relief and edge
width; exact rational arithmetic and the Rust reference each check this matrix.

`resina-command-layout <path|->` reads strict UTF-8 JSON up to 1 MiB. Success
exits 0 with complete JSON and no diagnostic; invalid input exits 1 with no
result; usage errors exit 2. The [external checker](../tools/check_command_layout_backend.py)
validates repeated output and duplicate rejection independently of Rust.
Measurement, placement in a parent layout, Toggle and Slider profiles, and
backend adoption of these values remain separate work.
