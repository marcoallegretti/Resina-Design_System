# Sequential focus traversal (candidate, 0.1.0)

Blueprint §67 requires deterministic focus and predictable tab order without a
pointer dependency. This operation chooses the next target in an explicitly
ordered focus scope. It supports ordinary sequences and explicitly cyclic
scopes. It does not derive spatial navigation, choose initial focus on loading,
change selection, activate controls, trap focus, or implement native events.
Its result is interaction policy output, separate from Resina paint IR.

The [request](../schemas/focus-traversal-request.schema.json) supplies version
`0.1.0`, ordered `targets`, an explicit `current` identifier or null, `direction`
(`forward` or `backward`) and explicit `wrap`. Every target supplies a nonempty
opaque string `id` and Boolean `eligible`. Identifiers MUST be unique, including
ineligible targets. Identity is exact: do not normalize Unicode, trim whitespace,
case-fold, sort identifiers, or create identifiers from labels.

The producer supplies an order aligned with the actual localized reading and
interaction order. The resolver MUST NOT infer it from geometry, RTL, platform,
toolkit, input device or positive tab indices. Eligibility is for this particular
sequence; it is not a claim that a target is enabled or globally focusable.
Component contracts determine eligibility. Disabled controls sometimes remain
focusable for discoverability, as explained by
[W3C keyboard guidance](https://www.w3.org/WAI/ARIA/apg/practices/keyboard-interface/#focusability-of-disabled-controls).
For a composite's outer tab sequence, expose the appropriate entry target rather
than inserting all internal targets; internal navigation remains separate.

With `current: null`, forward traversal chooses the first eligible target;
backward traversal chooses the last. This is an explicit scope-entry request,
not permission to set initial focus automatically. With a current identifier,
look strictly after its position for forward traversal or strictly before it
for backward traversal, skipping ineligible targets. The current identifier MUST
exist, but MAY have become ineligible; its stable position still anchors traversal.
Unknown current identifiers fail. Removing a focused target requires the owner
to choose explicit focus recovery; do not silently substitute scope entry.

If the directional remainder has no eligible target and `wrap` is true, continue
from the opposite end through the current position. A sole eligible current
target may therefore be returned again. With `wrap` false, do not cross the scope
boundary. Empty scopes and scopes without eligible targets produce no target.
No missing fields acquire defaults, including `current` and `wrap`.

The [result](../schemas/focus-traversal-result.schema.json) supplies version and
`targetId`, either the selected identifier or null when no target exists in the
permitted traversal. Null is a boundary/no-target outcome, not an instruction to
clear focus. The owner must apply native focus and display its visible cue only
after successful transfer. It remains responsible for scope entry/exit, focus
recovery, scrolling, selection policy and accessibility integration.

`resina-focus-traversal <path|->` reads a UTF-8 request up to 1 MiB. Success exits
0 with complete JSON. Validation or resolution failure exits 1 with a diagnostic
and no result; usage errors exit 2. Unknown/duplicate members, missing inputs,
invalid target identifiers, unsupported directions/versions, and unknown current
identifiers fail explicitly. Public cases and the external checker define the
portable boundary; this does not establish full component keyboard conformance.

Failure diagnostics follow the [public backend diagnostic policy](32-headless-conformance.md#failure-diagnostics). Negative public cases use `failure: true`; diagnostic wording is not a conformance requirement.
