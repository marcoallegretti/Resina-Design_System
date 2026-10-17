#!/usr/bin/env bash
set -euo pipefail
export LD_LIBRARY_PATH="$PWD/target/wayland-sdk/lib:$PWD/target/qt-sdk/6.11.2/gcc_64/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PKG_CONFIG_PATH="$PWD/target/wayland-sdk/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export PATH="$PWD/target/wayland-sdk/bin:$PATH"
git clone --branch v0.3.2 --depth 1 https://github.com/quickshell-mirror/quickshell.git target/quickshell-source
test "$(git -C target/quickshell-source rev-parse HEAD)" = 4f508be500dea6e5732cc3d50382a0048b17e7b1
cmake -S target/quickshell-source -B target/quickshell-build -G Ninja \
  -DCMAKE_BUILD_TYPE=RelWithDebInfo -DCMAKE_PREFIX_PATH="$PWD/target/qt-sdk/6.11.2/gcc_64;$PWD/target/wayland-sdk" \
  -DCRASH_HANDLER=OFF -DSCREENCOPY=OFF -DHYPRLAND=OFF -DI3=OFF \
  -DSERVICE_PIPEWIRE=OFF -DSERVICE_PAM=OFF -DSERVICE_POLKIT=OFF -DSERVICE_GREETD=OFF \
  -DSERVICE_STATUS_NOTIFIER=OFF -DSERVICE_MPRIS=OFF -DSERVICE_UPOWER=OFF \
  -DSERVICE_NOTIFICATIONS=OFF -DBLUETOOTH=OFF -DNETWORK=OFF
cmake --build target/quickshell-build --parallel 2
find target/quickshell-build -name '*.o' -delete
