#!/usr/bin/env bash
# Trae platforms/android-36 desde el artefacto de CI y arma el mini-SDK local en $HOME/android-sdk.
# (Necesario porque esta red no alcanza dl.google.com/android/repository.)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SDK="${ANDROID_SDK_LOCAL:-$HOME/android-sdk}"
mkdir -p "$SDK/platforms" "$SDK/licenses"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

RUN_ID="$(cd "$ROOT" && gh run list --workflow=CI --branch=develop --state=success --limit 1 --json databaseId --jq '.[0].databaseId')"
echo "usando run CI #$RUN_ID"
gh run download "$RUN_ID" -n android-37-platform -D "$TMP"

[ -f "$TMP/android-37-platform.zip" ] || { ls -la "$TMP"; exit 1; }
unzip -q "$TMP/android-37-platform.zip" -d "$TMP"

[ -d "$SDK/platforms/android-37" ] || mv "$TMP/android-37" "$SDK/platforms/"

cat > "$SDK/licenses/android-sdk-license" <<EOF
8933bad161af4178b1185d1a37fbf41ea5269c55
d56f5187479451eabf01fb78af6dfcb131a6481e
24333f8a63b6825ea9c5514f83c2829b004d1fee
EOF

cat > "$ROOT/android/local.properties" <<EOF
sdk.dir=$SDK
EOF

echo "Mini-SDK listo: $SDK/platforms/android-36"
