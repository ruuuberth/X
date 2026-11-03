#!/usr/bin/env bash
# Build local N1 en Termux: Rust (clang de Termux, sin NDK/proot) + Gradle wrapper + aapt2 de Termux.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [ ! -f android/local.properties ]; then
  echo "ERROR: falta android/local.properties → ejecuta primero: scripts/fetch-platform.sh" >&2
  exit 1
fi

cargo build --target aarch64-linux-android --release -p x-jni
mkdir -p android/app/src/main/jniLibs/arm64-v8a
cp -f target/aarch64-linux-android/release/libxjni.so android/app/src/main/jniLibs/arm64-v8a/

cd android
./gradlew :app:assembleDebug -Pandroid.aapt2FromMavenOverride="$PREFIX/bin/aapt2" "$@"

echo "APK: $ROOT/android/app/build/outputs/apk/debug/app-debug.apk"
