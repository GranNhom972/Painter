#!/usr/bin/env bash
# Builds Paint for Linux and produces the installable packages:
#   target/release/bundle/deb/Paint_<version>_amd64.deb
#   target/release/bundle/rpm/Paint-<version>-1.x86_64.rpm
#   target/release/bundle/appimage/Paint_<version>_amd64.AppImage
# Requirements: Rust (+ wasm32-unknown-unknown target), wasm-pack, Node.js/npm.
# WebKitGTK headers: system packages if installed, otherwise fetched without root.
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target list --installed | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
command -v wasm-pack >/dev/null || cargo install wasm-pack --locked
[ -d node_modules ] || npm install --no-audit --no-fund
if ! pkg-config --exists webkit2gtk-4.1 2>/dev/null; then
  export PKG_CONFIG_PATH="$(scripts/linux-devlibs.sh)${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
fi
export APPIMAGE_EXTRACT_AND_RUN=1 NO_STRIP=true
npx tauri build "$@"
ls -1 target/release/bundle/*/*.{deb,rpm,AppImage} 2>/dev/null || true
