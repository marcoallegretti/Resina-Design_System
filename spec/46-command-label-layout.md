# Complete command label layout (candidate, schema 0.1.0)

This contract implements the blueprint's content sizing and explicit reflow policy for a text label. It is one part of command anatomy. It does not define a complete Button, its accessibility tree, invocation, icon arrangement, or native event delivery.

## Ownership and measurement

Resina owns the layout policy and [portable IR](../schemas/command-label-ir.schema.json). A measurement producer owns font selection, shaping and complete text measurement. No font engine, toolkit object, font filename, glyph identifier or renderer capability belongs in the IR.

Inputs are nonblank text, resolved label typography, positive minimum/maximum sizes, nonnegative logical start/end/top/bottom padding, explicit layout direction and a measurement producer. Dimensions are logical pixels. Minimum size MUST NOT exceed maximum size on either axis. Typography already includes environment text scaling; neither layout nor its producer may apply that scale again.

The producer MUST measure the supplied text and typography with the same font mapping and fallback policy throughout resolution and drawing. Font registration must precede measurement. Font substitutions and unsupported typography require an explicit backend policy or diagnostic; they may not silently change Resina typography. A missing producer or producer failure is diagnostic, not an estimated character width.

Measurements report complete layout advances and line heights, not exact glyph ink bounds. Both dimensions MUST be finite and positive. A producer MUST preserve all text, including explicit line breaks and shaped scripts. No ellipsis, line limit or automatic font shrinking is allowed in this operation. Wrapping must preserve content, including a word wider than the offered width, or report failure. Rendering must allow required glyph overhang; layout dimensions are not an instruction to crop ink.

## Resolution

1. Validate policy before calling the producer. Padding must leave positive space within the maximum size.
2. Measure complete text without a width constraint. Add horizontal padding to its width.
3. Clamp this preferred width between the explicit minimum and maximum widths. Subtract horizontal padding to obtain the offered label width.
4. Measure complete text again at that offered width. The fitted measurement MUST NOT exceed it.
5. Set command height to the larger of minimum height and complete fitted height plus vertical padding. Exceeding maximum height MUST fail with no IR. Do not truncate or shrink the label to fit.
6. Position the label at start padding in LTR and end padding in RTL. Center its complete height within any remaining vertical space after padding. Preserve the **offered** wrapping width in the label box, rather than shrinking the box to the longest fitted line.

The label's lines are centered within that offered box when drawn. The same wrapping constraint, typography, content and font context MUST reach drawing. Paragraph shaping still follows the text's script and bidirectional rules; layout direction maps logical padding and does not force a script direction. A backend numeric conversion must preserve the wrapping constraint without widening it.

Nonfinite arithmetic or an entirely lost positive content/padding term MUST fail explicitly. Geometry must retain positive label space. Complete measurement failures propagate with their cause. This operation does not hide unsupported states behind default metrics.

The output preserves text and resolved typography, command size, physical label box and layout direction at schema version 0.1.0. The schema checks structure; semantic validation also enforces nonblank text, containment and the resolution equations.

## Evidence and native boundary

The [IR example](../conformance/ir/command-label-ir.json) is an independently specified layout arithmetic case with supplied natural and fitted extents, not a claim about a particular font. Rust tests cover minimum/content/maximum width, complete wrapped height, RTL padding, vertical slack, producer failures, impossible layouts and numeric extremes.

The GUIdo backend's `measure_command_label` uses the pinned renderer's actual styled shaping facade. Its caller supplies an explicitly selected `FontFamily` with verified font availability and fallback policy; GUIdo's ambient font lookup is not a portable font mapping contract. Unsupported nonzero letter spacing, fractional weight and metrics that exceed backend precision fail diagnostically. Draw with the same native size, weight and relative line height, centered alignment and a wrapping width rounded toward the lower representable value, without line limits or ellipsis.

Linux tests explicitly load DejaVu Sans before first measurement and exercise Latin, expanded German and Arabic labels at 100%, 150% and 200% text scaling. They verify actual complete shaping and reflow. The GUIdo `prepare_command_label` adapter remeasures at the same offered width before emitting a centered, complete text draw command with no line limit. It rejects mismatched complete height or width, unsupported metrics, unrepresentable bounds and invalid color channels. Callers retain responsibility for the explicit font/fallback mapping, actual guarded content color, placement transforms and surrounding clipping policy. It does not replace headless content contrast guards.

Required native GPU tests render the same three labels at text scales 100%, 150% and 200%, each at device scales 1, 1.25, 2 and 3. Actual readback verifies visible ink on every expected line and centered placement on the first submitted frame. Deliberately constrained expanded words can require glyph-boundary wrapping; this is content-preservation evidence, not a calibrated product width profile. Optional captures support visual review. These cases establish native text drawing evidence at the tested scales; a complete component, font substitution conformance and accessibility conformance remain separate work.
