# Resina Design System

Resina is a material-responsive design system specification. Its normative definitions are independent of languages, renderers, toolkits, operating systems, and products. The Rust workspace is a reference implementation of those definitions.

The [blueprint](RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes the intended architecture and development order. The versioned documents in `spec/`, machine-readable contracts in `schemas/`, and cases in `conformance/` define implemented behavior. A blueprint proposal is not considered implemented until these agree and the reference implementation passes its tests.

Current implemented scope: validation of platform-neutral environment snapshots and headless resolution of DTCG token documents, including structure, group extension, references, declared types, value shapes, and whole-token reference compatibility. Resina bundle compilation and rendering backends are not implemented yet.

Run `cargo test --workspace --locked` to check the reference model and conformance cases.
