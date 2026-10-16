#!/usr/bin/env bash
#
# build-vela-server.sh — Build the vela-server binary for one platform and package it for publishing.
#
# Shared by the vela-server workflow and local runs. Writes dist-server/vela-server-<platkey> and
# dist-server/vela-server-<platkey>.zip, whose only entry is named `vela-server`. Signing is left to the caller.
#
# The frontend must already be built: the release profile embeds ../dist into the binary at compile time.
# Linux targets cross-compile with cargo zigbuild against glibc 2.17 (CentOS 7+); macOS targets build natively.
#
# Usage:
#   scripts/build-vela-server.sh <platkey>
#
set -euo pipefail

cd "$(dirname "$0")/.."

die() { echo "build-vela-server: $1" >&2; exit 1; }

PLATKEYS="linux-x86_64 linux-aarch64 darwin-aarch64 darwin-x86_64"

[ $# -eq 1 ] || die "usage: $0 <platkey> (supported: $PLATKEYS)"
PLATKEY="$1"

case "$PLATKEY" in
  linux-x86_64)   TARGET="x86_64-unknown-linux-gnu.2.17";  BUILD="zigbuild" ;;
  linux-aarch64)  TARGET="aarch64-unknown-linux-gnu.2.17"; BUILD="zigbuild" ;;
  darwin-aarch64) TARGET="aarch64-apple-darwin";           BUILD="build" ;;
  darwin-x86_64)  TARGET="x86_64-apple-darwin";            BUILD="build" ;;
  *) die "unknown platkey: $PLATKEY (supported: $PLATKEYS)" ;;
esac

[ -f dist/index.html ] || die "dist/ is not built; run pnpm build first"

VLX_GIT_COMMIT="$(git rev-parse --short HEAD)" \
VLX_BUILD_TIME="$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
  cargo "$BUILD" --release --no-default-features --manifest-path src-tauri/Cargo.toml --target "$TARGET"

# zigbuild names its output directory after the plain triple, without the glibc suffix.
BIN="src-tauri/target/${TARGET%.2.17}/release/velaterm"
[ -f "$BIN" ] || die "expected build output not found: $BIN"

OUT="dist-server/vela-server-$PLATKEY"
mkdir -p dist-server
cp "$BIN" "$OUT"
chmod 755 "$OUT"

# Python's zipfile instead of `zip`, which not every runner ships. The mode bits are stored explicitly so the
# extracted binary stays executable.
# Each candidate is probed because Windows puts a non-functional Microsoft Store python3 stub on PATH.
PYTHON=""
for candidate in python3 python; do
  if "$candidate" -c "" >/dev/null 2>&1; then PYTHON="$candidate"; break; fi
done
[ -n "$PYTHON" ] || die "python3 is required to write the zip"
"$PYTHON" -I - "$OUT" "$OUT.zip" <<'EOF'
import sys, zipfile
src, dst = sys.argv[1], sys.argv[2]
info = zipfile.ZipInfo("vela-server")
info.create_system = 3  # Unix; unzip ignores the mode bits of an entry made on a Windows host otherwise.
info.external_attr = 0o100755 << 16
info.compress_type = zipfile.ZIP_DEFLATED
with open(src, "rb") as f, zipfile.ZipFile(dst, "w") as z:
    z.writestr(info, f.read())
EOF

echo "build-vela-server: wrote $OUT and $OUT.zip"
