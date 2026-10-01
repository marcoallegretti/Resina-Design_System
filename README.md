# Resina Design System

Resina is a material-responsive design system specification. Its normative definitions are independent of languages, renderers, toolkits, operating systems, and products. The Rust workspace is a reference implementation of those definitions.

The [blueprint](RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes the intended architecture and development order. The versioned documents in `spec/`, machine-readable contracts in `schemas/`, and cases in `conformance/` define implemented behavior. A blueprint proposal is not considered implemented until these agree and the reference implementation passes its tests.

Current implemented scope: validation of platform-neutral environment snapshots, interaction state sets, and surface form intent; strict semantic material, color, and spatial role assignments and optical treatment nesting; Frost capability fallback; and headless resolution of DTCG token sources, documents, semantic color and spatial bindings, including duplicate-safe parsing, structure, group extension, references, declared types, value shapes, and whole-token reference compatibility. The calibrated spatial foundation source is in `tokens/foundation.json`. Resina bundle compilation and rendering backends are not implemented yet.

Run `cargo test --workspace --locked` to check the reference model and conformance cases.
Run `python -m pip install -r tools/requirements-schema.txt` and `python tools/check_schemas.py` to check Draft 2020-12 schemas against applicable public vectors. Cross-field environment constraints remain covered by Rust validation.

Run `cargo run -p resina-tokens --bin resina-token-resolve -- <path>` to validate a UTF-8 DTCG authoring source and print its deterministic resolved path-to-token JSON. Use `-` instead of a path to read stdin. Invalid sources produce diagnostics on stderr, a nonzero status, and no resolved JSON. This command resolves tokens; it does not compile a Resina bundle.
