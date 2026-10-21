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

The label's lines are centered within that offered box when drawn. The same typography, content and font context MUST reach drawing, and drawing MUST reproduce the lines of the fitted measurement. When that measurement wrapped any text between mandatory line breaks, drawing MUST wrap within the offered width; a backend numeric conversion MUST NOT widen it, and a renderer that cannot reproduce the measured lines MUST fail. When nothing wrapped, drawing MAY omit the wrapping constraint, because no break depends on it; every mandatory line break still ends a line in the same box. A renderer that shapes at device resolution can otherwise break a line that exactly fills the box. Paragraph shaping still follows the text's script and bidirectional rules; layout direction maps logical padding and does not force a script direction.

Dimension arithmetic uses IEEE 754 binary64 values. Horizontal and vertical padding aggregates, preferred width, and fitted height plus vertical padding MUST use the smallest finite representable value greater than or equal to the exact sum of their binary64 operands. Available content width and height MUST use the largest representable value less than or equal to the exact difference between the chosen command dimension and its padding aggregate. Each operation uses its already resolved operands; do not collapse these steps into a differently associated expression. This minimal conservative rounding preserves complete content and never widens a wrapping offer. Exactly representable results remain unchanged. No fixed epsilon, unconditional extra ULP, pixel snapping or relaxation of maximum dimensions is permitted.

Before directed rounding, an ordinary round-to-nearest, ties-to-even sum that entirely loses either positive operand MUST fail. Subtraction that entirely loses positive padding MUST also fail. Directed rounding MUST NOT manufacture a surviving term in either case. Nonpositive available extent, nonfinite results (including outward overflow), a fitted width exceeding the inward offer, or a conservative required height exceeding the maximum remain diagnostic. Vertical centering uses the inward available height; this policy does not change font measurements, typography, contour containment or backend precision requirements.

Nonfinite arithmetic or an entirely lost positive content/padding term MUST fail explicitly. Geometry must retain positive label space. Complete measurement failures propagate with their cause. This operation does not hide unsupported states behind default metrics.

The output preserves text and resolved typography, command size, physical label box and layout direction at schema version 0.1.0. The schema checks structure; semantic validation also enforces nonblank text, containment and the resolution equations.

## Evidence and native boundary

The [IR example](../conformance/ir/command-label-ir.json) is an independently specified layout arithmetic case with supplied natural and fitted extents, not a claim about a particular font. The [supplied arithmetic cases](../conformance/content/command-label-arithmetic.json) cover fractional outward sums, inward offers, aggregate padding, exact subnormals, RTL slack, maximum-size rejection, overflow and genuinely lost positive terms. Their expected outcomes are checked independently with exact rational arithmetic, then consumed by the Rust reference. These are numeric layout cases, not native font or accessibility evidence.

Rust tests cover minimum/content/maximum width, complete wrapped height, RTL padding, vertical slack, producer failures, impossible layouts and numeric extremes.

The GUIdo backend's `measure_command_label` uses the pinned renderer's actual styled shaping facade. Its caller supplies an explicitly selected `FontFamily` with verified font availability and fallback policy; GUIdo's ambient font lookup is not a portable font mapping contract. Fractional weight and metrics that exceed backend precision fail diagnostically. The [backend README](../backends/guido/resina-guido/README.md#complete-label-drawing) describes how its drawing reproduces the measured lines.

Linux tests explicitly load the Inter 4.0 static Regular, Medium and SemiBold masters pinned in [`tools/conformance_font.json`](../tools/conformance_font.json) before first measurement and exercise Latin, expanded German and Arabic labels at 100%, 150% and 200% text scaling. They rebuild the face database of the pinned text stack, the system fonts followed by the registered files, and fail unless every visible upright, normal-width face of that family at an authored weight is byte-identical to the pinned master of that weight. With the font matching that the pinned text stack tries first, they then verify that each authored weight resolves to its own master; that check reports a Regular-only registration as a substituted weight. They verify actual complete shaping and reflow. The GUIdo `prepare_command_label` adapter remeasures at the same offered width before emitting a centered, complete text draw command with no line limit. It rejects mismatched complete height or width, unsupported metrics, unrepresentable bounds and invalid color channels. Callers retain responsibility for the explicit font/fallback mapping, actual guarded content color, placement transforms and surrounding clipping policy. It does not replace headless content contrast guards.

Required native GPU tests render the same three labels at text scales 100%, 150% and 200%, each at device scales 1, 1.25, 2 and 3. Actual readback verifies visible ink on every expected line and centered placement on the first submitted frame. Deliberately constrained expanded words can require glyph-boundary wrapping; this is content-preservation evidence, not a calibrated product width profile. Optional captures support visual review. These cases establish native text drawing evidence at the tested scales; a complete component, font substitution conformance and accessibility conformance remain separate work.

