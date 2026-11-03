#!/usr/bin/env bash
# Genera keystore local (una sola vez) y muestra los valores para los secrets de GitHub:
#   gh secret set ANDROID_KEYSTORE_B64   ← binario .keystore codificado base64
#   gh secret set ANDROID_KEYSTORE_PASS  ← contraseña (storePassword == keyPassword)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DIR="$ROOT/android/local/keystore"
mkdir -p "$DIR"

if [ ! -f "$DIR/keystore.properties" ]; then
  PASS="$(head -c 16 /dev/urandom | md5sum | cut -d' ' -f1)"
  keytool -genkeypair -v \
    -keystore "$DIR/xclient.keystore" \
    -alias xclient -keyalg RSA -keysize 2048 -validity 10000 \
    -storepass "$PASS" -keypass "$PASS" \
    -dname "CN=X Client, OU=personal, O=ruuuberth" >/dev/null
  cat > "$DIR/keystore.properties" <<EOF
storeFile=$DIR/xclient.keystore
storePassword=$PASS
keyAlias=xclient
keyPassword=$PASS
EOF
fi

PASS="$(sed -n 's/^storePassword=//p' "$DIR/keystore.properties")"
echo "== Secrets para GitHub =="
echo "gh secret set ANDROID_KEYSTORE_B64 --rev-file "$DIR/xclient.keystore" --body \"\$(base64 -w0 "$DIR/xclient.keystore")\""
echo "gh secret set ANDROID_KEYSTORE_PASS --body \"$PASS\""
echo "(el .properties queda solo en android/local/, gitignored)"
