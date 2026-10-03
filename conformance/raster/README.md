# Static opaque surface and focus raster evidence

The [CPU backend library](../../backends/cpu/resina-raster/README.md) `resina-raster` realizes the [opaque surface paint law](../../spec/36-opaque-surface-ir.md) and [focus ring paint law](../../spec/37-focus-indicator-ir.md), separate from headless resolution. Its optional PNG feature supplies these conformance commands, producing separate static surface and focus ring images from validated Rust IR. The raw-pixel API can also serve a toolkit adapter without image encoding or asynchronous decoding. It does not define Resina intent, certify a renderer, or implement components, layout, content, interaction, motion or optical treatments. GUIdo remains the high-fidelity reference renderer; Quickshell, Slint and Web retain their independent roles.

The [shared material scene catalog](../scenes/README.md) prepares self-contained requests from the authored themes, environment and appearance profile, with a common viewport for all sixteen static body/ring images.

## Coordinates and sampling

Supply an explicit finite physical origin, positive integer image width and height, positive finite pixels per logical unit, and integer samples per axis in `1..=8`. Image rows run downward from the physical origin; physical x runs rightward. The viewport deliberately clips paint outside its rectangle. It does not fit or translate the surface automatically, mirror RTL geometry or assume a background.

For pixel `(x,y)` and grid sample `(i,j)`, evaluate the canonical paint law at `origin + ((x+(i+0.5)/n), (y+(j+0.5)/n))/scale`. A sample contributes coverage exactly when its paint evaluator returns a color: inside the surface silhouette or in the focus ring's outer-minus-inner region. Decode each covered opaque sample into linear-light sRGB, average covered channels, then encode back into sRGB. Alpha is covered samples divided by `n²`. Thus color is straight, not premultiplied; uncovered pixels are `(0,0,0,0)`. Quantize all four channels to nearest integer after multiplication by 255. Material pigment and highlight calculations retain their specified encoded-channel law before raster integration.

This deterministic regular-grid integration is bounded reference evidence, not an analytic area integral. Increasing sampling changes edge coverage and cannot establish visual or accessibility conformance alone. Geometry and semantic assertions remain necessary alongside critical image review and eventual perceptual comparison.

The CPU backend can omit repeated point queries when a conservative geometric proof establishes identical paint at every sample in a pixel. Uniform body pixels preserve repeated addition before color conversion and quantization; clear pixels remain zero RGBA. Uncertain regions retain ordinary point sampling. The requested grid and sample budget remain unchanged. Tests compare this realization against the point path across geometry, device scales, all sampling grids and RGBA8 rounding thresholds.

## Output and limits

The conformance commands output row-major RGBA8 PNG with an sRGB chunk using perceptual intent. [PNG](https://www.w3.org/TR/png-3/) defines straight alpha and intensity-domain compositing; the [Resina conversion contract](../../spec/04-color-conversion.md) defines decoding and encoding. Encoding belongs to the optional `png` feature, not normative IR. Color, model and resolver layers acquire no image-format dependency; raw-pixel builds can disable the default feature to exclude PNG entirely.

Maximum output is 4,194,304 pixels (16 MiB raw RGBA), with at most 16,777,216 paint samples per call. Reject excessive requests before allocation or sampling. Reject coordinate ranges when endpoints are nonfinite, the viewport has no representable extent, or sample spacing is at most four machine epsilons times the larger absolute axis endpoint. This conservative precision guard avoids collapsed sample positions. Allocation, paint, conversion and output failures are reported explicitly. These limits belong to this CPU realization, not normative Resina IR or requirements on every backend.

## CLI

```sh
cargo run --release -p resina-raster --bin resina-surface-raster -- \
  conformance/ir/opaque-surface-request.json 48 36 -2 -2 2 4 > surface.png
```

The arguments are request path (or `-` for stdin), pixel width, pixel height, physical origin x/y, pixels per unit, and samples per axis. Requests have a 1 MiB limit and use the existing strict resolution protocol. Validate and render fully before writing any PNG bytes. Diagnostics go to stderr; output transport failures may leave a partial stream and return failure. Use a binary-preserving redirect or process API, particularly on Windows shells.

`resina-focus-raster` uses the same arguments with a [focus IR request](../../schemas/focus-ir-request.schema.json). Its image contains only the immediate opaque navigation ring, preserving transparent hole, gap and exterior. It does not resolve a focused surface body, lay out content or handle keyboard navigation. Use matching origins and scales when positioning surface and focus images. A conforming consumer must reserve the full focus paint bounds; this tool's explicit viewport may intentionally crop them for diagnostic tests.

```sh
cargo run --release -p resina-raster --bin resina-focus-raster -- \
  conformance/ir/focus-ir-request.json 64 52 -6 -6 2 4 > focus.png
```
