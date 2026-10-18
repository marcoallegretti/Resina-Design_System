#!/usr/bin/env bash
set -euo pipefail
python -m pip install aqtinstall==3.3.0
python -m aqt install-qt linux desktop 6.11.2 linux_gcc_64 --outputdir target/qt-sdk --archives qtbase qtdeclarative icu --modules qtshadertools
