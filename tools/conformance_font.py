"""Pinned static masters for native label conformance checks.

Resina does not name a product face. Inter gives Latin label evidence real 400,
500 and 600 weights. Noto Sans Arabic and Noto Sans Hebrew are the faces that
Arabic and Hebrew label text resolves to, so no script is drawn with whatever
fallback a machine happens to have. Measurements made with them are compared
within one run, never across machines.
"""

import argparse
import hashlib
import http.client
import io
import json
from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile
import time
import urllib.request
import zipfile


MANIFEST = json.loads(Path(__file__).with_name("conformance_font.json").read_text(encoding="utf-8"))
FAMILY = MANIFEST["family"]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def pinned_records(directory, source):
    """Return the pinned masters of one family in a directory, rejecting any missing or different file."""
    directory = Path(directory)
    records = []
    for master in source["masters"]:
        path = directory / master["file"]
        try:
            data = path.read_bytes()
        except OSError as error:
            raise ValueError(f"{source['family']} {master['style']} master is unavailable: {error}") from error
        if sha256(data) != master["sha256"]:
            raise ValueError(f"{path} is not the pinned {source['family']} {source['version']} {master['style']} master")
        records.append({"family": source["family"], "style": master["style"], "weight": master["weight"],
                        "file": master["file"], "sha256": master["sha256"], "path": path.resolve()})
    return records


def masters(directory):
    """Return the pinned Inter masters in a directory."""
    return pinned_records(directory, MANIFEST)


def script_masters(directory):
    """Return the pinned masters of every script face in a directory."""
    return [{**record, "script": script["script"], "lang": script["lang"]}
            for script in MANIFEST["scripts"] for record in pinned_records(directory, script)]


def qml_sources(directory):
    """Return the master file URLs that a QML probe loads, as a JSON array."""
    return json.dumps([record["path"].as_uri() for record in masters(directory)])


def qml_script_faces(root):
    """Return the script faces and the label evidence text in each script, as a JSON array for the QML oracle."""
    points = evidence_code_points(root)
    return json.dumps([{"script": script["script"], "family": script["family"],
                        "sample": "".join(map(chr, sorted(points[script["script"]])))}
                       for script in MANIFEST["scripts"]])


def resolved_script_faces(log, records, weights):
    """Identify the pinned script master that the native probe reported for each script and weight."""
    observed = {}
    for line in re.findall(r"RESINA_LABEL_SCRIPT_FACE (\{.*\})", log):
        report = json.loads(line)
        key = (report["script"], report["weight"])
        if key in observed:
            raise ValueError(f"native probe reported the {key[0]} face at weight {key[1]} twice")
        observed[key] = report["face"]
    expected = {(script["script"], weight) for script in MANIFEST["scripts"] for weight in weights}
    if set(observed) != expected:
        raise ValueError(f"native probe did not report every script face: {sorted(observed)}")
    return [{"script": script, **resolve([record for record in records if record["script"] == script],
                                         face["family"], face["style"], weight)}
            for (script, weight), face in sorted(observed.items())]


def evidence(record):
    return {key: record[key] for key in ("family", "style", "weight", "file", "sha256")}


def resolve(records, family, style, weight):
    """Identify the registered master a backend reported for a requested weight.

    A face whose own weight differs from the request is a substitution, even when
    the backend reports the requested weight.
    """
    for record in records:
        if record["family"] == family and record["style"] == style:
            if record["weight"] != weight:
                raise ValueError(f"weight {weight} resolved to the {family} {style} master "
                                 f"({record['weight']}, {record['file']}); the requested weight was substituted")
            return evidence(record)
    raise ValueError(f"weight {weight} resolved to {family!r} {style!r}, which is not a registered conformance master")


# Fontconfig's FC_WEIGHT_REGULAR, FC_WEIGHT_MEDIUM and FC_WEIGHT_DEMIBOLD.
FONTCONFIG_WEIGHTS = {80: 400, 100: 500, 180: 600}
FONTCONFIG_ROMAN = 0
FONTCONFIG_NORMAL_WIDTH = 100
FONTCONFIG_FIELDS = ("file", "index", "weight", "slant", "width")


