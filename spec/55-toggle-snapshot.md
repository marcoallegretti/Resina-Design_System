# Coherent binary Toggle snapshot (candidate, 0.1.0)

Blueprint sections 59–62, 84–85, 98 and 106–108 require one component's
appearance, content, selection, interaction and semantics to agree. This
operation publishes the existing horizontal layout, opaque track/thumb paint,
complete measured label with checked foreground/backdrop colors, stable target
and binary accessible semantics together.
It defines a portable composition law, not a native widget or transaction.

## Inputs and ownership

Supply validated complete label IR, its physical local origin, explicit portable
sRGB foreground and actual uniform opaque backdrop colors, and a minimum label
contrast ratio; validated [part layout](53-toggle-layout.md) and [track/thumb paint](54-toggle-part-paint.md);
a reserved hit-region IR; current activation, hover and checked signals;
description and focusability; explicit minimum thumb-edge contrast; environment;
available bounds, component target minimum and occupied neighboring reservations.
All coordinates use the track's local frame. The label origin places the
label's complete layout slot; its internal label bounds remain unchanged.

Layout owns the target independently of feedback. It must reserve track paint,
track navigation, complete label slot and both off/on thumb footprints throughout
the supported state/profile domain. Environment, profile or layout changes may
require a new reservation and coherent lifecycle update. Do not resize or move
the target solely because press, hover, selection or availability changed.

The producer owns common theme/environment/font provenance, scaled native
measurement, actual focus observation, color-role selection and actual uniform
label-backdrop provenance, and native coordinates. Comparing declared signals,
directions and physical geometry does not prove those external facts. A supplied measurement is not
permission to substitute approximate font metrics in production.

## Validation and publication

1. Require a declared track and thumb in their correct slots. Compare each body's
   **complete** state set to the exact current [Toggle projection](51-toggle-states.md).
   Matching phase alone does not establish current focus, hover, availability or
   selection. Report expected/supplied states and the offending part.
2. Require layout and both parts' checked signals to match current checked.
   Require label/layout direction to match the current environment. Require local
   front dimensions and origin to match the authored part allocation.
3. Translate the thumb's complete current painted silhouette bounds to **both**
   logical endpoints. Require every corner of each footprint rectangle inside
   the track's convex uniform content contour. This certifies uniform track
   pigment under the whole thumb, including extrusion. Allocation fit alone
   cannot certify this: an inset may leave paint on a curved edge, highlight band
   or outside the track. The rectangle certificate is intentionally conservative;
   fail explicitly when it cannot be established, even if some curved outlines
   could fit. Do not silently sample a center pixel or clip the thumb to pass.
   A nonuniform-backdrop or more permissive contour proof requires a separately
   specified resolution law; this operation does not claim it.
4. Recompute the current thumb edge's opaque contrast against the track's actual
   resolved body pigment. Require an explicit finite threshold in `[1,21]`,
   and require the reported thumb edge ratio to match within the existing
   opaque-paint ratio tolerance of `1e-12` absolute. The minimum threshold is
   still enforced strictly; this tolerance does not permit insufficient contrast.
   Resolve thumb paint with that actual track pigment as its adjacent color;
   stale hypothetical surroundings must not pass. Equal ratios alone do not
   prove theme identity. Preserve existing part readability/fallback laws.
5. Revalidate the reserved target against actual minima, available bounds and
   neighboring reservations. Reject any required resizing. Require it to contain
   current track silhouette, both placed thumb footprints, placed track outer
   navigation bounds when focused, and the complete label layout slot. Use closed
   bounds containment; pointer ownership retains the separate half-open target law.
6. Require the label slot not to overlap the complete part or navigation footprint
   rectangles. Fail nonfinite, overflowed or swallowed placement/extents explicitly.
7. Require an explicit finite minimum label contrast in `[1,21]` reflecting the
   applicable text/accessibility policy. Use the existing [opaque contrast law](12-contrast.md)
   on the exact supplied label foreground and uniform backdrop. Both colors must
   be opaque: fail with the underlying foreground/background diagnostic rather
   than guessing, clamping alpha, compositing over an invented backdrop or
   changing capabilities. A translucent parent must first supply its actual
   resolved opaque composited label backdrop. Nonuniform label backdrops require
   separate spatial readability evidence and cannot be represented by a guessed
   flat color here. A readable track foreground does not establish an external
   label's contrast. Do not infer label colors from track or thumb paint, apply
   an unrequested inverse color, or weaken the minimum using numeric tolerance.
8. Resolve [binary semantics](52-toggle-accessibility.md) from the exact complete
   label and current activation/checked signals, description and focusability.
9. Publish all members only after every check succeeds. On failure publish no
   snapshot. Do not remove independent state, weaken thresholds, repair supplied
   paint, suppress navigation, clip content or enlarge the reservation.

The snapshot is not action authorization. Raw and semantic action delivery still
uses the current activation/availability lifecycle, including changes after
publication. One component exposes one switch identity, not separate interactive
track and thumb identities.

## Reference and evidence

Rust supplies `resolve_toggle_snapshot`, typed input and a read-only result
borrowing the validated label/layout/part paint while owning target, semantics,
checked label foreground/backdrop colors and verified thumb/label contrast
ratios. Live activation, palette color inputs and layout context are not retained.
No new wire format is introduced: the members retain their existing portable IR schemas and the label origin is ordinary physical geometry.

[Public state cases](../conformance/interaction/toggle-snapshot-cases.json) cover
off/on live and stale channels, held keys, pointer membership and independent
selection disagreements. Rust runs them across three persistent families and
both directions. Geometry tests distinguish allocation fit from uniform pigment
coverage at both endpoints, stale adjacent-color evidence, insufficient contrast,
part size/slot mismatches, label occlusion, navigation coverage, clipping,
overlap, minimum changes and semantic failures. Label extents are explicit
arithmetic fixtures; these tests do not claim native font measurement. Label
readability tests reuse the independent public color-contrast vectors, distinguish
track readability from external-label readability, reject invalid minima and
translucency, and verify owned colors after live palette input changes.

Native event/accessibility publication, calibrated complete component styling,
nonuniform track composition, motion and renderer integration remain separate
conformance requirements. A headless snapshot does not certify a complete native
Toggle release.
