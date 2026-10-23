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
| `shaders/material.frag` | One fragment shader (GLSL 440, compiled with `qsb --qt6`) for a rounded body: signed-distance outline, swept side plane, edge ring, analytic relief normal, key, ambient and interaction light, analytic edge coverage, and an optional soft contact shadow. Compiled with `-D SUPERSAMPLE` it takes 4 by 4 point samples per pixel like the CPU raster. |
| `qml/MaterialBody.qml` | A `ShaderEffect` whose properties are the per-object uniforms. |
| `qml/capture.qml` | Draws every non-focused lit control scene at scales 1 and 2 and saves the pixels. |
| `qml/bench.qml` | N objects whose size and position spring every frame; records frame intervals. |
| `qml/baseline.qml` | N production `ResinaCommandControl` instances in motion, for the same measurement through the CPU raster path. |
| `tools/dump_scenes.py` | Writes the shader parameters of each scene from the independent lit body arithmetic and the CPU raster reference images. |
| `tools/compare.py` | Compares captures with the CPU raster and with each other and checks the candidate criteria. |
| `run_spike.py` | Runs `bench`, `baseline` and `conformance`. |

The shader is driven only by values a resolved control already carries: size, corner radii, edge width, depth,
relief kind, amplitude and shoulder width, key direction, ambient and interaction light, and the body, highlight,
side and edge colors. The key direction is the unit vector the resolver publishes for the scene (`-0.3401`,
`-0.9404`). The contact shadow is outside the CPU oracle, which has none, and is off in every comparison.

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

`conformance` builds the CPU rasters, renders the 76 non-focused lit control scenes at scales 1 and 2 (scale 2 is a twofold `grabToImage` of an output at scale 1) through each
renderer, and prints the difference from the CPU raster and between renderers. The configurations `opengl-supersampled`, `opengl-shadow` (capture only) and `opengl-no-ring` select the 4 by 4 shader variant, the contact shadow and a negative control that omits the edge ring; the control must fail the comparison or the stage exits non-zero. `baseline` needs the compiled
`Resina.Runtime` module and the pinned Inter masters used by the other Quickshell checks.

## Results

Measured on Linux x86_64 with 14 logical CPUs (Intel Core Ultra 7 255U), Mesa 26.2 with LLVM 23, released Quickshell
0.3.2, Qt 6.11.2, headless sway 1.12 with the pixman renderer, a 1024 by 600 output at scale 1 and 2048 by 1200 at
scale 2 (a real compositor scale of 2 for the frame-time tables). There is no GPU in these runs: OpenGL is Mesa
llvmpipe and Vulkan is Mesa lavapipe, so the shader executes on the CPU. The frame-time tables show how each path
scales with the object count on a CPU rasterizer; they say nothing about a GPU, whose upload, command submission and
compositing costs are not bounded by them. The frame-time tables were taken before master changed the key direction of the lit scenes; the shader cost does not depend on it. The fidelity tables are from the current master.

### Which Qt Quick paths draw the shader

| Configuration | Result |
| --- | --- |
| `QT_QUICK_BACKEND=software` (the configuration of the existing checks) | `ShaderEffect` draws nothing: every scene capture is empty. `GraphicsInfo.api` reads Software. `ShaderEffect.status` read Compiled at the end of the run. |
| `QT_QPA_PLATFORM=offscreen`, any `QSG_RHI_BACKEND` | Qt falls back to the software scene graph ("Loading backend software"). |
| `QT_QPA_PLATFORM=eglfs`, `linuxfb`, `minimalegl` | Fail to start without a DRM device or X display. |
| headless sway (pixman), Qt RHI OpenGL on llvmpipe | Draws; this is the Qt default there. |
| headless sway (pixman), Qt RHI Vulkan on lavapipe | Draws. |

`ShaderEffect.status` is not a usable probe: it read Compiled under the software scene graph, and under OpenGL and
Vulkan it read Uncompiled at the end of the 1-object runs and Compiled at 10 objects and more, although every object
was drawn. `GraphicsInfo.api` separates the software scene graph from the others.

### Fidelity against the CPU raster

76 scenes at scales 1 and 2 (30 Command, 46 Toggle track and thumb), differences in 8-bit steps of the image
composited over the surrounding color. Scale 2 here is a twofold `grabToImage` of an output at scale 1, not a
compositor scale of 2. Ring pixels are the pixels on a contour (partial coverage, or next to a step above 20 in the
CPU raster): the edge ring, the silhouette and the side band.

