import argparse
from pathlib import Path
import shutil
import sys


def main():
    parser = argparse.ArgumentParser(description="Package the compiled Linux Resina.Runtime Qt plugin.")
    parser.add_argument("--library", required=True, type=Path)
    parser.add_argument("--metadata-dir", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path, help="fresh QML import root")
    arguments = parser.parse_args()
    created = False
    try:
        if sys.platform != "linux" or arguments.library.suffix != ".so" or not arguments.library.is_file():
            raise ValueError("requires Linux and the compiled resina-qt-runtime shared library")
        metadata = arguments.metadata_dir
        if (metadata / "qmldir").read_text() != (
            "module Resina.Runtime\noptional plugin Resina_Runtime\nclassname Resina_Runtime_plugin\n"
            "typeinfo plugin.qmltypes\nprefer :/qt/qml/Resina/Runtime/\n"
        ) or not (metadata / "plugin.qmltypes").is_file():
            raise ValueError("requires the generated Resina.Runtime metadata")
        arguments.output_dir.mkdir(parents=True, exist_ok=False)
        created = True
        destination = arguments.output_dir / "Resina/Runtime"
        destination.mkdir(parents=True)
        for name in ("qmldir", "plugin.qmltypes"):
            shutil.copyfile(metadata / name, destination / name)
        shutil.copyfile(arguments.library, destination / "libResina_Runtime.so")
    except (OSError, ValueError) as error:
        if created:
            shutil.rmtree(arguments.output_dir)
        print("FAIL Qt runtime module: " + str(error), file=sys.stderr)
        return 1
    print(str(arguments.output_dir.resolve()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
