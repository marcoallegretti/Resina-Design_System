# Token format and authority (candidate)

Generic Resina design tokens MUST use the [DTCG Design Tokens Format Module 2025.10](https://www.designtokens.org/TR/2025.10/format/). Color tokens MUST follow its [Color Module 2025.10](https://www.designtokens.org/TR/2025.10/color/). These are stable Design Tokens Community Group reports, though they are not W3C Standards. Resina does not define alternate syntax for token values, groups, type inheritance, aliases, or group extensions.

Authoring files in `tokens/` are normative. Generated Rust constants, backend assets, and compiled bundles are derived artifacts. A backend MUST NOT infer token meaning from a group name or a physical color value. It MUST consume the resolved token type and the semantic role specified by Resina.

Resina organizes authoring tokens into foundation, semantic, and occasional component layers. Foundation tokens contain raw scales. Semantic tokens name intent and SHOULD alias foundation or other semantic tokens. Component tokens SHOULD exist only when a semantic token cannot express the needed component contract. Alias cycles, unresolved references, missing types, and invalid values MUST be rejected. Implementations processing the format MUST support both whole-token curly-brace aliases and JSON Pointer `$ref` references, including chained references and cycle detection, as required by the DTCG format.

Group extensions and token-set composition are processed before a token's aliases are resolved. Reference resolution does not itself validate the resolved value's declared type; type validation MUST follow before a token enters a Resina bundle. The [reference vectors](../conformance/tokens/reference-vectors.json) cover whole-token aliases, property references, escaped JSON Pointers, cycles, and invalid targets.

The [structure vectors](../conformance/tokens/structure-vectors.json) cover group and token shape, metadata, reserved names, and root tokens. Structural validation alone does not establish DTCG conformance: extension targets, inherited types, and each declared value type also require validation.

The [DTCG Resolver Module 2025.10](https://www.designtokens.org/TR/2025.10/resolver/) is also a stable Community Group report. Its token-set and modifier composition may be useful for authoring themes. It does not define Resina's resolution of material roles, environment, component state, accessibility, renderer capabilities, quality policy, or fallbacks into Resina IR. The Resina resolver MUST define those decisions independently. A future token-set integration MAY consume DTCG resolver output as input, without changing this boundary.

No numerical spatial scale or palette is fixed by this document. The blueprint requires spatial values to be calibrated through component prototyping, and color assignment must meet the later legibility and fallback contracts. Values MUST NOT be presented as normative before those contracts and conformance cases exist.
