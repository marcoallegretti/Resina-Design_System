"""Compare an opaque face's prepared pixels with an owned nested Wayland capture."""
import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

from PIL import Image
from check_schemas import ROOT, load_json


def diagnostic_request():
    request = copy.deepcopy(load_json(ROOT / "conformance/ir/relief-paint-vectors.json")[7]["request"])
    request["outline"]["radii"] = {
        "topStart": {"x": 20, "y": 12}, "topEnd": {"x": 30, "y": 15},
        "bottomEnd": {"x": 24, "y": 18}, "bottomStart": {"x": 12, "y": 20},
    }
    request["profile"].update(amplitude=0.25, shoulderWidth=1)
    request.update(edgeWidth=0.25, points=[])
    return request


def capture_matches(capture, prepared, origin):
    if origin[0] < 0 or origin[1] < 0 or origin[0] + prepared.width > capture.width or origin[1] + prepared.height > capture.height:
        raise ValueError("native capture clips the complete prepared image")
    count, worst = 0, 0
    for y in range(prepared.height):
        for x in range(prepared.width):
            expected = prepared.getpixel((x, y))
            if expected[3] != 255:
                continue
            actual = capture.getpixel((origin[0] + x, origin[1] + y))
            worst = max(worst, *(abs(expected[c] - actual[c]) for c in range(3)))
            count += 1
    if not count:
        raise ValueError("prepared image has no opaque verification pixels")
    return count, worst


def check_scale(probe, request, output, parent, scale):
    with tempfile.TemporaryDirectory(prefix="resina-relief-native-") as runtime:
        config = Path(runtime) / "config"
        config.write_text(f"xwayland disable\noutput * mode 840x840 scale {scale}\nseat seat0 xcursor_theme default 24\n")
        env = dict(os.environ, XDG_RUNTIME_DIR=runtime, WAYLAND_DISPLAY=str(parent),
                   WLR_BACKENDS="wayland", WLR_RENDERER="pixman", WLR_LIBINPUT_NO_DEVICES="1")
        children = []
        client_path = output / f"client-{scale}.log"
        with (output / f"compositor-{scale}.log").open("w") as compositor_log, client_path.open("w") as client_log:
            try:
                compositor = subprocess.Popen(["sway", "--config", str(config)], env=env,
                                              stdout=compositor_log, stderr=compositor_log)
                children.append(compositor)
                socket, ipc = None, None
                for _ in range(200):
                    sockets = [p for p in Path(runtime).glob("wayland-*") if p.is_socket()]
                    peers = list(Path(runtime).glob("sway-ipc.*.sock"))
                    if sockets and peers:
                        socket, ipc = sockets[0], peers[0]
                        break
                    if compositor.poll() is not None:
                        raise ValueError("nested compositor exited; inspect its saved log")
                    time.sleep(0.05)
                if socket is None or ipc is None:
                    raise ValueError("nested compositor did not publish its sockets")
                env.update(WAYLAND_DISPLAY=str(socket), SWAYSOCK=str(ipc))
                outputs = json.loads(subprocess.check_output(
                    ["swaymsg", "--socket", str(ipc), "-t", "get_outputs"], env=env, text=True))
                if len(outputs) != 1 or outputs[0]["scale"] != scale:
                    raise ValueError("compositor did not adopt the requested single-output scale")
                mode = outputs[0]["current_mode"]
                if (mode["width"], mode["height"]) != (840, 840):
                    raise ValueError("compositor did not adopt the required physical output size")
                prepared_path = output / f"prepared-{scale}.png"
                client = subprocess.Popen([str(probe), str(request), str(scale), str(prepared_path)],
                                          cwd=ROOT, env=env, stdout=client_log, stderr=client_log)
                children.append(client)
                metadata = None
                for _ in range(400):
                    for line in client_path.read_text().splitlines():
                        if line.startswith("RESINA_RELIEF_PREPARED "):
                            metadata = json.loads(line.split(" ", 1)[1])
                    if metadata is not None and prepared_path.exists():
                        break
                    if client.poll() is not None:
                        raise ValueError("native client exited; inspect its saved log")
                    time.sleep(0.05)
                if metadata is None or not prepared_path.exists():
                    raise ValueError("native client did not publish prepared paint")
                if metadata["scale"] != scale:
                    raise ValueError("native fixture preparation scale differs from compositor scale")
                prepared = Image.open(prepared_path).convert("RGBA")
                origin = tuple(round(metadata["origin"][axis] * scale) for axis in ("x", "y"))
                capture_path = output / f"native-{scale}.png"
                for _ in range(100):
                    subprocess.run(["grim", "-o", outputs[0]["name"], str(capture_path)], env=env,
                                   check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
                    capture = Image.open(capture_path).convert("RGBA")
                    if capture.size != (mode["width"], mode["height"]):
                        raise ValueError("native capture resamples the physical output; pixel comparison is unsupported")
                    count, worst = capture_matches(capture, prepared, origin)
                    if worst <= 2:
                        return {"scale": scale, "output": outputs[0]["name"], "comparedOpaquePixels": count,
                                "worstOpaqueDifference": worst,
                                "captureSha256": hashlib.sha256(capture_path.read_bytes()).hexdigest()}
                    if client.poll() is not None:
                        raise ValueError("native client exited before matching captured pixels")
                    time.sleep(0.1)
                raise ValueError(f"scale {scale}: opaque native pixels differ by {worst} (limit 2)")
            finally:
                for child in reversed(children):
                    child.terminate()
                    try:
                        child.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        child.kill()
                        child.wait(timeout=5)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--probe", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--request", type=Path)
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 1.25, 1.5, 2, 3])
    args = parser.parse_args()
    if any(not math.isfinite(s) or s <= 0 or (840 / s) != round(840 / s) for s in args.scales):
        parser.error("each positive scale must divide the 840 px output into integral logical dimensions")
    try:
        parent = Path(os.environ["WAYLAND_DISPLAY"])
        if not parent.is_absolute():
            parent = Path(os.environ["XDG_RUNTIME_DIR"]) / parent
        if not parent.is_socket():
            raise ValueError("parent Wayland socket is absent")
        args.output_dir.mkdir(parents=True)
        request_path = args.output_dir / "request.json"
        request = load_json(args.request) if args.request else diagnostic_request()
        request_path.write_text(json.dumps(request, allow_nan=False))
        records = [check_scale(args.probe.resolve(), request_path.resolve(), args.output_dir.resolve(), parent, scale)
                   for scale in args.scales]
        report = {"records": records, "nestedBackend": "wayland", "renderer": "pixman",
                  "scope": "opaque native face-paint pixels; no appearance, input, focus or AT acceptance"}
        (args.output_dir / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report))
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"FAIL native relief paint: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
