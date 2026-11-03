#!/usr/bin/env bash
# Setup de un Codespace: cmdline-tools + platforms android-36 (net OK en nube).
set -euo pipefail
SDK="$HOME/android-sdk"
mkdir -p "$SDK/licenses"
echo "24333f8a63b6825ea9c5514f83c2829b004d1fee" > "$SDK/licenses/android-sdk-license"
echo "d56f5187479451eabf01fb78af6dfcb131a6481e" > "$SDK/licenses/android-sdk-license"
if [ ! -d "$SDK/platforms/android-37" ]; then
  curl -fsSL -o /tmp/ctl.zip https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip
  unzip -q /tmp/ctl.zip -d /tmp/ -o
  yes | /tmp/cmdline-tools/bin/sdkmanager --sdk_root="$SDK" "platforms;android-37" >/dev/null
fi
echo "sdk.dir=$SDK" > android/local.properties || true
echo "[devcontainer] SDK listo: $SDK/platforms/android-36"
