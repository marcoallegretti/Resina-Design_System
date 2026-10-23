# Architecture decision records

An architecture decision record (ADR) explains why Resina made a technical choice, which alternatives it rejected and what follows from the choice. The normative behaviour stays in `spec/`, `schemas/`, `definitions/`, `tokens/` and `conformance/`. An ADR does not add requirements to them.

Numbers follow the initial ADR list in the [blueprint](../../RESINA_DESIGN_SYSTEM_BLUEPRINT.md) (§161). A record is added when its decision is made and implemented, so some numbers have no record yet. A later decision that changes a record supersedes it with a new record and does not rewrite the old one.

| Record | Status | Decision |
| --- | --- | --- |
| [0007](0007-dynamic-colour-model.md) | Accepted | Dynamic colour from explicit seeds in Oklch, with explicit legibility guards |
| [0008](0008-environmental-pigment-adaptation.md) | Accepted | Environmental pigment adaptation selected by theme resolution inside authored ranges |