| shader variant, OpenGL llvmpipe | contour ratio (max) | ring pixels RGB (max / p99 of a scene) | edge offset, device px (max / mean) | interior RGB (max) |
| --- | --- | --- | --- | --- |
| analytic coverage, scale 1 | 0.284 | 35 / 33 | 0.122 / 0.030 | 2 |
| analytic coverage, scale 2 | 0.275 | 25 / 25 | 0.129 / 0.033 | 2 |
| 4 by 4 samples, scale 1 | 0.102 | 9 / 1 | 0.063 / 0.000 | 1 |
| 4 by 4 samples, scale 2 | 0.179 | 20 / 4 | 0.063 / 0.000 | 1 |
| negative control, no edge ring, scale 1 | 1.357 | 149 / 149 | 0.122 / 0.030 | 2 |
| negative control, no edge ring, scale 2 | 7.400 | 149 / 149 | 0.129 / 0.033 | 2 |

The contour ratio is the largest difference above 3 steps divided by the largest step to a neighbouring pixel. The
ring pixels and the contour ratio are what the comparison has to be judged on. The interior is fully covered pixels at
least two pixels from a contour and excludes the ring band: the negative control, a shader that omits the edge ring,
scores the same interior and edge offset as the real one and is caught only by the ring pixels and the contour ratio.
The analytic shader differs from the raster by up to 35 steps on the ring (about a quarter of the local contrast)
because its coverage is analytic and the raster's is 16 point samples. The 4 by 4 variant removes most of that, at
sixteen times the shading work (cost not measured); the remaining 9 to 20 steps in a few scenes are not explained.

The candidate criteria in `tools/compare.py` (interior at most 3 steps, edge offset at most 0.15 px, contour ratio at
most 0.35) are a proposal for evaluation, not an established tolerance class. The analytic shader and the 4 by 4
variant pass them and the negative control fails them. An absolute bound on the ring error tighter than about 35 steps
would need the 4 by 4 variant or an equivalent filter; none has been defined.

OpenGL on llvmpipe and Vulkan on lavapipe differ from each other by at most 1 step and 0.000 px of edge offset. Two runs on the same
renderer are byte-identical.

### Frame times, shader path

`run_spike.py bench`: objects of 160 by 80 units whose size and position follow a closed-form damped spring evaluated
in JavaScript every frame, with no text, resolver or input, laid out in a 824 by 480 area (overlapping from 30 objects
on). Intervals in milliseconds between swapped frames, median / p95 / p99, with the frame count when below 300. The
60 Hz output paces the cheap rows at 16 to 17 ms, so the interval hides any headroom; there is no unpaced companion
metric.

| N | OpenGL scale 1 | Vulkan scale 1 | OpenGL scale 2 | Vulkan scale 2 |
| --- | --- | --- | --- | --- |
| 1 | 17 / 20 / 20 | 17 / 18 / 20 | 18 / 22 / 26 | 18 / 19 / 22 |
| 10 | 16 / 18 / 20 | 17 / 18 / 19 | 21 / 28 / 32 | 17 / 20 / 24 |
| 30 | 16 / 17 / 18 | 16 / 17 / 18 | 53 / 60 / 102 (204 frames) | 54 / 99 / 176 (188) |
| 100 | 50 / 101 / 114 (216) | 45 / 51 / 66 (245) | 166 / 213 / 315 (50) | 182 / 243 / 279 (46) |
| 300 | 141 / 282 / 306 (62) | 130 / 258 / 268 (74) | 540 / 578 / 578 (5) | 482 / 505 / 505 (5) |

### Frame times, current CPU raster path

`run_spike.py baseline`: N production `ResinaCommandControl` instances (160 by 80 units), each the complete control with
resolution, raster, PNG encoding and decoding, label, hit region and accessibility, whose availability is toggled
every 120 ms so that every control is always moving. Each frame is prepared on the GUI thread. The motion is the
Command's body and depth springs, not size or position, so this measures the cost per moving production control and
not identical motion to the shader table. Frames per second over the measurement window (at most 30 s), with the
median interval and the frame count:

