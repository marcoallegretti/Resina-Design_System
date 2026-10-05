# Resina Design System

Resina is a material-responsive design system specification. Its normative definitions are independent of languages, renderers, toolkits, operating systems, and products. The Rust workspace is a reference implementation of those definitions.

The [blueprint](RESINA_DESIGN_SYSTEM_BLUEPRINT.md) describes the intended architecture and development order. The versioned documents in `spec/`, machine-readable contracts in `schemas/`, and cases in `conformance/` define implemented behavior. A blueprint proposal is not considered implemented until these agree and the reference implementation passes its tests.

The authored [light](tokens/themes/light.json) and [dark](tokens/themes/dark.json) theme sources compose [foundation tokens](tokens/foundation.json) into complete current semantic assignments. The [Tier 0 appearance profile](definitions/tier0-surface-appearance.json) supplies their explicit opaque surface geometry, depth, pigment, light, and band choices. Complete component and state-specific appearance remains to be specified.

Current implemented scope: validation of platform-neutral environment snapshots, interaction state sets, semantic state composition, and surface form intent; strict semantic material, color, spatial, and typography role assignments and optical treatment nesting; Frost capability fallback and portable Frost body color; capability-based minimum hit-target resolution; and headless resolution of DTCG token sources, documents, semantic color, spatial, and typography bindings, including duplicate-safe parsing, structure, group extension, references, declared types, value shapes, and whole-token reference compatibility. The Rust reference can compose DTCG resolver sets and modifier contexts from explicit source text before token resolution. A theme source can embed tokens or use that composition result for validated, environment-independent bindings, then resolve against an environment snapshot. Portable sRGB fallback, authored opaque fallback selection, known-backdrop sRGB compositing, and opaque color contrast are also defined and tested. The authored spatial, type-size, raw radius, and depth foundation source is in `tokens/foundation.json`. Headless commands resolve a complete semantic snapshot or bind one surface from a versioned scenario. Static opaque surface IR is implemented; complete component/treatment IR, bundle compilation, and full backend conformance remain outstanding.

Run `cargo test --workspace --locked` to check the reference model and conformance cases.
The [Tier 0 shape fallback](definitions/tier0-shapes.json) resolves semantic shape intents to bounded elliptical radii, covered by headless vectors. The [opaque base-state surface IR](spec/36-opaque-surface-ir.md) composes these shapes with readable pigments, directional depth, and explicit edge, highlight, and content regions. Advanced curvature and full component/treatment IR remain future contracts.
Run `python -m pip install -r tools/requirements-schema.txt` and `python tools/check_schemas.py` to check Draft 2020-12 schemas against applicable public vectors. Cross-field environment constraints remain covered by Rust validation.

Run `cargo run -p resina-tokens --bin resina-token-resolve -- <path>` to validate a UTF-8 DTCG authoring source and print its deterministic resolved path-to-token JSON. Use `-` instead of a path to read stdin. Invalid sources produce diagnostics on stderr, a nonzero status, and no resolved JSON. This command resolves tokens; it does not compile a Resina bundle.

Run `cargo run -p resina-tokens --bin resina-token-compose -- <path>` to compose a [resolver request](schemas/resolver-module-request.schema.json) from a file, or use `-` for stdin. The command emits the same resolved token mapping. `python tools/check_resolver_backend.py -- <backend-command> -` tests this protocol against the public resolver vectors and authored foundation source.

Run `cargo run -p resina-resolver --bin resina-theme-resolve -- <path>` to compile a theme and resolve it against an environment using a [theme resolution request](schemas/theme-resolution-request.schema.json); use `-` for stdin. `python tools/check_theme_backend.py -- <backend-command> -` tests the language-neutral boundary, including external foundation reuse.

Run `python tools/check_token_backend.py -- <backend-command> -` to check any token resolver that implements the public [token command boundary](spec/03-tokens.md). Build `resina-token-resolve` first to check the Rust reference through the same external boundary.

The Rust reference caps group-extension expansion and token-reference resolution at 100,000 constructed JSON value nodes and 8 MiB of cloned string and member-name bytes each. The reference-resolution budget covers the complete document. An over-limit source fails explicitly without token output. These resource guards do not define Resina or DTCG format limits.

