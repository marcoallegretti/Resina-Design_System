"""Compare shader captures with the CPU raster references.

Differences are 8-bit steps of the image composited over the surrounding color. Ring pixels are the pixels on a
contour: partly covered in either image, or within one pixel of a step above 20 in the CPU raster, which covers the
edge ring, the silhouette and the side band. The contour ratio is the largest difference above a few steps divided
by the largest step to an eight-connected neighbour, so it measures the ring against its own contrast. The edge offset
is the coverage difference of partly covered pixels in device pixels (a straight edge shifted by d pixels changes its
coverage by d). The interior is fully covered pixels at least two pixels from any contour; it cannot see a missing
ring. The criteria are candidates under evaluation, not an established tolerance class, and the exit status is
non-zero when a capture violates them.
"""
import argparse
import json
import statistics
import sys
from pathlib import Path

from PIL import Image

from stats import percentile

STEP = 20
NOISE = 3
CRITERIA = {"interior": 3, "edge": 0.15, "ratio": 0.35}
NEIGHBOURS = ((1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1))


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
    composited, shading, interior, edge, ring, ratio = [], [], [], [], [], 0.0
    for index, (a, b, over_a, over_b) in enumerate(zip(cpu_pixels, gpu_pixels, cpu_over, gpu_over)):
        composited += [abs(x - y) for x, y in zip(over_a[:3], over_b[:3])]
        difference = largest_step(over_a, over_b)
        if contour[index] and (a[3] or b[3]):
            ring.append(difference)
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
            "interior": spread(interior), "edgePixels": len(edge), "edge": spread(edge), "ring": spread(ring), "contourRatio": ratio}


def verdict(rows):
    """The candidate criteria that the rows violate, as readable strings."""
    failures = []
    interior = max((r["interior"]["max"] for r in rows if r["interior"]), default=0)
    edge = max((r["edge"]["max"] for r in rows if r["edge"]), default=0)
    ratio = max(r["contourRatio"] for r in rows)
    if interior > CRITERIA["interior"]:
        failures.append(f"interior {interior} > {CRITERIA['interior']} steps")
    if edge > CRITERIA["edge"]:
        failures.append(f"edge offset {edge:.3f} > {CRITERIA['edge']} px")
    if ratio > CRITERIA["ratio"]:
        failures.append(f"contour ratio {ratio:.3f} > {CRITERIA['ratio']}")
    return failures


def summary(rows):
    print(f"{'group':<14}{'n':>4} | {'contour ratio':>13} | {'ring pixels RGB':^15} | {'edge offset, device px':^19} | "
          f"{'interior RGB (excludes the ring band)':^22} | {'all pixels RGB':^14}")
    print(f"{'':<18} | {'max':>13} | {'max':>7}{'p99':>8} | {'max':>9}{'mean':>10} | {'max':>10}{'mean':>12} | {'max':>7}{'p99':>7}")
    groups = {}
    for row in rows:
        for key in ((row["component"], row["scale"]), ("all", row["scale"])):
            groups.setdefault(key, []).append(row)
    for (component, scale), items in sorted(groups.items()):
        def pick(kind, member, reducer=max):
            present = [r[kind][member] for r in items if r[kind]]
            return reducer(present) if present else float("nan")
        print(f"{component + ' @' + str(scale):<14}{len(items):>4} | {max(r['contourRatio'] for r in items):>13.3f} | "
              f"{pick('ring', 'max'):>7.0f}{pick('ring', 'p99'):>8.0f} | {pick('edge', 'max'):>9.3f}"
              f"{pick('edge', 'mean', statistics.fmean):>10.3f} | {pick('interior', 'max'):>10.0f}"
              f"{pick('interior', 'mean', statistics.fmean):>12.3f} | {pick('composited', 'max'):>7.0f}{pick('composited', 'p99'):>7.0f}")
    failures = verdict(rows)
    print("candidate criteria (interior <= %d steps, edge offset <= %.2f px, contour ratio <= %.2f): %s" % (
        CRITERIA["interior"], CRITERIA["edge"], CRITERIA["ratio"], "FAIL: " + "; ".join(failures) if failures else "pass"))


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
    return 1 if verdict(rows) else 0


if __name__ == "__main__":
    sys.exit(main())
