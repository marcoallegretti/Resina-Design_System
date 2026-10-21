"""Pinned Inter static masters for native label conformance checks.

Resina does not name a product face. These masters only give label evidence
real 400, 500 and 600 weights. Measurements made with them are compared within
one run, never across machines.
"""

import argparse
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import sys
import urllib.request
import zipfile


MANIFEST = json.loads(Path(__file__).with_name("conformance_font.json").read_text(encoding="utf-8"))
FAMILY = MANIFEST["family"]


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def masters(directory):
    """Return the pinned masters in a directory, rejecting any missing or different file."""
    directory = Path(directory)
    records = []
    for master in MANIFEST["masters"]:
        path = directory / master["file"]
        try:
            data = path.read_bytes()
        except OSError as error:
            raise ValueError(f"{FAMILY} {master['style']} master is unavailable: {error}") from error
        if sha256(data) != master["sha256"]:
            raise ValueError(f"{path} is not the pinned {FAMILY} {MANIFEST['version']} {master['style']} master")
        records.append({"family": FAMILY, "style": master["style"], "weight": master["weight"],
                        "file": master["file"], "sha256": master["sha256"], "path": path.resolve()})
    return records


def qml_sources(directory):
    """Return the master file URLs that a QML probe loads, as a JSON array."""
    return json.dumps([record["path"].as_uri() for record in masters(directory)])


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


def fontconfig_faces(environment=None):
    """List the faces of the conformance family that fontconfig makes visible in an environment."""
    try:
        result = subprocess.run(["fc-list", "--format", "\t".join(f"%{{{field}}}" for field in FONTCONFIG_FIELDS)
                                 + "\n", f":family={FAMILY}"], env=environment, capture_output=True,
                                encoding="utf-8", errors="strict", timeout=60, check=False)
    except (subprocess.TimeoutExpired, UnicodeError) as error:
        raise ValueError(f"fc-list did not list the {FAMILY} faces: {error}") from error
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


def isolated_fontconfig(directory):
    """Write a fontconfig file with no font directories so only registered files are visible."""
    directory = Path(directory)
    (directory / "cache").mkdir(parents=True, exist_ok=True)
    config = directory / "fonts.conf"
    config.write_text('<?xml version="1.0"?>\n<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">\n'
                      f"<fontconfig><cachedir>{(directory / 'cache').resolve()}</cachedir></fontconfig>\n",
                      encoding="utf-8")
    return {"FONTCONFIG_FILE": str(config.resolve())}


def extract(archive, output):
    if sha256(archive) != MANIFEST["archive"]["sha256"]:
        raise ValueError("downloaded Inter archive does not match the pinned SHA-256")
    output = Path(output)
    output.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(archive)) as bundle:
        for master in MANIFEST["masters"]:
            (output / master["file"]).write_bytes(bundle.read(master["archivePath"]))
    return masters(output)


ARCHIVE_LIMIT = 64 * 1024 * 1024


def fetch(output):
    with urllib.request.urlopen(MANIFEST["archive"]["url"], timeout=120) as response:
        archive = response.read(ARCHIVE_LIMIT + 1)
    if len(archive) > ARCHIVE_LIMIT:
        raise ValueError(f"downloaded Inter archive exceeds {ARCHIVE_LIMIT} bytes")
    return extract(archive, output)


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


def main():
    parser = argparse.ArgumentParser(description=f"Provide or verify the pinned {FAMILY} static masters.")
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("fetch", help="download the pinned release archive and extract the masters").add_argument(
        "--output", type=Path, required=True)
    commands.add_parser("verify", help="verify a directory that contains the masters").add_argument(
        "directory", type=Path)
    arguments = parser.parse_args()
    try:
        records = fetch(arguments.output) if arguments.command == "fetch" else masters(arguments.directory)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(f"FAIL {FAMILY} conformance masters: {error}", file=sys.stderr)
        return 1
    for record in records:
        print(f"{record['family']} {record['style']} {record['weight']} {record['path']} {record['sha256']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
