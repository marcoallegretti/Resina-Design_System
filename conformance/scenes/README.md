# Static material scenarios (candidate, 0.1.0)

Blueprint §§128–130 require shared scenarios, semantic and geometry assertions, and image review. The [Tier 0 catalog](tier0-materials.json) provides thirty-two explicit scenes: Light and Dark themes, Cast/Frost/Elastomer/Gel, each with a resting opaque body, a separate immediate focus ring, complete focused paint and complete rest paint. These are static slices of the existing contracts, not components, input interaction scenarios or backend certification.

The [manifest schema](../../schemas/material-scene-manifest.schema.json) defines repository-root relative JSON asset paths, explicit theme sources and external-source identifier mappings, actual environment and appearance source, common logical surface size and image capture, and named scenarios. Resolve asset paths within the repository, including symlink targets; do not fetch URLs or infer platform settings. Names are unique; each scene names a declared theme. The catalog uses the authored theme/profile assets directly rather than copying their numerical choices into a second palette or profile.

Each scenario supplies a complete surface intent and expected material family. `opaqueSurface` requires exactly `rest` and explicit foreground role and minimum content/edge contrast. `focusRing` requires `focused`, preserves concurrent states, and rejects body-only contrast/foreground fields. Existing input versions remain unchanged; manifest version is `0.1.0`. Unsupported body states do not silently resolve as resting state.

## Preparing requests

`surfacePaint` requires the same foreground and contrast fields as an opaque body,
and permits only `rest`, `focused`, or both. Each state set is explicitly authored
and preserved; complete requests are not inferred by stripping states from a ring
scene. Focused/rest pairs follow each other in the catalog for replacement checks.

Compile and resolve each declared theme against the supplied environment using the [theme protocol](../../spec/22-theme-compilation.md). Verify the resolved material role agrees with the declared family. Resolve `surroundingColorRole` from that theme's **opaque** color fallbacks. This chooses the actual uniform surrounding color for this authored scene; it does not infer a real application's backdrop.

For an opaque body, assemble the [opaque surface request](../../schemas/opaque-surface-request.schema.json), using the complete appearance profile and resolved surroundings as `adjacentColor`. Supply the same known surroundings as `postTreatmentBackdrop` for Frost. For a ring, assemble the [focus IR request](../../schemas/focus-ir-request.schema.json), using the profile's complete shape/depth assignments and key light and the same resolved surroundings. Preserve theme source text, external source identifiers/text, environment, size, surface intent and all declared guards. Do not force capabilities or strip accessibility preferences to obtain opacity.

```sh
python tools/material_scenarios.py -- target/debug/resina-theme-resolve - > scenes.json
```

For complete paint, assemble the body request from the scene's own intent and
guards, then wrap it as `body` in a [surface paint request](../../schemas/surface-paint-request.schema.json)
with version `0.1.0` and the resolved actual `surroundingColor`. Both channels
resolve from the same semantic snapshot. An unfocused result omits navigation.

The generator accepts a theme backend command after `--` and emits one UTF-8 JSON bundle only after all scenes prepare successfully. It has no image or UI dependency. The bundle has version `0.1.0`, the common `capture`, and ordered `scenarios` containing `name`, `kind`, `expectedMaterialFamily` and a complete self-contained `request`. Each request can be fed directly to the corresponding existing IR or raster protocol; no repository asset lookup is then needed. Backend failure, diagnostics, malformed/duplicate JSON, schema errors, missing assets, unknown themes, duplicate names, state misuse or mismatched material families fail explicitly without a partial bundle.

## Evidence and capture

```sh
python tools/check_material_scenarios.py \
  --theme-backend target/debug/resina-theme-resolve \
  --surface-backend target/debug/resina-opaque-surface \
  --focus-backend target/debug/resina-focus-ir \
  --paint-backend target/debug/resina-surface-paint
```

The checker resolves every generated request, validates its IR, checks material/form/color/state and foreground-role preservation, checks reported contrast against the declared guards with the existing `1e-12` absolute numeric tolerance, and verifies the capture contains all required paint bounds with the existing absolute or relative `1e-12` geometry tolerance. Focus capture uses the complete translated outer boundary, including the side footprint. CI runs this against Rust on Linux and Windows. Other language implementations can consume the same requests and contracts independently.

Complete paint checks body and navigation channels separately against the authored
intent and capture, and checks their shared binding and swept silhouette. Missing
required navigation, unexpected inactive navigation, cross-channel mismatch,
failed body guards or clipping either channel fails the scene.

`capture` supplies physical origin, pixel dimensions, pixels per logical unit and the [reference raster tool's](../raster/README.md) samples per axis. Use the same viewport when comparing images. Other renderers may use analytic coverage or their own antialiasing; the sample grid is reference evidence, not a normative renderer technique. `opaqueSurface` and `focusRing` retain separate diagnostic images; `surfacePaint` renders complete body/navigation paint in one image. The separate rings retain their authored concurrent selection signal without claiming selected body appearance. Image metrics, perceived material identity, component typography/layout, navigation behavior and accessibility still need further conformance evidence; passing this catalog does not certify them.
