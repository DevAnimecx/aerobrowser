#!/bin/sh
set -eu
# Local-only install. No sudo, no remote script execution, no implicit engine download.
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
DEST=${AERO_INSTALL_DIR:-"$HOME/.local/bin"}
mkdir -p "$DEST"
if [ -x "$ROOT/package-v04/aero-bundled-linux-x86_64" ]; then
  install -m 755 "$ROOT/package-v04/aero-bundled-linux-x86_64" "$DEST/aero"
else
  (cd "$ROOT" && cargo build --release --locked)
  install -m 755 "$ROOT/target/release/aero" "$DEST/aero"
  printf '%s\n' 'Installed unbundled build. Select an external CDP endpoint, or run aero engine install moli then --engine moli.'
fi
printf 'Installed %s/aero\n' "$DEST"