def fontconfig_value(text):
    """Parse a fontconfig number or [low high] range into an inclusive interval."""
    match = re.fullmatch(r"\[\s*([0-9.]+)\s+([0-9.]+)\s*\]|([0-9.]+)", text.strip())
    if match is None:
        raise ValueError(f"fontconfig reported an unreadable value {text!r}")
    low, high, single = match.groups()
    return (float(single), float(single)) if single is not None else (float(low), float(high))


def parse_fontconfig_faces(output):
    faces = []
    for line in output.splitlines():
        if not line:
            continue
        fields = line.split("\t")
        if len(fields) != len(FONTCONFIG_FIELDS):
            raise ValueError(f"fontconfig reported an unreadable face {line!r}")
        face = dict(zip(FONTCONFIG_FIELDS, fields))
        face["index"] = int(face["index"])
        for key in ("weight", "slant", "width"):
            face[key] = fontconfig_value(face[key])
        faces.append(face)
    return faces


def fontconfig_faces(environment=None, pattern=None):
    """List the faces that fontconfig makes visible in an environment, by default those of the conformance family."""
    pattern = pattern or f":family={FAMILY}"
    try:
        result = subprocess.run(["fc-list", "--format", "\t".join(f"%{{{field}}}" for field in FONTCONFIG_FIELDS)
                                 + "\n", pattern], env=environment, capture_output=True,
                                encoding="utf-8", errors="strict", timeout=60, check=False)
    except (subprocess.TimeoutExpired, UnicodeError) as error:
        raise ValueError(f"fc-list did not list the faces of {pattern}: {error}") from error
    if result.returncode:
        raise ValueError(f"fc-list failed (exit {result.returncode}): {result.stderr[-2000:]}")
    return parse_fontconfig_faces(result.stdout)


def check_visible_faces(records, faces):
    """Reject a visible family face that could be matched for an authored weight but is not its pinned master.

    Qt matches an exact upright, normal-width weight when one exists, so only those
    faces can compete with a registered master. Each of them must be byte-identical
    to the master of that weight, otherwise the master Qt used cannot be identified
    by its family and style.
    """
    pinned = {record["weight"]: record for record in records}
    checked = []
    for face in faces:
        if not (face["slant"][0] <= FONTCONFIG_ROMAN <= face["slant"][1]
                and face["width"][0] <= FONTCONFIG_NORMAL_WIDTH <= face["width"][1]):
            continue
        for fontconfig_weight, weight in FONTCONFIG_WEIGHTS.items():
            if not face["weight"][0] <= fontconfig_weight <= face["weight"][1]:
                continue
            master = pinned[weight]
            if face["index"] != 0 or sha256(Path(face["file"]).read_bytes()) != master["sha256"]:
                raise ValueError(f"{face['file']} face {face['index']} is visible as {FAMILY} weight {weight} "
                                 f"but is not the pinned {master['style']} master")
            checked.append({"file": face["file"], "weight": weight})
    return sorted(checked, key=lambda face: (face["weight"], face["file"]))


def check_fontconfig(records, environment=None):
    """Check the fontconfig-visible faces of the family against the pinned masters."""
    return check_visible_faces(records, fontconfig_faces(environment))


def check_visible_script_faces(records, faces):
    """Require every pinned master of a script to be visible and reject any other visible face of it.

    Fallback for a script can select any face that supports it, whatever its
    family, weight or style, so every such face must be byte-identical to a master,
    and an empty listing proves nothing about which face would draw the text.
    """
    pinned = {record["sha256"]: record for record in records}
    seen = set()
    for face in faces:
        digest = sha256(Path(face["file"]).read_bytes())
        if face["index"] != 0 or digest not in pinned:
            raise ValueError(f"{face['file']} face {face['index']} is visible for "
                             f"{records[0]['script']} text but is not a pinned {records[0]['family']} master")
        seen.add(digest)
    missing = sorted(record["file"] for digest, record in pinned.items() if digest not in seen)
    if missing:
        raise ValueError(f"pinned {records[0]['family']} masters are not visible for "
                         f"{records[0]['script']} text: {', '.join(missing)}")
    return sorted(face["file"] for face in faces)


