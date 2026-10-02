# Resina Design System

Resina is a material-responsive design system specification. Its normative definitions are independent of languages, renderers, toolkits, operating systems, and products. The Rust workspace is a reference implementation of those definitions.

The [blueprint](RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes the intended architecture and development order. The versioned documents in `spec/`, machine-readable contracts in `schemas/`, and cases in `conformance/` define implemented behavior. A blueprint proposal is not considered implemented until these agree and the reference implementation passes its tests.

Current implemented scope: validation of platform-neutral environment snapshots, interaction state sets, and surface form intent; strict semantic material, color, spatial, and typography role assignments and optical treatment nesting; Frost capability fallback and portable Frost body color; capability-based minimum hit-target resolution; and headless resolution of DTCG token sources, documents, semantic color, spatial, and typography bindings, including duplicate-safe parsing, structure, group extension, references, declared types, value shapes, and whole-token reference compatibility. A versioned theme source can be compiled into validated, environment-independent bindings and resolved against an environment snapshot. Portable sRGB fallback, authored opaque fallback selection, known-backdrop sRGB compositing, and opaque color contrast are also defined and tested. The calibrated spatial and type-size foundation source is in `tokens/foundation.json`. Headless commands resolve a complete semantic snapshot or bind one surface from a versioned scenario. Resina IR, bundle compilation, and rendering backends are not implemented yet.

Run `cargo test --workspace --locked` to check the reference model and conformance cases.
Run `python -m pip install -r tools/requirements-schema.txt` and `python tools/check_schemas.py` to check Draft 2020-12 schemas against applicable public vectors. Cross-field environment constraints remain covered by Rust validation.

Run `cargo run -p resina-tokens --bin resina-token-resolve -- <path>` to validate a UTF-8 DTCG authoring source and print its deterministic resolved path-to-token JSON. Use `-` instead of a path to read stdin. Invalid sources produce diagnostics on stderr, a nonzero status, and no resolved JSON. This command resolves tokens; it does not compile a Resina bundle.

Run `python tools/check_token_backend.py -- <backend-command> -` to check any token resolver that implements the public [token command boundary](spec/03-tokens.md). Build `resina-token-resolve` first to check the Rust reference through the same external boundary.

The Rust reference caps group-extension expansion and token-reference resolution at 100,000 constructed JSON value nodes and 8 MiB of cloned string and member-name bytes each. The reference-resolution budget covers the complete document. An over-limit source fails explicitly without token output. These resource guards do not define Resina or DTCG format limits.

Run `cargo run -p resina-resolver --bin resina-headless -- <path>` to resolve a [headless request](schemas/headless-resolution.schema.json) from a file, or use `-` for stdin. The command prints a complete semantic snapshot, including portable and opaque color fallbacks, and emits no partial JSON on failure.

Run `python tools/check_headless_backend.py -- <backend-command> -` to check any backend that implements the [headless command protocol](spec/32-headless-conformance.md) against the public result schema and conformance cases. Build the Rust `resina-headless` binary first to check the reference implementation through the same external boundary.

Run `cargo run -p resina-resolver --bin resina-surface-bind -- <path>` to resolve a [surface scenario](schemas/surface-scenario.schema.json) from a file, or use `-` for stdin. The command prints one bound surface only when both semantic resolution and binding succeed. This result is not render-ready Resina IR.

Run `python tools/check_surface_backend.py -- <backend-command> -` to check a surface scenario backend against the public binding vectors and capability-sensitive scenario cases.
