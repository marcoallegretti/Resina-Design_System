"""Run the material renderer spike: frame-time benchmark, CPU-raster baseline and conformance comparison.

    python3 lab/quickshell/material-spike/run_spike.py bench --quickshell QUICKSHELL --qsb QSB
    python3 lab/quickshell/material-spike/run_spike.py baseline --quickshell QUICKSHELL --qsb QSB --module-path DIR --font-dir DIR
    python3 lab/quickshell/material-spike/run_spike.py conformance --quickshell QUICKSHELL --qsb QSB

Every run opens windows in a Wayland session: a private headless sway session by default (--host sway), or the
session already running (--host session), which is how the numbers are taken on real hardware.
"""
import argparse
import json
import os
import re

import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "tools"))
from stats import percentile  # noqa: E402
ROOT = HERE.parents[2]
API = {0: "unknown", 1: "software", 2: "openvg", 3: "opengl", 4: "direct3d11", 5: "vulkan", 6: "metal", 7: "null"}
STATUS = {0: "Uncompiled", 1: "Compiled", 2: "Error"}
CONFIGS = {
    "opengl": {"QSG_RHI_BACKEND": "opengl"},
    "vulkan": {"QSG_RHI_BACKEND": "vulkan"},
    "software": {"QT_QUICK_BACKEND": "software"},
    "default": {},
    "opengl-shadow": {"QSG_RHI_BACKEND": "opengl", "MATERIAL_SPIKE_SHADOW": "1"},
    "opengl-supersampled": {"QSG_RHI_BACKEND": "opengl", "MATERIAL_SPIKE_SHADER": "material-supersampled.frag.qsb"},
    "opengl-no-ring": {"QSG_RHI_BACKEND": "opengl", "MATERIAL_SPIKE_NO_RING": "1"},
}


