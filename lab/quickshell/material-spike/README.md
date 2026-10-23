# Material renderer spike

This directory is a feasibility spike for part of #125. It is not production code, it is not normative, and it
changes no specification, definition, schema, token or conformance contract. Nothing here is a Resina Core
artifact, and nothing depends on it. It exists to answer one question with measurements: can a per-object
fragment shader in Qt Quick, running in released Quickshell, draw the lit Command body, Toggle track and Toggle
thumb well enough, fast enough and verifiably enough to be the production material renderer, with the CPU raster as
the oracle and as the fallback?

The numbers below were produced by the scripts in this directory. The CI job in
`.github/workflows/material-spike.yml` reruns the comparison and the benchmark on a GitHub runner and writes the same
tables to its job summary.

## What is here

| File | Purpose |
| --- | --- |
| `shaders/material.frag` | One fragment shader (GLSL 440, compiled with `qsb --qt6`) for a rounded body: signed-distance outline, swept side plane, edge ring, analytic relief normal, key, ambient and interaction light, analytic edge coverage, and an optional soft contact shadow. |
| `qml/MaterialBody.qml` | A `ShaderEffect` whose properties are the per-object uniforms. |
| `qml/capture.qml` | Draws every non-focused lit control scene at scales 1 and 2 and saves the pixels. |
| `qml/bench.qml` | N objects whose size and position spring every frame; records frame intervals. |
| `qml/baseline.qml` | N production `ResinaCommandControl` instances in motion, for the same measurement through the CPU raster path. |
| `tools/dump_scenes.py` | Writes the shader parameters of each scene from the independent lit body arithmetic and the CPU raster reference images. |
| `tools/compare.py` | Compares captures with the CPU raster and with each other. |
| `run_spike.py` | Runs `bench`, `baseline` and `conformance`. |

The shader is driven only by values a resolved control already carries: size, corner radii, edge width, depth,
relief kind, amplitude and shoulder width, key direction, ambient and interaction light, and the body, highlight,
side and edge colors. The key direction is the unit vector the resolver publishes for the scene (`-0.7071`,
`-0.7071`). The contact shadow is outside the CPU oracle, which has none, and is off in every comparison.

## Run it

Requirements: released Quickshell 0.3.2 built against Qt 6.11.x, the Qt `qsb` tool, a Wayland session, Python 3 with
Pillow, and for `conformance` a Rust toolchain.

```sh
python3 lab/quickshell/material-spike/run_spike.py bench --quickshell /path/to/quickshell --qsb /path/to/qsb
```

This opens a private headless sway session per output scale and prints one table row per renderer, scale and object
count: the swap-to-swap interval (median, 95th and 99th percentile) and the frame rate. On a machine with a GPU, run
it in the running session so that the driver and compositor are the real ones:

```sh
python3 lab/quickshell/material-spike/run_spike.py bench --host session --configs default \
  --counts 1 10 30 100 300 --scales 1 --quickshell /path/to/quickshell --qsb /path/to/qsb
```

`--configs` takes `opengl`, `vulkan`, `software` (the Qt software scene graph) and `default` (whatever Qt picks).
`--output FILE` also writes the rows as JSON.

The other two stages:

```sh
python3 lab/quickshell/material-spike/run_spike.py conformance --quickshell ... --qsb ... --configs opengl vulkan
python3 lab/quickshell/material-spike/run_spike.py baseline --quickshell ... --qsb ... \
  --module-path DIR_WITH_Resina.Runtime --font-dir DIR_WITH_PINNED_INTER --configs software opengl
```

`conformance` builds the CPU rasters, renders the 76 non-focused lit control scenes at scales 1 and 2 through each
renderer, and prints the difference from the CPU raster and between renderers. `baseline` needs the compiled
`Resina.Runtime` module and the pinned Inter masters used by the other Quickshell checks.

## Results

Measured on Linux x86_64 with 14 logical CPUs (Intel Core Ultra 7 255U), Mesa 26.2 with LLVM 23, released Quickshell
0.3.2, Qt 6.11.2, headless sway 1.12 with the pixman renderer, a 1024 by 600 output at scale 1 and 2048 by 1200 at
scale 2. There is no GPU in these runs: OpenGL is Mesa llvmpipe and Vulkan is Mesa lavapipe, so the shader executes on
the CPU. The numbers show how the shader path scales and where it fails; they are not GPU timings and are an upper
bound on a GPU's cost.

### Which Qt Quick paths draw the shader

| Configuration | Result |
| --- | --- |
| `QT_QUICK_BACKEND=software` (the configuration of the existing checks) | `ShaderEffect` draws nothing: every scene capture is empty (edge offset 1.0 px, no covered pixel). `ShaderEffect.status` still reads Compiled; `GraphicsInfo.api` reads Software. |
| `QT_QPA_PLATFORM=offscreen`, any `QSG_RHI_BACKEND` | Qt falls back to the software scene graph ("Loading backend software"). |
| `QT_QPA_PLATFORM=eglfs`, `linuxfb`, `minimalegl` | Fail to start without a DRM device or X display. |
| headless sway (pixman), Qt RHI OpenGL on llvmpipe | Draws; this is the Qt default there. |
| headless sway (pixman), Qt RHI Vulkan on lavapipe | Draws. |

