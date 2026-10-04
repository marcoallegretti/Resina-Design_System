# Toggle interaction projection (candidate, 0.1.0)

Blueprint §§59–61 require selection, interaction, availability and navigation to
coexist. This operation projects current binary Toggle state into the existing
semantic [state set](14-interaction.md) before appearance resolution.

## Inputs and ownership

The [request](../schemas/toggle-states-request.schema.json) requires version
`0.1.0`, validated `activation`, explicit boolean `hovered` and boolean `checked`.
Use current committed activation and checked values from
[binary toggle activation](50-toggle-activation.md), or the latest external
checked property update. Actual focus and hover remain separate observations;
hover MUST NOT be inferred from focus, capture or the held pointer's membership.
A producer without hover supplies false explicitly. No missing signal gets a
default; unknown/duplicate fields and invalid activation states fail.

## Projection

1. Project availability, hover, momentary press, body rest and actual focus by
   the [ordinary activation projection law](48-command-states.md).
2. Include `checked` exactly when the supplied checked value is true. Checked
   selection MUST NOT remove or replace any projected signal. In particular,
   idle checked Toggle retains `rest`; disabled checked Toggle retains its
   selection, independent actual focus and explicit hover.
3. Return one complete nonempty versioned state set in canonical order.

[Composition](16-state-composition.md) places checked in selection, rest in base,
focus in navigation, hover/press in interaction and disabled in availability.
The false checked value adds no state: it does not invent `unchecked`, selected,
active, busy or validation signals. Projection performs no activation, capture,
focus transfer or checked-value update.

This result is component interaction policy, not paint. Ordinary command paint
rejects checked state and MUST NOT be made to appear conforming by dropping it.
Toggle selection appearance, geometry, motion and accessible semantics require
component-specific contracts. This operation does not certify a native Toggle
or assistive technology delivery.

## Reference and conformance

Rust supplies `resolve_toggle_states` and `resolve_toggle_states_source`.
[Public cases](../conformance/interaction/toggle-states-cases.json) cover on/off
projection over body rest, hover, key/pointer holds, independent pointer hover,
disabled focus discovery and invalid inputs. Sequence tests connect actual
Toggle transitions to composition and preserve the command paint guard.

`resina-toggle-states <path|->` reads strict UTF-8 JSON up to 1 MiB. Success exits
0 with complete state-set JSON and empty stderr; invalid input exits 1 with a
diagnostic and no output; usage errors exit 2. The
[external checker](../tools/check_toggle_states_backend.py) runs valid cases
twice and checks exact signals, including selection, without importing Rust.
