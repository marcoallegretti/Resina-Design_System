#!/usr/bin/env bash
set -euo pipefail
curl --fail --location https://gitlab.freedesktop.org/wayland/wayland/-/releases/1.24.0/downloads/wayland-1.24.0.tar.xz --output target/wayland-1.24.0.tar.xz
printf '%s\n' '82892487a01ad67b334eca83b54317a7c86a03a89cfadacfef5211f11a5d0536  target/wayland-1.24.0.tar.xz' | sha256sum -c
tar -xf target/wayland-1.24.0.tar.xz -C target
meson setup target/wayland-build target/wayland-1.24.0 --prefix "$PWD/target/wayland-sdk" --libdir lib -Ddocumentation=false -Dtests=false -Ddtd_validation=false
meson compile -C target/wayland-build -j 2
meson install -C target/wayland-build