### Fidelity against the CPU raster

76 scenes at scale 1 and 2 (30 Command, 46 Toggle track and thumb). Differences are 8-bit steps of the image
composited over the surrounding color.

| | all pixels (max / p99 / mean) | interior (max / p99 / mean) | edge offset, device px (max / mean) | contour ratio |
| --- | --- | --- | --- | --- |
| scale 1 | 34 / 11 / 0.24 | 1 / 1 / 0.005 | 0.118 / 0.030 | 0.264 |
| scale 2 | 25 / 8 / 0.12 | 2 / 1 / 0.006 | 0.122 / 0.033 | 0.219 |

The interior is every fully covered pixel at least two pixels from a contour. The edge offset is the coverage
difference of partly covered pixels, which equals the position error of a straight edge. The contour ratio is the
largest difference above 3 steps divided by the largest step to a neighbouring pixel. The largest differences sit on
the high-contrast edge ring. The CPU raster quantizes coverage to 16 samples, which is a part of the offset.

OpenGL on llvmpipe and Vulkan on lavapipe differ from each other by at most 2 steps and 0.004 px. Two runs on the same
renderer are byte-identical.

### Frame times, shader path

`run_spike.py bench`, objects of 160 by 80 units springing in size and position every frame, laid out in a
824 by 480 area (overlapping for N of 30 and more). Intervals in milliseconds, median / p95 / p99; 300 frames unless
fewer are shown.

| N | OpenGL scale 1 | Vulkan scale 1 | OpenGL scale 2 | Vulkan scale 2 |
| --- | --- | --- | --- | --- |
| 1 | 17 / 20 / 20 | 17 / 18 / 20 | 18 / 22 / 26 | 18 / 19 / 22 |
| 10 | 16 / 18 / 20 | 17 / 18 / 19 | 21 / 28 / 32 | 17 / 20 / 24 |
| 30 | 16 / 17 / 18 | 16 / 17 / 18 | 53 / 60 / 102 (204 frames) | 54 / 99 / 176 (188 frames) |
| 100 | 50 / 101 / 114 (216 frames) | 45 / 51 / 66 (245 frames) | 166 / 213 / 315 (50 frames) | 182 / 243 / 279 (46 frames) |
| 300 | 141 / 282 / 306 (62 frames) | 130 / 258 / 268 (74 frames) | 540 / 578 / 578 (5 frames) | 482 / 505 / 505 (5 frames) |

The 60 Hz output paces the cheap rows at 16 to 17 ms. The rows above that are the CPU running the fragment shader,
which a GPU would do in a small fraction of the time.

### Frame times, current CPU raster path

`run_spike.py baseline`, N production `ResinaCommandControl` instances (160 by 80 units) whose availability toggles
every 120 ms so every control is always moving, Qt software scene graph and Qt RHI OpenGL on llvmpipe. Each control
resolves, rasterizes and encodes a frame on the GUI thread. Median / p95 / p99 in milliseconds, frames measured in
brackets. The OpenGL rows for 100 controls alternate between bursts and stalls of over a second, hence the wide spread.

| N | software scale 1 | software scale 2 | OpenGL scale 1 | OpenGL scale 2 |
| --- | --- | --- | --- | --- |
| 1 | 16 / 26 / 32 (300) | 16 / 29 / 38 (300) | 17 / 23 / 28 (300) | 17 / 28 / 31 (300) |
| 10 | 87 / 227 / 250 (279) | 122 / 336 / 356 (190) | 71 / 149 / 175 (300) | 111 / 140 / 252 (300) |
| 30 | 252 / 724 / 785 (86) | 428 / 850 / 1216 (46) | 222 / 261 / 473 (217) | 386 / 449 / 469 (112) |
| 100 | 898 / 2584 / 2584 (18) | 4592 / 4659 / 4659 (4) | 42 / 1679 / 1715 (58) | 68 / 2938 / 4535 (26) |

Publications in about 30 s for 10 controls: 711 (software, scale 1) and 90 (software, scale 2), that is 2.4 and 0.3
per control per second. For 100 controls: 106 and 100 in 30 s, about one per control per 30 s.

## What the spike shows, and what it does not

- Shown: a single shader of about 170 lines reproduces the lit Command body, track and thumb within the figures
  above, with the same code on OpenGL and Vulkan, deterministically on one renderer.
- Shown: the shader path holds the 60 Hz pace to 30 objects at scale 1 on a CPU rasterizer, where the CPU raster path
  already drops to 9.5 frames per second for 10 moving controls.
- Shown: the Qt software scene graph draws no `ShaderEffect`, and `ShaderEffect.status` does not report that. Any
  GPU path needs its own capability test and a CPU raster fallback.
- Not shown: timings on a GPU. Run `bench --host session` on real hardware to fill that in.
- Not shown: focus ring, Frost, Gel, Lens, emergence and deformation realizations, text and icon content, input and
  accessibility. The shader draws the decorative layer only, as the existing view does.
- The contact shadow is a prototype outside the oracle; no number above covers it.
- Fidelity is measured with this shader against this raster on llvmpipe and lavapipe only. Other drivers, GPUs and
  fractional scales are not covered.