## Independent content expansion

Blueprint §83 requests string-length expansion independently of font scaling. The [public length matrix](../conformance/content/command-label-expansion.json) supplies “Reconnect”, “Reconnect now” and “Reconnect to Wi-Fi”: 9, 13 and 18 ASCII characters relative to the 9-character baseline. Their target ratios are 100%, approximately 150% and 200%; actual ratios must remain within 5% relative error of those targets in this fixture. ASCII scalar, byte and grapheme counts coincide here. This classification is test authoring metadata and MUST NOT be used to estimate rendered width or replace font shaping. It does not prescribe translation length.

The same matrix reaches native complete measurement and drawing at every tested text/device scale, alongside the original Latin, German and Arabic cases: six labels × three text scales × four device scales, or 72 GPU frames. Content length, text scale and device scale are independent axes. Public schema checks and native fixture loading reject missing target coverage, duplicate names/targets, blank or miscounted text and ratios outside the fixture tolerance. These scenarios do not establish universal line breaking, all-language or complete component conformance.

## Uniform body content containment

Before drawing a label using the opaque body's guarded foreground, which in the disabled phase is the [dimmed disabled label](43-command-paint.md#disabled-label), composition MUST establish that its complete layout box lies within the body's uniform content region defined in [opaque surface IR](36-opaque-surface-ir.md). Matching command dimensions or testing only the rectangular bounds of that region is insufficient around rounded corners. Labels and paint MUST refer to the same command front size and physical origin. The extrusion silhouette is not the command front size.

The Rust reference exposes `CommandLabelIr::validate_content(&OpaqueSurfaceIr)`. It accepts the body from either static or sampled motion paint, compares front dimensions and origin, and tests all four physical label-box corners against the placed content contour. The current resolved content contour is a convex rectangle with circular or square corners: containing the four vertices contains their entire rectangular convex hull. Membership includes the boundary and uses the same contour geometry as opaque paint sampling. Nonrepresentable arithmetic and unsupported geometry are diagnostic. A future nonconvex content representation requires a different containment proof before this guard can accept it.

This guard neither changes layout nor guesses new padding. On failure, composition must resolve a suitable size, padding or complete reflow policy and validate again; it MUST NOT crop, silently shrink or relocate text. Logical padding has already been mapped to physical coordinates by label resolution. The caller must retain the common environment and typography context for label and paint; containment does not validate that context.

This is layout-box evidence, not exact glyph-ink containment. Producers and renderers must still allow required overhang and verify its actual background when evaluating content contrast. Antialiasing coverage, font substitutions, motion frames, surrounding clipping and full Button conformance remain separate requirements. The [public containment cases](../conformance/content/command-label-containment.json) specify supplied measurement extents and expected outcomes independently of any font engine.

## Relief face content

The same complete-label policy applies to [opaque relief face paint](83-opaque-relief-paint.md).
Its certified content region is a uniform rectangle, independently established
inside the analytic plateau and contrasting edge interior. Before using the
face's guarded foreground, composition MUST match the label's command size to
the relief outline size and contain the complete physical layout box in that
published region. Both paint and label retain the same placement origin.

`CommandLabelIr::validate_relief_content(&ReliefPaint)` performs this guard without
changing layout, relief, targets or navigation. The label's right/bottom endpoints
use the smallest finite binary64 values at least their exact coordinate-plus-extent
sums, using the existing conservative extent arithmetic. Compare them to the
certified represented content endpoints used by face sampling. An endpoint that
cannot retain both positive operands is diagnostic; rounding cannot authorize
text outside the plateau. Rectangle boundaries are included. A shoulder, edge,
mismatched size or unrepresentable box fails explicitly. Resolve suitable size,
padding or complete reflow and validate again; do not crop or shrink content.

This is layout-box containment, with the same font, ink-overhang, actual-background
and environment obligations as the preceding guard. It does not certify a
focused component or native interaction. Relief does not alter text measurement.

The [supplied containment matrix](../conformance/content/command-relief-containment.json)
uses complete 120×60 px faces and supplied 1 px wide layout advances. For curved
faces, 14/18 px logical padding keeps the complete 20 px high label away from
all shoulders; zero horizontal padding crosses the exterior edge. For square
convex/recessed faces with a 6 px shoulder, the plateau is exactly
`[6,114] × [6,54]`: the 48 px high box with 6 px horizontal padding is included,
while its next binary64 height is outside. Both layout directions consume the
same physical containment law. These are supplied measurement cases, not native
font or complete styling evidence.
