"""Compare shader captures with the CPU raster references.

Per scene and scale it reports, in 8-bit steps of the image composited over the surrounding color, the RGB
difference of every pixel, of the fully covered pixels, and of the interior (fully covered pixels at least two
pixels from any contour). Where either image is partly covered it reports the coverage difference as an edge
offset in device pixels (a straight edge shifted by d pixels changes its coverage by d). The contour ratio is the
largest RGB difference above a few steps, divided by the largest step to an eight-connected neighbour.
"""
import argparse
import json
import statistics
import sys
from pathlib import Path

from PIL import Image

STEP = 20
NOISE = 3
NEIGHBOURS = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1))


def percentile(values, fraction):
    ordered = sorted(values)
    return ordered[min(len(ordered) - 1, int(fraction * len(ordered)))]


def spread(values):
    return {"max": max(values), "p99": percentile(values, 0.99), "mean": statistics.fmean(values)} if values else None


def composite(image, surround):
    background = Image.new("RGBA", image.size, tuple(round(c * 255) for c in surround) + (255,))
    background.alpha_composite(image)
    return background


def pixels(image):
    return list(image.get_flattened_data()) if hasattr(image, "get_flattened_data") else list(image.getdata())


def distance_from(flags, size):
    columns, rows = size
    distance = [None] * (columns * rows)
    frontier = [index for index, flag in enumerate(flags) if flag]
    for index in frontier:
        distance[index] = 0
    step = 0
    while frontier:
        step += 1
        following = []
        for index in frontier:
            y, x = divmod(index, columns)
            for dx, dy in NEIGHBOURS:
                nx, ny = x + dx, y + dy
                if 0 <= nx < columns and 0 <= ny < rows and distance[ny * columns + nx] is None:
                    distance[ny * columns + nx] = step
                    following.append(ny * columns + nx)
        frontier = following
    return distance


def largest_step(pixel, other):
    return max(abs(p - q) for p, q in zip(pixel[:3], other[:3]))


def compare(scene, scale, cpu_path, gpu_path):
    cpu = Image.open(cpu_path).convert("RGBA")
    gpu = Image.open(gpu_path).convert("RGBA")
    if cpu.size != gpu.size:
        raise ValueError(f"{scene['name']}@{scale}: size {gpu.size} differs from {cpu.size}")
    columns, rows = cpu.size
    cpu_pixels, gpu_pixels = pixels(cpu), pixels(gpu)
    cpu_over = pixels(composite(cpu, scene["surround"]))
    gpu_over = pixels(composite(gpu, scene["surround"]))
    contour = [a[3] not in (0, 255) or b[3] not in (0, 255) or (a[3] == 255) != (b[3] == 255)
               for a, b in zip(cpu_pixels, gpu_pixels)]
    contrast = [0] * len(cpu_over)
    for index, a in enumerate(cpu_over):
        y, x = divmod(index, columns)
        for dx, dy in NEIGHBOURS:
            if 0 <= x + dx < columns and 0 <= y + dy < rows:
                contrast[index] = max(contrast[index], largest_step(a, cpu_over[index + dy * columns + dx]))
        if contrast[index] > STEP:
            contour[index] = True
    distance = distance_from(contour, cpu.size)
    composited, shading, interior, edge, ratio = [], [], [], [], 0.0
    for index, (a, b, over_a, over_b) in enumerate(zip(cpu_pixels, gpu_pixels, cpu_over, gpu_over)):
        composited += [abs(x - y) for x, y in zip(over_a[:3], over_b[:3])]
        difference = largest_step(over_a, over_b)
        if difference > NOISE:
            ratio = max(ratio, difference / max(contrast[index], 1))
        if a[3] == 255 and b[3] == 255:
            steps = [abs(x - y) for x, y in zip(a[:3], b[:3])]
            shading += steps
            if distance[index] is not None and distance[index] >= 2:
                interior += steps
        elif a[3] or b[3]:
            edge.append(abs(a[3] - b[3]) / 255)
    return {"name": scene["name"], "scale": scale, "component": scene["component"], "family": scene["family"],
            "pixels": len(cpu_pixels), "composited": spread(composited), "shading": spread(shading),
            "interior": spread(interior), "edgePixels": len(edge), "edge": spread(edge), "contourRatio": ratio}


def summary(rows):
    print(f"{'group':<14}{'n':>4} | {'composited RGB':^21} | {'fully covered RGB':^21} | {'interior RGB':^21} | "
          f"{'edge offset, device px':^25} | contour")
    print(f"{'':<18} | {'max':>6}{'p99':>7}{'mean':>8} | {'max':>6}{'p99':>7}{'mean':>8} | {'max':>6}{'p99':>7}{'mean':>8} | "
          f"{'max':>7}{'p99':>8}{'mean':>8} | ratio")
    groups = {}
    for row in rows:
        for key in ((row["component"], row["scale"]), ("all", row["scale"])):
            groups.setdefault(key, []).append(row)
    for (component, scale), items in sorted(groups.items()):
        cells = []
        for kind in ("composited", "shading", "interior", "edge"):
            present = [r[kind] for r in items if r[kind]]
            if not present:
                cells.append(f"{'-':>6}{'-':>7}{'-':>8}")
                continue
            values = (max(p["max"] for p in present), max(p["p99"] for p in present),
                      statistics.fmean(p["mean"] for p in present))
            cells.append(f"{values[0]:>7.3f}{values[1]:>8.3f}{values[2]:>8.3f}" if kind == "edge"
                         else f"{values[0]:>6}{values[1]:>7}{values[2]:>8.3f}")
        print(f"{component + ' @' + str(scale):<14}{len(items):>4} | " + " | ".join(cells)
              + f" | {max(r['contourRatio'] for r in items):.3f}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scenes", required=True, type=Path)
    parser.add_argument("--cpu-dir", required=True, type=Path)
    parser.add_argument("--gpu-dir", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    scenes = json.loads(arguments.scenes.read_text())["scenes"]
    rows = [compare(scene, scale, arguments.cpu_dir / f"{scene['name']}@{scale}.png",
                    arguments.gpu_dir / f"{scene['name']}@{scale}.png") for scene in scenes for scale in (1, 2)]
    if arguments.output:
        arguments.output.write_text(json.dumps(rows, indent=1) + "\n")
    summary(rows)


if __name__ == "__main__":
    sys.exit(main())
