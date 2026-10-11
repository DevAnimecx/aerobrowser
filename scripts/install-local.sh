#!/bin/sh
set -eu
# Local-only install. No sudo, no remote script execution, no implicit engine download.
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST=${DELTA_INSTALL_DIR:-"$HOME/.local/bin"}
mkdir -p "$DEST"
if [ -x "$ROOT/package-v04/delta-bundled-linux-x86_64" ]; then
  install -m 755 "$ROOT/package-v04/delta-bundled-linux-x86_64" "$DEST/delta"
else
  (cd "$ROOT" && cargo build --release --locked)
  install -m 755 "$ROOT/target/release/delta" "$DEST/delta"
  printf '%s\n' 'Installed unbundled build. Select an external CDP endpoint, or run delta engine install moli then --engine moli.'
fi
printf 'Installed %s/delta\n' "$DEST"