Run `cargo run -p resina-resolver --bin resina-headless -- <path>` to resolve a [headless request](schemas/headless-resolution.schema.json) from a file, or use `-` for stdin. The command prints a complete semantic snapshot, including portable and opaque color fallbacks, and emits no partial JSON on failure.

Run `python tools/check_headless_backend.py -- <backend-command> -` to check any backend that implements the [headless command protocol](spec/32-headless-conformance.md) against the public result schema and conformance cases. Build the Rust `resina-headless` binary first to check the reference implementation through the same external boundary.

Run `cargo run -p resina-resolver --bin resina-surface-bind -- <path>` to resolve a [surface scenario](schemas/surface-scenario.schema.json) from a file, or use `-` for stdin. The command prints one bound surface only when both semantic resolution and binding succeed. This result is not render-ready Resina IR.

Run `python tools/check_surface_backend.py -- <backend-command> -` to check a surface scenario backend against the public binding vectors and capability-sensitive scenario cases.

Run `cargo run -p resina-resolver --bin resina-frost-legibility -- <path>` for a [known-backdrop Frost legibility request](schemas/frost-legibility-request.schema.json), or use `-` for stdin. `python tools/check_frost_legibility_backend.py -- <backend-command> -` checks the external protocol and fallback cases. This local color decision does not measure a dynamic rendered backdrop or produce Resina IR.

Run `cargo run -p resina-resolver --bin resina-edge-contrast -- <path>` for a [known-adjacent edge contrast request](schemas/edge-contrast-request.schema.json), or use `-` for stdin. `python tools/check_edge_contrast_backend.py -- <backend-command> -` checks an independent backend against the public edge vectors. This chooses an authored outline color, not a complete edge style or Resina IR.

Run `cargo run -p resina-resolver --bin resina-frost-surface-readability -- <path>` for a [Frost surface color readability request](schemas/frost-surface-readability-request.schema.json), or use `-` for stdin. The Rust `resolve_frost_surface_readability` API accepts a compiled theme resolution and surface intent directly. `python tools/check_frost_surface_readability_backend.py -- <backend-command> -` checks composition of the bound Frost body, semantic foreground, and authored edge colors against known local surroundings.

Run `cargo run -p resina-resolver --bin resina-surface-readability -- <path>` for the [base-state surface readability request](schemas/surface-readability-request.schema.json) across all four material families. `python tools/check_surface_readability_backend.py -- <backend-command> -` checks content and local edge color decisions through the public command protocol.

Run `cargo run -p resina-resolver --bin resina-opaque-pigment -- <path>` for an [opaque material pigment request](schemas/opaque-pigment-request.schema.json). The operation derives opaque side and highlight colors from the selected body and explicit family profiles. `python tools/check_opaque_pigment_backend.py -- <backend-command> -` checks its numerical output and diagnostic failures.

Run `cargo run -p resina-resolver --bin resina-inset-contour -- <path>` for an [inset contour request](schemas/inset-contour-request.schema.json). The operation derives bounded inner geometry for portable edge and highlight bands. `python tools/check_inset_contour_backend.py -- <backend-command> -` checks its geometry and diagnostic failures.

Run `cargo run -p resina-resolver --bin resina-key-light -- <path>` for a [key-light request](schemas/key-light-request.schema.json). The operation resolves a physical lighting direction, side offset, and highlight weights for straight edges and supplied normals. `python tools/check_key_light_backend.py -- <backend-command> -` checks its directional output and failures.

Run `cargo run -p resina-resolver --bin resina-extruded-contour -- <path>` for an [extruded contour request](schemas/extruded-contour-request.schema.json). It resolves the swept silhouette as portable lines and elliptical arcs, including physical corner mapping and split curves. `python tools/check_extruded_contour_backend.py -- <backend-command> -` checks the public geometry protocol.

Run `cargo run -p resina-resolver --bin resina-opaque-surface -- <path>` for an [opaque base-state surface request](schemas/opaque-surface-request.schema.json). It resolves one complete static opaque surface appearance from a theme, actual environment and surroundings, and explicit appearance profile. `python tools/check_opaque_surface_backend.py -- <backend-command> -` checks its portable IR and diagnostic failures. The Rust `resolve_opaque_surface` API reuses a compiled theme across surfaces.