class Session:
    """A Wayland session for one scale: a private headless sway, or the running one."""

    def __init__(self, host, scale, sway):
        self.process = None
        self.directory = None
        self.environment = dict(os.environ)
        if host == "session":
            return
        self.directory = tempfile.mkdtemp(prefix="material-spike-")
        os.chmod(self.directory, 0o700)
        config = Path(self.directory) / "sway.conf"
        config.write_text(f"output HEADLESS-1 resolution {round(1024 * scale)}x{round(600 * scale)} scale {scale}\n")
        self.environment.update(XDG_RUNTIME_DIR=self.directory, WLR_BACKENDS="headless", WLR_LIBINPUT_NO_DEVICES="1",
                                WLR_RENDERER="pixman")
        self.environment.pop("DISPLAY", None)
        self.environment.pop("WAYLAND_DISPLAY", None)
        self.process = subprocess.Popen([sway, "-c", str(config)], env=self.environment,
                                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            sockets = [name for name in os.listdir(self.directory) if re.fullmatch(r"wayland-\d+", name)]
            if sockets:
                self.environment["WAYLAND_DISPLAY"] = sockets[0]
                return
            time.sleep(0.1)
        self.close()
        raise RuntimeError("sway did not publish a Wayland socket")

    def close(self):
        if self.process:
            self.process.terminate()
            try:
                self.process.wait(5)
            except subprocess.TimeoutExpired:
                self.process.kill()
        if self.directory:
            shutil.rmtree(self.directory, ignore_errors=True)


def prepare(work, qsb):
    work.mkdir(parents=True, exist_ok=True)
    for name in ("MaterialBody.qml", "Recorder.qml", "bench.qml", "baseline.qml", "capture.qml", "bench-scene.js"):
        shutil.copyfile(HERE / "qml" / name, work / name)
    source = str(HERE / "shaders" / "material.frag")
    subprocess.run([qsb, "--qt6", "-o", str(work / "material.frag.qsb"), source], check=True)
    subprocess.run([qsb, "--qt6", "-D", "SUPERSAMPLE", "-o", str(work / "material-supersampled.frag.qsb"), source], check=True)


def launch(quickshell, work, qml, session, environment, timeout):
    env = dict(session.environment)
    env.update(QT_QPA_PLATFORM="wayland", **environment)
    env.pop("QT_SCALE_FACTOR", None)
    try:
        done = subprocess.run([quickshell, "--no-color", "--no-duplicate", "--path", str(work / qml)], env=env,
                              capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return f"timed out after {timeout:.0f} s"
    return done.stdout + done.stderr


def result_of(text):
    for line in text.splitlines():
        if "BENCH_RESULT " in line:
            return json.loads(line.split("BENCH_RESULT ", 1)[1])
    return None


def row(configuration, scale, count, text):
    result = result_of(text)
    if result is None:
        return {"config": configuration, "scale": scale, "n": count, "failed": "no result: " + text[-300:].strip()}
    if result.get("failed") or not result["intervals"]:
        errors = [line for line in text.splitlines() if "BENCH_ERROR" in line or "ERROR" in line or "WARN" in line]
        return {"config": configuration, "scale": scale, "n": count, "failed": result.get("failed", "no frames") + " " + " ".join(errors[:3]),
                "kind": result["kind"]}
    intervals = result["intervals"]
    return {"config": configuration, "scale": scale, "n": count, "kind": result["kind"],
            "api": API.get(result["api"], result["api"]), "dpr": result["dpr"], "frames": len(intervals),
            "intervalMedian": statistics.median(intervals), "intervalP95": percentile(intervals, 0.95),
            "intervalP99": percentile(intervals, 0.99), "fps": 1000 * len(intervals) / sum(intervals),
            "publications": result.get("publications"), "seconds": result["seconds"], "setup": result.get("setup"),
            "status": result.get("status"), "shaderLog": result.get("log")}


def table(rows):
    print(" | ".join(("config", "scale", "N", "api", "frames", "interval ms med/p95/p99", "fps", "note")))
    for r in rows:
        if "failed" in r:
            print(" | ".join(map(str, (r["config"], r["scale"], r["n"], "-", "-", "-", "-", r["failed"]))))
            continue
        if r.get("publications") is not None:
            note = f"ready after {r['setup']:.1f} s, {r['publications']} publications in {r['seconds']:.1f} s"
        elif r["api"] == "software":
            note = "ShaderEffect draws nothing on this scene graph; the interval is not a shader measurement"
        else:
            note = f"ShaderEffect.status {STATUS.get(r['status'], r['status'])} when the run ended"
        print(" | ".join(map(str, (r["config"], r["scale"], r["n"], r["api"], r["frames"],
                                    f"{r['intervalMedian']:.0f}/{r['intervalP95']:.0f}/{r['intervalP99']:.0f}",
                                    f"{r['fps']:.1f}", note))))


def measure(arguments, qml, extra_env, kind):
    work = Path(tempfile.mkdtemp(prefix="material-spike-run-"))
    rows = []
    try:
        prepare(work, arguments.qsb)
        if kind == "raster":
            shutil.copytree(ROOT / "backends/quickshell/resina-qml/qml", work / "controls")
            (work / "request.js").write_text(request_script())
        for scale in arguments.scales:
            session = Session(arguments.host, scale, arguments.sway)
            try:
                for name in arguments.configs:
                    for count in arguments.counts:
                        environment = dict(CONFIGS[name], MATERIAL_SPIKE_N=str(count), **extra_env)
                        text = launch(arguments.quickshell, work, qml, session, environment, arguments.timeout)
                        rows.append(row(name, scale, count, text))
                        print(json.dumps(rows[-1]), file=sys.stderr)
            finally:
                session.close()
    finally:
        shutil.rmtree(work, ignore_errors=True)
    return rows


def request_script():
    sys.path.insert(0, str(ROOT / "tools"))
    from check_qt_command_snapshot_runtime import request_for
    request = request_for("light", "Reconnect")
    request["content"]["environment"]["accessibilityPreferences"]["reducedMotion"] = False
    request["availableBounds"] = {"x": -100, "y": -100, "width": 4000, "height": 3000}
    channels = json.loads((ROOT / "conformance/ir/command-motion-request.json").read_text())["channels"]
    return ".pragma library\nvar configuration = %s;\nvar channels = %s;\n" % (
        json.dumps(json.dumps(request)), json.dumps(json.dumps(channels)))


def bench(arguments):
    rows = measure(arguments, "bench.qml", {}, "shader")
    table(rows)
    return rows


def baseline(arguments):
    sys.path.insert(0, str(ROOT / "tools"))
    import conformance_font
    fonts = conformance_font.qml_sources(Path(arguments.font_dir))
    environment = {"MATERIAL_SPIKE_FONTS": fonts, "RESINA_COMMAND_FONTS": fonts,
                   "QML_IMPORT_PATH": str(Path(arguments.module_path).resolve())}
    rows = measure(arguments, "baseline.qml", environment, "raster")
    table(rows)
    return rows


def difference(first, second, path="scene"):
    if isinstance(first, dict) and isinstance(second, dict):
        if first.keys() != second.keys():
            return path + " keys"
        return next((found for key in first if (found := difference(first[key], second[key], f"{path}.{key}"))), None)
    if isinstance(first, list) and isinstance(second, list):
        if len(first) != len(second):
            return path + " length"
        return next((found for index, (a, b) in enumerate(zip(first, second))
                     if (found := difference(a, b, f"{path}[{index}]"))), None)
    if isinstance(first, (int, float)) and isinstance(second, (int, float)) and not isinstance(first, bool):
        return None if abs(first - second) <= 1e-9 * max(1.0, abs(first)) else f"{path}: {first} against {second}"
    return None if first == second else f"{path}: {first!r} against {second!r}"


def check_bench_scene(scenes):
    source = (HERE / "qml" / "bench-scene.js").read_text()
    committed = json.loads(source.split("var scene = ", 1)[1].rsplit(";", 1)[0])
    current = next(dict(scene) for scene in scenes if scene["name"] == committed["name"])
    current.pop("views")
    found = difference(committed, current)
    if found:
        raise SystemExit(f"qml/bench-scene.js differs from the scene dumped from the manifest at {found}; regenerate it")


def conformance(arguments):
    sys.path.insert(0, str(HERE / "tools"))
    import compare
    target = Path(arguments.target_dir)
    subprocess.run(["cargo", "build", "--release", "--locked", "--target-dir", str(target),
                    "-p", "resina-raster", "--bin", "resina-command-raster", "--bin", "resina-toggle-part-raster",
                    "-p", "resina-resolver", "--bin", "resina-theme-resolve"], cwd=ROOT, check=True)
    output = Path(arguments.output_dir)
    references = output / "cpu"
    subprocess.run([sys.executable, str(HERE / "tools" / "dump_scenes.py"),
                    "--theme-backend", str(target / "release" / "resina-theme-resolve"),
                    "--command-raster", str(target / "release" / "resina-command-raster"),
                    "--toggle-raster", str(target / "release" / "resina-toggle-part-raster"),
                    "--output-dir", str(references)], check=True)
    scenes = json.loads((references / "scenes.json").read_text())["scenes"]
    check_bench_scene(scenes)
    work = Path(tempfile.mkdtemp(prefix="material-spike-run-"))
    try:
        prepare(work, arguments.qsb)
        (work / "scenes.js").write_text(".pragma library\nvar scenes = " + json.dumps(scenes) + ";\n")
        session = Session(arguments.host, 1, arguments.sway)
        try:
            for name in arguments.configs:
                captured = output / name
                captured.mkdir(parents=True, exist_ok=True)
                text = launch(arguments.quickshell, work, "capture.qml", session,
                              dict(CONFIGS[name], MATERIAL_SPIKE_OUT=str(captured)), arguments.timeout)
                if "CAPTURE_DONE" not in text:
                    raise SystemExit(f"{name}: capture did not complete: {text[-400:]}")
        finally:
            session.close()
    finally:
        shutil.rmtree(work, ignore_errors=True)
    controls = {name for name in arguments.configs if name.endswith("no-ring")}
    renderers = [name for name in arguments.configs if name not in controls]
    status = 0
    for name in arguments.configs:
        rows = [compare.compare(scene, scale, references / f"{scene['name']}@{scale}.png",
                                output / name / f"{scene['name']}@{scale}.png")
                for scene in scenes for scale in (1, 2)]
        print(f"\nshader on {name} against the CPU raster" + (" (negative control: no edge ring, must fail)" if name in controls else ""))
        compare.summary(rows)
        if bool(compare.verdict(rows)) != (name in controls):
            status = 1
    for name in renderers[1:]:
        print(f"\nshader on {name} against shader on {renderers[0]}")
        compare.summary([compare.compare(scene, scale, output / renderers[0] / f"{scene['name']}@{scale}.png",
                                         output / name / f"{scene['name']}@{scale}.png")
                         for scene in scenes for scale in (1, 2)])
    return status


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("stage", choices=("bench", "baseline", "conformance"))
    parser.add_argument("--quickshell", required=True)
    parser.add_argument("--qsb", required=True)
    parser.add_argument("--host", choices=("sway", "session"), default="sway")
    parser.add_argument("--sway", default="sway")
    parser.add_argument("--configs", nargs="+", default=["opengl"], choices=sorted(CONFIGS))
    parser.add_argument("--counts", nargs="+", type=int, default=[1, 10, 30, 100])
    parser.add_argument("--scales", nargs="+", type=float, default=[1, 2])
    parser.add_argument("--timeout", type=float, default=240)
    parser.add_argument("--module-path")
    parser.add_argument("--font-dir")
    parser.add_argument("--target-dir", default=str(ROOT / "target" / "material-spike"))
    parser.add_argument("--output-dir", default=str(ROOT / "target" / "material-spike-out"))
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    stages = {"bench": bench, "baseline": baseline, "conformance": conformance}
    rows = stages[arguments.stage](arguments)
    if arguments.stage == "conformance":
        return rows
    if arguments.output and rows is not None:
        arguments.output.write_text(json.dumps(rows, indent=1) + "\n")


if __name__ == "__main__":
    sys.exit(main())
