# Static opaque surface raster evidence

`resina-raster` is conformance tooling for the [opaque surface paint law](../../spec/36-opaque-surface-ir.md), separate from headless resolution. It produces static surface images from validated Rust IR. It does not define Resina intent, certify a renderer, or implement components, layout, content, focus, interaction, motion or optical treatments. GUIdo remains the high-fidelity reference renderer; Quickshell, Slint and Web retain their independent roles.

## Coordinates and sampling

Supply an explicit finite physical origin, positive integer image width and height, positive finite pixels per logical unit, and integer samples per axis in `1..=8`. Image rows run downward from the physical origin; physical x runs rightward. The viewport deliberately clips paint outside its rectangle. It does not fit or translate the surface automatically, mirror RTL geometry or assume a background.

For pixel `(x,y)` and grid sample `(i,j)`, evaluate the canonical paint law at `origin + ((x+(i+0.5)/n), (y+(j+0.5)/n))/scale`. Outside the silhouette contributes zero coverage. Decode each covered opaque sample into linear-light sRGB, average covered channels, then encode back into sRGB. Alpha is covered samples divided by `n²`. Thus color is straight, not premultiplied; uncovered pixels are `(0,0,0,0)`. Quantize all four channels to nearest integer after multiplication by 255. Material pigment and highlight calculations retain their specified encoded-channel law before raster integration.

This deterministic regular-grid integration is bounded reference evidence, not an analytic area integral. Increasing sampling changes edge coverage and cannot establish visual or accessibility conformance alone. Geometry and semantic assertions remain necessary alongside critical image review and eventual perceptual comparison.

## Output and limits

Output is row-major RGBA8 PNG with an sRGB chunk using perceptual intent. [PNG](https://www.w3.org/TR/png-3/) defines straight alpha and intensity-domain compositing; the [Resina conversion contract](../../spec/04-color-conversion.md) defines decoding and encoding. Encoding belongs to this tool, not normative IR. The PNG crate is isolated here; color, model and resolver layers acquire no image-format dependency.

Maximum output is 4,194,304 pixels (16 MiB raw RGBA), with at most 16,777,216 paint samples per call. Reject excessive requests before allocation or sampling. Reject coordinate ranges when endpoints are nonfinite, the viewport has no representable extent, or sample spacing is at most four machine epsilons times the larger absolute axis endpoint. This conservative precision guard avoids collapsed sample positions. Allocation, paint, conversion and output failures are reported explicitly. These limits belong to this reference tool, not backend conformance requirements.

## CLI

```sh
cargo run --release -p resina-raster --bin resina-surface-raster -- \
  conformance/ir/opaque-surface-request.json 48 36 -2 -2 2 4 > surface.png
```

The arguments are request path (or `-` for stdin), pixel width, pixel height, physical origin x/y, pixels per unit, and samples per axis. Requests have a 1 MiB limit and use the existing strict resolution protocol. Validate and render fully before writing any PNG bytes. Diagnostics go to stderr; output transport failures may leave a partial stream and return failure. Use a binary-preserving redirect or process API, particularly on Windows shells.