The Rust `OpaqueSurfaceIr::sample_paint` API evaluates that surface's opaque paint color at one logical physical point, including directional highlights and tied corner normals. It provides headless reference evidence for the paint law. Static raster evidence uses the separate conformance tool; renderer conformance remains separate work.

Run `cargo run -p resina-motion --bin resina-spring -- <path|->` for a [normalized scalar spring request](schemas/spring-request.schema.json). The [spring contract](spec/38-spring-reference.md) defines all damping regimes, conservative endpoint settling and immediate reduced motion. `python tools/check_spring_backend.py -- <backend-command> -` checks the public protocol. This foundation does not define calibrated material motion profiles or component animation.

The [CPU paint backend](backends/cpu/resina-raster/README.md) consumes validated surface/focus IR and produces bounded straight RGBA8 sRGB pixels synchronously. Its optional PNG feature supplies the [static raster evidence commands](conformance/raster/README.md). It has explicit viewport and sampling controls and linear-light color integration. Rendering remains separate from the headless resolver; this backend does not implement components or certify a toolkit adapter.

The [shared static material scenes](conformance/scenes/README.md) reference the authored Light/Dark themes, environment and appearance profile. Their generator prepares thirty-two portable body/ring and complete focused/rest requests, and CI checks resolved bindings, channel agreement and complete capture bounds on Linux and Windows.

Run `cargo run -p resina-resolver --bin resina-focus-indicator -- <path>` for a [focused surface indicator request](schemas/focus-indicator-request.schema.json), or use `-` for stdin. The Rust `resolve_focus_indicator` API accepts a headless resolution and surface intent directly. `python tools/check_focus_indicator_backend.py -- <backend-command> -` checks the bound state, authored indicator color, contrast, and Tier 0 geometry against a known surrounding color.

Run `cargo run -p resina-resolver --bin resina-focus-ir -- <path>` for a [focus indicator IR request](schemas/focus-ir-request.schema.json). It resolves both boundaries of the exterior navigation ring around the bound shape's directional footprint, preserving concurrent states and guarded focus color. `python tools/check_focus_ir_backend.py -- <backend-command> -` checks the public IR protocol. The Rust `resolve_focus_ir` API reuses a compiled theme.

`FocusIndicatorIr::sample_paint` evaluates the ring's opaque paint without rasterization. The separate `resina-focus-raster` command and Rust `render_focus` API produce [static PNG evidence](conformance/raster/README.md) with a transparent hole and gap. This does not implement focused component behavior or keyboard navigation.

The Web backend's [SVG focus realization](backends/web/resina-svg/README.md) maps the validated IR to exact vector lines/arcs and a compound ring fill. It preserves the complete physical paint bounds and opaque pigment, with no renderer objects added to normative layers. Its public checker runs on Linux and Windows; complete Web components and Lab remain outstanding.

The independent Shell backend's [native QML focus realization](backends/quickshell/resina-qml/README.md) maps that IR to Qt Quick lines/arcs with a software rendering fallback. Its command and public checker run on Linux and Windows; native Quickshell rendering is exercised separately on Linux. Complete Shell components remain outstanding.

The application backend's [native Slint focus realization](backends/slint/resina-slint/README.md) maps the same IR to native paths with opposite contour winding. Its command and public checker run on Linux and Windows; Linux CI also verifies released Slint software screenshots against independent ring geometry. Complete application components remain outstanding.

The reference renderer's [GUIdo paint preparation](backends/guido/resina-guido/README.md) prepares complete Tier 0 surface and focus images before exposure. Its isolated Linux workspace pins a verified upstream image-cache correction. Required GPU CI checks all thirty-two authored scenes across four device scales, including first-frame paint, source replacement and navigation removal. Complete reference components and advanced material effects remain outstanding.

The [surface paint IR](spec/39-surface-paint-ir.md) resolves body and navigation channels as one complete headless result from a single semantic snapshot. Run `resina-surface-paint <path|->` after building the resolver binaries. Focused input requires the actual opaque surrounding color; a body or ring failure publishes no partial appearance. Its external checker runs on Linux and Windows.

The [rectangular hit-region IR](spec/40-hit-region-ir.md) expands visual bounds to the environment and component minimum, rejects clipping and overlap, and defines shared-edge membership. Run `resina-hit-region <path|->` after building the resolver binaries. `python tools/check_hit_region_backend.py -- <backend-command> -` checks the public placement and failure cases. This supplies geometry for controls; component interaction remains separate work.

