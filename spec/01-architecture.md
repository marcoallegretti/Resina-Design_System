# Architecture and authority

Resina is a design-system specification, not a UI toolkit. The specification, canonical reference implementation, rendering backends, platform integration, and products have distinct authority. Rust is the canonical reference implementation of Resina algorithms; it is not the normative definition. A conforming implementation in another language MUST be possible from the public specifications, schemas, definitions, tokens, and conformance cases without reverse-engineering Rust.

The [blueprint](../RESINA_DESIGN_SYSTEM_BLUEPRINT.md) sets the intended system and development direction. A proposed feature becomes an implemented contract when its public specification, applicable machine-readable contract, conformance evidence, and reference behavior agree. Authoring tokens in `tokens/` are authoritative over generated constants and compiled bundles. Generated artifacts MUST NOT acquire independent normative meaning.

## Layers

| Layer | Responsibility |
| --- | --- |
| Specification | Defines semantic intent, input and output contracts, resolution rules, fallbacks, and conformance requirements. |
| Reference model and resolver | Parse and validate inputs, resolve tokens and design intent, and emit inspectable Resina results. |
| Resina IR | Carries resolved design intent from the resolver to a renderer. |
| Rendering backend | Realizes Resina intent using capabilities it actually provides. |
| Platform integration | Reports environment, input and rendering capabilities to the resolver and connects to operating-system facilities. |
| Product | Selects themes, components and content for a particular application or shell. |

Normative Resina models MUST remain independent of GUIdo, Qt, Slint, Wayland, wgpu, Web APIs, and product-specific objects. Shaders, compositor effects, toolkit controls, and window handles belong to implementation layers. Material definitions describe intent; shaders are one possible realization.

The reference implementation in `reference/rust/` is the canonical implementation of specified parsing, validation and resolution behavior. The `resina-color` crate owns color fallback and contrast calculations; `resina-resolver` applies those calculations to semantic roles and environment decisions. A difference between reference behavior and a public contract is a defect to investigate, not an automatic change to the contract.

## Resolution boundary

The resolver combines validated component intent, variant, state, theme, environment, accessibility preferences, renderer capabilities, and quality policy into Resina IR. Identical inputs SHOULD produce semantically equivalent results. Capability decisions MUST use declared capabilities and requirements, not backend names or device classes. Accessibility constraints and quality policy are explicit inputs, not inferred renderer properties.

Resina IR represents computed design intent. It MUST remain renderer, toolkit, and platform agnostic. It is not a widget tree, layout engine, event system, scene graph, or GPU command buffer. Backends MAY use different rendering techniques for the same IR but MUST preserve its semantics and documented lower-capability fallbacks. A feature that has no valid lower-capability representation is not ready for a Resina contract. Tier 0 MUST still preserve recognizable Resina hierarchy, shape, edge, pigment and interaction state.

Headless validation and resolution establish behavior before renderer-specific work. The [environment contract](18-environment.md), [capability model](30-capabilities.md), and [fallback rules](31-fallback.md) define currently implemented slices. No unspecified material, motion, typography, or component behavior should be inferred from those slices.

The [opaque base-state surface IR](36-opaque-surface-ir.md) defines one static appearance slice, including portable geometry and paint semantics. It does not define full component, optical treatment, or motion IR.

The [focus indicator IR](37-focus-indicator-ir.md) independently resolves the navigation cue around the bound form's swept footprint while preserving concurrent state signals. It does not substitute for their component appearance rules.

The [surface paint IR](39-surface-paint-ir.md) composes opaque body and navigation channels from one semantic snapshot and matching geometry inputs. It publishes a complete result only when every required channel succeeds.

The [scalar spring reference](38-spring-reference.md) defines deterministic sampling and an explicit reduced-motion endpoint. `resina-model` owns its validated parameters; `resina-motion` owns the calculation. Material motion profiles, component transitions and motion IR remain separate contracts.

The [command activation lifecycle](42-command-activation.md) defines portable
command interaction state and effects. The model validates states and event
vocabulary; the resolver chooses the next state and effects. This interaction
protocol is separate from paint IR, native event delivery and product actions.

The [ordinary command accessibility snapshot](47-command-accessibility.md) resolves semantic content, current state, focusability and invocation availability from validated label and activation inputs. It is separate from paint IR and native accessibility tree delivery.

The [command interaction projection](48-command-states.md) derives supported paint signals from the same committed activation state and explicit hover observation. Native adapters supply observations; they do not recreate pressed, focused or disabled policy independently for painting.

## Backend roles

GUIdo is the high-fidelity reference renderer. Its advanced drawing techniques demonstrate Resina but do not raise the minimum capability floor. Quickshell is an independent Shell conformance backend and must be able to realize the semantics without copying GUIdo's rendering architecture. Slint is the application backend for conventional interfaces. Web provides documentation, conformance views and a playground using broadly available web primitives first. Headless conformance is the shared reference for all backends.

Backends MUST report unsupported capabilities honestly and apply the specified fallback result. A backend-specific enhancement MAY improve presentation only when it preserves the resolved semantics and does not become a hidden normative requirement. Product-specific features belong in products or profiles rather than Core Resina.

The words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY express requirement strength in public normative documents. Informative examples do not override explicit contracts or conformance vectors.