def check_script_fontconfig(records, environment=None):
    """Check that fontconfig shows only pinned faces for each script of the pinned script masters."""
    return {script["script"]: check_visible_script_faces(
        [record for record in records if record["script"] == script["script"]],
        fontconfig_faces(environment, f":lang={script['lang']}")) for script in MANIFEST["scripts"]}


def isolated_fontconfig(directory, fonts=None):
    """Write a fontconfig file whose only font files are those in fonts, or none.

    The system rendering configuration stays included, because it sets the hinting
    and metrics Qt shapes with; it declares no font directories.
    """
    directory = Path(directory)
    (directory / "cache").mkdir(parents=True, exist_ok=True)
    config = directory / "fonts.conf"
    config.write_text('<?xml version="1.0"?>\n<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">\n'
                      '<fontconfig><include ignore_missing="yes">/etc/fonts/conf.d</include>'
                      + (f"<dir>{Path(fonts).resolve()}</dir>" if fonts else "")
                      + f"<cachedir>{(directory / 'cache').resolve()}</cachedir></fontconfig>\n",
                      encoding="utf-8")
    return {"FONTCONFIG_FILE": str(config.resolve())}


_PROCESS_FONTCONFIG = []


def isolate_process(fonts):
    """Make fontconfig show only the pinned files in this process; call before any Qt application exists."""
    directory = tempfile.TemporaryDirectory(prefix="resina-fontconfig-")
    _PROCESS_FONTCONFIG.append(directory)
    os.environ.update(isolated_fontconfig(directory.name, fonts))


def sources():
    return [MANIFEST, *MANIFEST["scripts"]]


def verified(archive, source):
    if sha256(archive) != source["archive"]["sha256"]:
        raise ValueError(f"downloaded {source['family']} archive does not match the pinned SHA-256")
    return archive


def extract(archive, output, source=None):
    source = source or MANIFEST
    verified(archive, source)
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        for master in source["masters"]:
            (output / master["file"]).write_bytes(bundle.read(master["archivePath"]))
    return pinned_records(output, source)


ARCHIVE_LIMIT = 64 * 1024 * 1024


DOWNLOAD_ATTEMPTS = 3


def download(source):
    for attempt in range(DOWNLOAD_ATTEMPTS):
        try:
            with urllib.request.urlopen(source["archive"]["url"], timeout=120) as response:
                archive = response.read(ARCHIVE_LIMIT + 1)
            break
        except (OSError, http.client.HTTPException) as error:
            if attempt + 1 == DOWNLOAD_ATTEMPTS:
                raise ValueError(f"{source['family']} archive could not be downloaded: {error}") from error
            time.sleep(2 ** (attempt + 1))
    if len(archive) > ARCHIVE_LIMIT:
        raise ValueError(f"downloaded {source['family']} archive exceeds {ARCHIVE_LIMIT} bytes")
    return verified(archive, source)


def fetch(output):
    """Download every pinned archive, verify all of them, then extract the masters into one directory."""
    archives = [(source, download(source)) for source in sources()]
    return [record for source, archive in archives for record in extract(archive, output, source)]


SCRIPT_BLOCKS = {
    "Arabic": ((0x0600, 0x06FF), (0x0750, 0x077F), (0x08A0, 0x08FF), (0xFB50, 0xFDFF), (0xFE70, 0xFEFE)),
    "Hebrew": ((0x0590, 0x05FF), (0xFB1D, 0xFB4F)),
}
CODE_POINT_ESCAPE = re.compile(r"\\u\{([0-9a-fA-F]{1,6})\}|\\u([0-9a-fA-F]{4})")


def script_of(code_point):
    for script, blocks in SCRIPT_BLOCKS.items():
        if any(low <= code_point <= high for low, high in blocks):
            return script
    return None


def evidence_code_points(root):
    """Return the Arabic and Hebrew code points that tracked sources write literally or as escapes."""
    names = subprocess.run(["git", "ls-files", "-z"], cwd=root, capture_output=True, check=True).stdout
    found = {script: set() for script in SCRIPT_BLOCKS}
    for name in names.decode("utf-8").split("\0"):
        path = Path(root) / name
        if not name or name == "tools/conformance_font.json" or not path.is_file():
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        candidates = [ord(character) for character in text if ord(character) >= 0x0590]
        candidates += [int(braced or plain, 16) for braced, plain in CODE_POINT_ESCAPE.findall(text)]
        for code_point in candidates:
            script = script_of(code_point)
            if script:
                found[script].add(code_point)
    return found


