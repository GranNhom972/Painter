#!/usr/bin/env bash
# Desktop end-to-end tests: builds the app, then drives it with tauri-driver
# and WebKitWebDriver. Needs a display (uses $DISPLAY, or starts Xvfb :99).
# Tools: cargo install tauri-driver --locked ; apt install webkitgtk-webdriver xvfb
set -euo pipefail
cd "$(dirname "$0")/.."
command -v pkg-config >/dev/null && pkg-config --exists webkit2gtk-4.1 || export PKG_CONFIG_PATH="$(scripts/linux-devlibs.sh)"
npm run build:wasm >/dev/null
cargo build -p paint
export TAURI_DRIVER="${TAURI_DRIVER:-$(command -v tauri-driver || echo "$HOME/.cargo/bin/tauri-driver")}"
export WEBKIT_WEBDRIVER="${WEBKIT_WEBDRIVER:-$(command -v WebKitWebDriver || echo /usr/bin/WebKitWebDriver)}"
# Use a private X server unless HEADED=1: the test windows must not pop up on
# the user's desktop (GTK would otherwise pick the Wayland session by default).
if [ "${HEADED:-0}" != 1 ]; then
  if [ "${DISPLAY:-}" != ":99" ]; then
    XVFB="${XVFB:-$(command -v Xvfb)}"
    "$XVFB" :99 -screen 0 1400x900x24 -nolisten tcp >/dev/null 2>&1 &
    trap 'kill $!' EXIT
    sleep 1
  fi
  export DISPLAY=:99 GDK_BACKEND=x11
  unset WAYLAND_DISPLAY
fi
exec dbus-run-session -- node --test --test-concurrency=1 tests/desktop/desktop.test.mjs