| N | OpenGL scale 1 | OpenGL scale 2 | software scene graph scale 1 | software scene graph scale 2 |
| --- | --- | --- | --- | --- |
| 1 | 60.8 fps, 17 ms (300 frames) | 57.9 fps, 17 ms (300) | 60.9 fps, 16 ms (300) | 58.5 fps, 16 ms (300) |
| 10 | 20.2 fps, 71 ms (300) | 11.3 fps, 111 ms (300) | 9.5 fps, 87 ms (279) | 6.6 fps, 122 ms (190) |
| 30 | 7.6 fps, 222 ms (217) | 4.1 fps, 386 ms (112) | 3.1 fps, 252 ms (86) | 1.8 fps, 428 ms (46) |
| 100 | 2.3 fps, 42 ms median, p95 1679 ms (58) | 1.2 fps, 68 ms median, p95 2938 ms (26) | 0.8 fps, 898 ms (18) | 0.2 fps, 4592 ms (4) |

Motion publications in the measurement window: with OpenGL at scale 1, 447 in 15.5 s for 10 controls, 323 in 30.1 s
for 30 and 1409 in 30.7 s for 100; with the software scene graph at scale 1, 711 in 30.1 s for 10 controls (2.4 per
control per second), and at scale 2, 90 in 30.2 s (0.3 per control per second).

Like for like, OpenGL on llvmpipe, frames per second at N of 1, 10, 30 and 100. At scale 1 the shader reaches
59.2 / 60.6 / 60.8 / 19.8 and the production control 60.8 / 20.2 / 7.6 / 2.3. At scale 2 the shader reaches
55.9 / 46.8 / 18.7 / 5.9 and the production control 57.9 / 11.3 / 4.1 / 1.2. The software scene graph rows apply only
to the production control, because the shader draws nothing there.

### On a GitHub-hosted runner

The workflow runs `conformance` and `bench` on `ubuntu-24.04` (no GPU; Mesa 25.2 with LLVM 20, llvmpipe and lavapipe)
and writes the tables to its job summary. The job took 5 minutes in run 39997452201 (commit 83ee3c1) and 18 minutes in
a later run, where the package installation step took about 12 minutes. In run 39997452201 the shader drew in headless
sway on both renderers; against the CPU raster the contour ratio was at most 0.284, the ring pixels differed by up to
35 steps (the analytic shader), the edge by at most 0.129 device px and the interior by at most 2 steps. The two
renderers differed by at most 1 step. Frame intervals in milliseconds, median / p95 / p99, with frame counts when
below 300:

| N | OpenGL scale 1 | Vulkan scale 1 | OpenGL scale 2 | Vulkan scale 2 |
| --- | --- | --- | --- | --- |
| 1 | 16 / 17 / 17 | 16 / 17 / 17 | 17 / 17 / 20 | 17 / 18 / 18 |
| 10 | 16 / 17 / 17 | 16 / 17 / 17 | 25 / 28 / 31 | 26 / 28 / 52 |
| 30 | 18 / 29 / 45 | 18 / 22 / 37 | 69 / 74 / 138 (154 frames) | 74 / 78 / 82 (141) |
| 100 | 57 / 61 / 120 (190) | 60 / 65 / 117 (179) | 229 / 448 / 453 (33) | 248 / 489 / 492 (28) |

## What the spike shows, and what it does not

- Shown: one shader reproduces the lit Command body, track and thumb with the same code on OpenGL and Vulkan,
  deterministically on one renderer. Its interior and edge position agree closely with the CPU raster; its ring pixels
  differ by up to about 35 steps because of the coverage model, and a variant that samples like the raster agrees to
  within 20 steps in the worst scene.
- Shown: the comparison can fail. A shader without the edge ring passes the interior and edge-offset criteria and fails
  the ring-based one.
- Shown: on a CPU rasterizer the shader path holds 60 Hz to 30 objects at scale 1, while the production control path
  runs at 20 fps for 10 moving controls with OpenGL and 9.5 fps in the software scene graph.
- Shown: the Qt software scene graph draws no `ShaderEffect`, and `ShaderEffect.status` does not report that. Any GPU
  path needs its own capability test and a CPU raster fallback.
- Not shown: timings on a GPU. Run `bench --host session` on real hardware to fill that in.
- Not shown: focus ring, Frost, Gel, Lens, emergence and deformation realizations, text and icon content, input and
  accessibility. The shader draws the decorative layer only, as the existing view does.
- The contact shadow is a prototype outside the oracle; no number above covers it.
- Fidelity is measured with this shader against this raster on llvmpipe and lavapipe only. Other drivers, GPUs and
  fractional scales are not covered, and no tolerance class is established.