def cmap_code_points(font):
    """Return the code points that a TrueType font's Unicode cmap maps to a glyph, from format 4 and 12 subtables."""
    def u16(at):
        return int.from_bytes(font[at:at + 2], "big")

    def u32(at):
        return int.from_bytes(font[at:at + 4], "big")

    table = next((u32(record + 8) for record in range(12, 12 + 16 * u16(4), 16) if font[record:record + 4] == b"cmap"),
                 None)
    if table is None:
        raise ValueError("font has no cmap table")
    mapped = set()
    for record in range(table + 4, table + 4 + 8 * u16(table + 2), 8):
        if (u16(record), u16(record + 2)) not in ((0, 3), (0, 4), (3, 1), (3, 10)):
            continue
        subtable = table + u32(record + 4)
        if u16(subtable) == 4:
            segments = u16(subtable + 6) // 2
            ends, starts = subtable + 14, subtable + 16 + 2 * segments
            deltas, offsets = starts + 2 * segments, starts + 4 * segments
            for segment in range(segments):
                first, last = u16(starts + 2 * segment), u16(ends + 2 * segment)
                delta, offset = u16(deltas + 2 * segment), u16(offsets + 2 * segment)
                for code_point in range(first, last + 1):
                    if not offset:
                        glyph = (code_point + delta) & 0xFFFF
                    else:
                        indexed = u16(offsets + 2 * segment + offset + 2 * (code_point - first))
                        glyph = (indexed + delta) & 0xFFFF if indexed else 0
                    if glyph:
                        mapped.add(code_point)
        elif u16(subtable) == 12:
            for group in range(subtable + 16, subtable + 16 + 12 * u32(subtable + 12), 12):
                first, last = u32(group), u32(group + 4)
                mapped.update(range(first + (u32(group + 8) == 0), last + 1))
    if not mapped:
        raise ValueError("font has no Unicode cmap of format 4 or 12")
    return mapped


def register_qt(directory):
    """Register the masters in the current Qt process and return their records."""
    from PySide6.QtGui import QFontDatabase

    records = masters(directory)
    for record in records:
        font_id = QFontDatabase.addApplicationFont(str(record["path"]))
        if font_id < 0 or FAMILY not in QFontDatabase.applicationFontFamilies(font_id):
            raise ValueError(f"Qt did not register {record['file']} as {FAMILY}")
    return records


def resolve_qt(records, weight):
    """Identify the master Qt matches for the conformance family at a requested weight."""
    from PySide6.QtGui import QFont, QFontInfo

    font = QFont(FAMILY)
    font.setPixelSize(20)
    font.setWeight(QFont.Weight(weight))
    info = QFontInfo(font)
    return resolve(records, info.family(), info.styleName(), weight)


def check_coverage(records, code_points):
    """Reject a script master that lacks a glyph for a code point which tracked label evidence uses."""
    for record in records:
        missing = sorted(code_points[record["script"]] - cmap_code_points(Path(record["path"]).read_bytes()))
        if missing:
            raise ValueError(f"{record['file']} has no glyph for label evidence code points "
                             + " ".join(f"U+{code_point:04X}" for code_point in missing))


def main():
    parser = argparse.ArgumentParser(description="Provide or verify the pinned conformance static masters.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("fetch", help="download the pinned release archives and extract the masters").add_argument(
        "--output", type=Path, required=True)
    commands.add_parser("verify", help="verify a directory that contains the masters").add_argument(
        "directory", type=Path)
    arguments = parser.parse_args()
    try:
        directory = arguments.output if arguments.command == "fetch" else arguments.directory
        if arguments.command == "fetch":
            fetch(directory)
        records = masters(directory) + script_masters(directory)
        check_coverage([record for record in records if "script" in record],
                       evidence_code_points(Path(__file__).resolve().parent.parent))
    except (OSError, ValueError, KeyError, zipfile.BadZipFile, subprocess.CalledProcessError) as error:
        print(f"FAIL conformance masters: {error}", file=sys.stderr)
        return 1
    for record in records:
        print(f"{record['family']} {record['style']} {record['weight']} {record['path']} {record['sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
