# Interaction state input (candidate, schema 0.1.0)

Resina uses the canonical states `rest`, `hover`, `focused`, `pressed`, `active`, `selected`, `checked`, `disabled`, `busy`, `dragging`, `error`, `warning`, and `success`. A state set is an explicit snapshot of states applicable to one intent at one resolution point. It does not identify a component, input device, toolkit, or rendering primitive. The machine-readable form is [state-set.schema.json](../schemas/state-set.schema.json).

A producer MUST provide a nonempty `states` array and `schemaVersion` `0.1.0`. Names are case-sensitive. Duplicate states, unknown states, unknown members, and unsupported versions are invalid. An empty array MUST NOT silently become `rest`. Combinations remain representable at this layer, including focused plus selected, pressed plus selected, disabled plus busy, and base rest plus hover. Component contracts may later restrict their own supported states, but the generic model MUST NOT infer component-specific incompatibilities.

A state-set source MUST be a JSON object, and every state name MUST be a string. Positional records and object-encoded names are invalid, including when decoded through the shared input type.

Consumers MUST treat the set as unordered. The reference model serializes it in the state order listed above for inspection and deterministic conformance; this order does not define visual priority. [Semantic state composition](16-state-composition.md) groups these signals into layers while preserving every applicable state. Resolution rules for pigment, edge, motion, and other outputs must preserve applicable layers instead of replacing the whole result with one winning state. Those rules require separate contracts and vectors.

The [state vectors](../conformance/states/state-set-vectors.json) cover valid combinations, normalization, and rejection cases.