The [sequential focus operation](spec/41-focus-traversal.md) chooses a target from an explicit ordered scope, eligibility, current identifier, direction and wrapping policy. Run `resina-focus-traversal <path|->`; `python tools/check_focus_traversal_backend.py -- <backend-command> -` checks its public protocol. This interaction result is separate from paint IR and does not apply native focus or activation.

The [command activation lifecycle](spec/42-command-activation.md) resolves primary pointer gestures, Space/Enter, cancellation, availability and semantic invocation into next state, momentary pressed feedback, capture changes and one activation intent. Run `resina-activation <path|->`; `python tools/check_activation_backend.py -- <backend-command> -` checks the portable protocol. Native event routing, product action delivery and full component appearance remain separate responsibilities.

[Opaque command paint](spec/43-command-paint.md) resolves authored rest, hover, pressed and disabled surface endpoints with independent focus, actual capability fallback and checked content contrast. This is portable paint evidence; full command components and native gesture adapters remain separate work.

[Scalar spring trajectories](spec/44-spring-trajectories.md) preserve position and velocity when changing a target, with deterministic headless sampling and immediate reduced-motion fallback. Material calibration and animated component contracts remain separate work.

[Sampled command paint](spec/45-command-motion.md) connects retargetable spring state to complete guarded paint, with explicit bounded channel projection and actual reduced-motion feedback. Material calibration and native animation delivery remain separate work.

[Complete command label layout](spec/46-command-label-layout.md) resolves content sizing and bounded reflow from complete producer measurements. Its portable IR preserves resolved typography and offered wrapping width. The GUIdo producer has real Linux font shaping evidence at 100%, 150% and 200% text scaling; native complete label drawing also has required GPU evidence at four device scales. Complete components and accessibility conformance remain pending.

[Ordinary command accessibility](spec/47-command-accessibility.md) resolves complete label names, descriptions, current availability/focus, explicit focusability and semantic invocation availability before native tree mapping. Public headless cases distinguish disabled focus discovery from action availability and momentary feedback from toggle semantics. Native assistive technology delivery and complete Button conformance remain pending.

[Command interaction projection](spec/48-command-states.md) connects current activation and explicit hover to paint state, retaining actual focus through disabled discovery and reflecting gesture cancellation from the shared lifecycle. The public command and external checker verify this boundary independently of native adapters.

[Coherent command snapshots](spec/49-command-snapshot.md) bind validated label and static or sampled paint to current interaction, a stable reserved target and accessible semantics. They reject stale signals, mismatched direction and any content/target/semantic failure before publishing the checked channels. Press feedback cannot move the target under a stationary pointer. Native component delivery remains separate work.

[Binary toggle activation](spec/50-toggle-activation.md) composes the checked on/off value
with the verified gesture lifecycle, preserving selection through cancellation,
availability and focus changes. Run `resina-toggle-activation <path|->` and
`python tools/check_toggle_activation_backend.py -- <backend-command> -` for the
portable protocol. Complete Toggle appearance and native accessibility delivery
remain separate component requirements.

[Toggle interaction projection](spec/51-toggle-states.md) retains checked selection
alongside current activation, hover, body rest and actual focus. Its public
protocol and checker run on Linux and Windows. Checked-state appearance remains
an explicit Toggle requirement; ordinary command paint rejects that state.

[Binary Toggle accessibility](spec/52-toggle-accessibility.md) carries complete
stable labeling, boolean checked state, actual focus and explicit invocation
availability in a portable snapshot. Native tree and action delivery remain
separate conformance requirements.

[Horizontal Toggle part layout](spec/53-toggle-layout.md) allocates authored
track/thumb bounds and distinct logical off/on endpoints, preserving RTL and
stable track ownership. Shape, checked paint and full component target coverage
remain separate requirements.

[Opaque Toggle part paint](spec/54-toggle-part-paint.md) composes checked color
roles with independent interaction response and track-owned focus. Cast, Frost
and Elastomer use existing contrast, geometry and actual-capability fallback
laws. Part paint does not certify an assembled native Toggle.

[Coherent Toggle snapshots](spec/55-toggle-snapshot.md) validate full current
part states, selection, uniform track adjacency, label placement, reserved target
coverage, external-label contrast and binary semantics together before publication.

