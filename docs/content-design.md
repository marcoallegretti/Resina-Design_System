# Content design guide

This is informative guidance for interface-copy authors, producers that supply text to Resina, and component authors. It is not normative and introduces no new normative behavior. The [blueprint](../RESINA_DESIGN_SYSTEM_BLUEPRINT.md#91-content-design) states Resina's content-design intent in sections 91 to 97. The rules that Resina can check are already normative in the [label layout](../spec/46-command-label-layout.md), [command accessibility](../spec/47-command-accessibility.md) and [command content](../spec/90-command-content-accessibility.md) specifications. This guide classifies every sentence of sections 92 to 97 as one of those, as advisory wording guidance, or as belonging to a component contract that does not exist yet. It paraphrases the blueprint; where the two differ, the blueprint states the intent and the specifications state the implemented behavior.

Wording quality is a human judgement. A validator cannot tell a specific label from a vague one in every language, so most rules below have no diagnostic. The table says so for each rule instead of implying a check.

## Terms

- **Producer**: the party that supplies text, measurements and localization to Resina before resolution. The specifications give the producer ownership of localization.
- **Complete localized string**: one string, in the locale in use, that is the whole text a person reads or hears for a label, name or description.
- **Diagnostic**: a rejection or error that an existing specification, schema, vector or checker defines. "None; human review" means no tool in this repository evaluates the rule.

## Rule table

Status values:

- **Normative**: a specification already requires the checkable part, linked in the row. The guide adds no requirement.
- **Advisory**: wording guidance for human authors and reviewers.
- **Later component**: the rule needs a component contract that is not specified yet. See [Not yet owned](#not-yet-owned).

Locale values: **Neutral** means the principle holds for any language. **Latin script** means the concrete form given applies to languages written in Latin script only; the guide states no form for other scripts, because the repository has no evidence for them.

| # | Blueprint | Guidance | Scope | Status | Locale | Diagnostic |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | §92 concise | Say what is needed and no more. Brevity is an authoring choice; it never licenses shortening a string at display time. A layout reflows complete text. | Interface-copy author | Advisory. Preservation of the complete string is normative in [spec/46](../spec/46-command-label-layout.md): no ellipsis, line limit or automatic font shrinking, and all text including explicit line breaks is preserved | Neutral | None; human review. Truncation and shrinking are forbidden by spec/46 |
| 2 | §92 direct | Address the reader plainly and put the point first. | Interface-copy author | Advisory | Neutral principle; word order is language-specific | None; human review |
| 3 | §92 specific | Name the object and the action instead of a generic term. | Interface-copy author | Advisory | Neutral | None; human review. [WCAG 2.4.6](https://www.w3.org/WAI/WCAG22/Understanding/headings-and-labels.html) asks for descriptive labels and leaves the judgement to a person |
| 4 | §92 action-oriented | Tell the person what they can do or what the control does. | Interface-copy author | Advisory | Neutral | None; human review |
| 5 | §92 non-technical unless the context is technical | Use the reader's words. Keep codes, identifiers and internal terms for contexts whose reader expects them, and keep them out of the primary message. | Interface-copy author | Advisory | Neutral | None; human review |
| 6 | §92 non-judgmental | Describe the situation without blaming the person or labelling their input as wrong. | Interface-copy author | Advisory | Neutral | None; human review |
| 7 | §93 prefer action verbs | A control that starts an action names that action. This guide gives no part-of-speech rule: verb-first labels are a convention of some languages, and the neutral principle is that the label names the action. | Interface-copy author, producer | Advisory | Neutral principle; verb-first form is not stated for any language | None; human review |
| 8 | §93 example verbs (Save, Connect, Remove, Restart, Try again) | The blueprint examples illustrate the principle. They are examples in English, not a vocabulary Resina requires or checks. | Interface-copy author | Advisory | Examples are English; localize the principle, not the words | None; human review |
| 9 | §93 avoid OK, Proceed, Do it when a more specific action exists | Prefer the specific action when one exists. A generic label is acceptable when no specific action exists. No list of discouraged words is maintained: such a list would be English-specific, and a passing list would give false assurance about other languages. | Interface-copy author, reviewer | Advisory | Neutral principle; no word list | None; human review |
| 10 | §94 explain what failed | State the failure in plain terms. | Interface-copy author | Later component for message structure; advisory for wording | Neutral | None; human review |
| 11 | §94 explain what remains safe | Say what was not lost or changed when that is true and relevant. The sentence has no universal shape, so no message template is defined. | Interface-copy author | Later component for message structure; advisory for wording | Neutral | None; human review |
| 12 | §94 explain what the user can do | Offer the next step when one is known. [WCAG 3.3.3](https://www.w3.org/WAI/WCAG22/Understanding/error-suggestion.html) requires that a known correction be provided when an input error is detected, which is application behavior and is tested by triggering the error. | Interface-copy author, component author | Later component for message structure; advisory for wording | Neutral | None; human review |
| 13 | §94 bad and better example | The blueprint contrasts a bare numeric code with a sentence that names the failure and the next step. The contrast is the point; the example text is not a template. | Interface-copy author | Advisory | Example is English | None; human review |
| 14 | §94 technical detail through expandable diagnostics | Detail for a technical reader may sit behind a disclosure, away from the primary message. The expandable pattern for a Command is a [disclosure](../spec/93-disclosure.md) concern; the placement of error detail is not specified. | Component author | Later component | Neutral | None for placement. Disclosure name and description follow [spec/47](../spec/47-command-accessibility.md) |
| 15 | §95 explicit verbs for destructive actions | Name the verb and the object (the blueprint gives Delete file, Remove account, Forget network). | Interface-copy author | Advisory for the label; confirmation behavior is a later component | Neutral principle; examples are English | None; human review |
| 16 | §95 rather than Confirm or Yes | A bare Confirm or Yes cannot be understood without the question beside it. Do not rely on neighboring text for the meaning of the action. | Interface-copy author, reviewer | Advisory; the confirmation pattern is a later component | Neutral | None; human review |
| 17 | §96 prefer meaningful progress | When the amount of work done is unknown, describe the activity in progress. The blueprint examples end in an ellipsis; that character is authored text that Resina preserves exactly, and it is unrelated to the ellipsis truncation that spec/46 forbids. | Interface-copy author | Later component for progress presentation; advisory for wording | Neutral principle; ellipsis form is Latin script and other scripts follow their own convention | None; human review |
| 18 | §96 avoid inventing percentages when real progress is unknown | Show a percentage only when it is measured. This is a behavior of a progress presentation, so it is owned by the component that presents it. | Component author | Later component | Neutral | None |
| 19 | §97 explain the state | Say what is empty. | Interface-copy author | Later component for structure; advisory for wording | Neutral | None; human review |
| 20 | §97 optionally explain why | Add the reason only when it helps the reader act. | Interface-copy author | Later component for structure; advisory for wording | Neutral | None; human review |
| 21 | §97 offer a useful action where applicable | Provide a next step when one exists. The action is an ordinary Command whose label follows rules 7 to 9. | Interface-copy author, component author | Later component | Neutral | None |

Section 91 frames these rules (interface language communicates, in addition to visuals) and states no rule of its own.

### Capitalization and punctuation

The blueprint states no capitalization or punctuation rule. Mature systems that publish one use sentence case: the [GOV.UK A to Z style guide](https://guidance.publishing.service.gov.uk/writing-to-gov-uk-standards/style-guides/a-to-z-style-guide/) says to always use sentence case, with exceptions such as proper nouns, and [IBM Carbon's content overview](https://carbondesignsystem.com/guidelines/content/overview/) uses sentence-style capitalization for headings, titles, labels and similar elements. These sources describe English-language house styles enforced by editorial review.

For languages written in Latin script, a project may adopt the same convention for its own copy: capitalize the first word and proper nouns only. A synthetic example string follows, invented for illustration and not a translation of anything.

| Convention | Sentence case | Not sentence case |
| --- | --- | --- |
| Latin script, synthetic | Mirela donvar | Mirela Donvar |

The guide states no case, punctuation or ellipsis rule for scripts without letter case or for scripts whose punctuation differs, and Resina provides no check. Resina preserves capitalization and punctuation exactly as supplied ([spec/46](../spec/46-command-label-layout.md), [spec/47](../spec/47-command-accessibility.md)).

## Localization-safe authoring

Resina carries opaque complete localized strings. It does not interpret, assemble or format them. The producer owns localization before resolution ([spec/46](../spec/46-command-label-layout.md), [spec/47](../spec/47-command-accessibility.md), [spec/90](../spec/90-command-content-accessibility.md)). The following notes are informative recommendations for producers.

- **Write whole strings.** Supply one complete localized string per label or message. Concatenating fragments assumes an order, agreement and punctuation that differ between languages.
- **Format plurals, numbers and dates in the producer.** Choose the plural form and format the number or date for the locale before the string reaches Resina. A producer can use any message-formatting facility. [Unicode MessageFormat 2](https://unicode.org/reports/tr35/tr35-messageFormat.html) (Unicode LDML part 9, a stable document) is one such format: its messages select a variant by plural category and require a catch-all variant. Resina does not adopt it or any other message format, and defines no message contract.
- **Isolate inserted text for bidirectional display.** When a message inserts text whose direction may differ from the message, such as a device name, the producer wraps the insertion in bidirectional isolation controls so it cannot reorder neighboring text. The MessageFormat 2 specification encourages isolating controls around placeholders. Resina preserves the string exactly; this guide does not verify how each platform's accessible-name computation treats those control characters.
- **Do not assume English length.** The [blueprint](../RESINA_DESIGN_SYSTEM_BLUEPRINT.md#83-text-expansion) asks controls not to assume English label length, and spec/46 reflows complete text instead of truncating it.
- **Treat the length matrix as test metadata.** The [expansion matrix](../conformance/content/command-label-expansion.json) holds three English strings of 9, 13 and 18 characters, labelled 100%, 150% and 200% of the first. [spec/46](../spec/46-command-label-layout.md#independent-content-expansion) states that the classification is test authoring metadata, must not estimate rendered width and does not prescribe translation length. Real expansion varies: the [W3C article on text size in translation](https://www.w3.org/International/articles/article-text-size) reports that, on average, the shorter the English source, the greater the expansion, and that this does not hold for every string: some translations are shorter than their source.
- **Layout direction is not script direction.** Padding follows the layout direction and the paragraph follows the script of the text ([spec/46](../spec/46-command-label-layout.md)). Physical light direction does not mirror ([design language](design-language.md)).

## Accessible names

- A Command whose label is visible takes its accessible name from the complete laid-out label, exactly, with no trimming, shortening or tooltip substitution ([spec/47](../spec/47-command-accessibility.md), [spec/90](../spec/90-command-content-accessibility.md)).
- An icon-only Command requires a producer-supplied complete localized `accessibleLabel`. Resina never derives the name from the icon reference, an icon mapping, a tooltip or default text ([spec/90](../spec/90-command-content-accessibility.md)).
- Write the accessible label as the action it performs, following rules 7 to 9, in the locale in use.

**Limit of machine checking.** A string is blank, and rejected, when it is empty or contains only Unicode `White_Space` characters. Zero-width format characters such as U+200B, U+2060 and U+FEFF are not `White_Space`, so a label made only of them passes the structural checker. [spec/90](../spec/90-command-content-accessibility.md) states that such a label names nothing a person can hear or read and does not meet the obligation to provide a complete label, and the [content vectors](../conformance/content/command-content-vectors.json) and [accessibility cases](../conformance/accessibility/command-content-cases.json) record the case as accepted. Meeting the obligation is the producer's responsibility, and review of the supplied text remains a human task.

## Not yet owned

These areas are named in the blueprint's content rules and have no component contract, schema or conformance case in this repository. Nothing here is implemented.

- Error message structure (what failed, what remains safe, what the person can do), and where expandable detail is placed.
- Progress presentation, including when a percentage may be shown.
- Empty-state structure.
- Destructive-action confirmation.
- Notification content.

When a component contract is specified for one of these areas, the issue that specifies it cites this guide, decides which rules become normative for that component, and states its own proof. Until then they stay advisory. A structured error record and a message-format contract would be new public contracts and need a maintainer decision before they are specified.