The GUIdo backend can prepare a checked Toggle snapshot's track, placed thumb
and complete external label as one native command array. Required GPU tests
cover actual themes, selection, interaction/focus replacement, RTL placement,
wrapped/scaled native text and fractional device scales. This paint composition
does not provide native Toggle event or accessibility-tree delivery.

[Checked thumb travel](spec/56-toggle-thumb-travel.md) samples complete Toggle
snapshots with retained spring state, bounded placement, stable targets and
immediate Cast/reduced-motion endpoints. GUIdo consumes the checked placement;
frame scheduling and material calibration remain owner responsibilities.

[Sampled Toggle interaction paint](spec/57-toggle-part-motion.md) retains independent
pigment/depth trajectories while preserving selection and track-only focus. Each
sample reruns contrast and geometry guards before component assembly.

[Bounded Slider values](spec/58-slider-value.md) validate explicit numeric bounds
and current value before resolving normalized progress. The strict portable
protocol and independent rational checker preserve endpoints and diagnose
unrepresentable progress. Steps, complete control interaction, appearance and
accessibility publication remain separate component obligations.

[Slider adjustment](spec/59-slider-adjustment.md) applies explicit numeric intents
to the current bounded value with live enabled/read-only permission, bounded
increment/decrement and distinct accepted/changed results. Native delivery,
discrete steps and complete Slider components remain separate requirements.

[Slider accessibility](spec/60-slider-accessibility.md) preserves complete localized
label/value text, bounded numeric value, explicit orientation and focus state,
and supported numeric actions under live enabled/read-only permission. Native
assistive-technology delivery and complete Slider interaction remain required.

[Slider part allocation](spec/61-slider-layout.md) places independently sized
track and thumb within an explicit outer allocation for both axes, RTL and
explicit numeric endpoint placement. Geometry uses checked bounded value progress
and exact containment; material paint, targets and full gestures remain separate
component obligations.

[Slider position mapping](spec/62-slider-position.md) maps an explicit thumb
origin through current allocation and numeric bounds with live adjustment
permission. Stationary origins preserve exact values; unrepresentable movement
fails diagnostically. Native coordinate conversion and capture remain required.

[Cancellable Slider edits](spec/63-slider-edit.md) retain a complete visible
preview separately from committed value. Completion checks live permission and
the current revision; cancellation or conflict closes the edit and preserves
current committed state. Native gesture routing and styled controls remain required.

[Slider pointer anchoring](spec/64-slider-pointer-anchor.md) maps pointer points
to desired thumb origins while retaining an explicit grab distance or track-jump
center. Anchors persist across previews; incompatible geometry fails explicitly.

[Slider pointer lifecycle](spec/65-slider-pointer-lifecycle.md) composes checked
targets, held anchors and cancellable edits. Continuous routing requires actual
acquisition; a release-only track path supports clicks without dragging. Loss,
conflict or invalidation clears ownership without publishing a product change.

[Discrete Slider stops](spec/66-slider-stops.md) retain explicit allowed values
for index movement and nearest-stop selection. Permission, exact membership and
tie policy are checked; native key delivery remains required.

[Slider value policy](spec/67-slider-value-policy.md) keeps continuous or stopped
admissibility explicit throughout edit and pointer sessions. Allowed previews,
policy conflicts and release-only clicks have headless conformance evidence;
native routing and complete styled controls remain required.

[Slider keyboard adjustment](spec/68-slider-keyboard.md) applies explicit arrow,
endpoint and optional Page policies to live values. Stopped arrows visit adjacent
values; repeat delivery, focus/permission loss and pointer interruption have
headless evidence. Native keyboard and assistive routing remain required.

[Slider value presentation](spec/69-slider-presentation.md) pairs checked committed
and visible values, keeps preview semantics coherent, and adopts keyboard or
semantic adjustments against the product baseline with live permission checks.

[Slider appearance profiles](spec/71-slider-appearance.md) require explicit track
and thumb responses for dragging and read-only feedback, with independent focus
and strict state coherence. Public coefficients are arithmetic fixtures; complete
paint and visual calibration remain required.

[Slider interaction projection](spec/70-slider-states.md) preserves actual focus,
hover, keyboard press and pointer manipulation without treating read-only as
disabled or release-only clicks as continuous dragging.
