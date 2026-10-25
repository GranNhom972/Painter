#!/usr/bin/env bash
# Makes the WebKitGTK/libsoup development files available to the Rust build
# WITHOUT root: downloads the -dev packages with `apt download`, unpacks them
# under target/devlibs and prints the PKG_CONFIG_PATH to use.
# Only needed when `pkg-config --exists webkit2gtk-4.1` fails and you cannot run:
#   sudo apt install libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libjavascriptcoregtk-4.1-dev
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/target/devlibs"
SYSROOT="$DEST/sysroot"
MULTIARCH="$(dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || echo x86_64-linux-gnu)"
LIBDIR="$SYSROOT/usr/lib/$MULTIARCH"

if [ ! -d "$LIBDIR/pkgconfig" ]; then
  mkdir -p "$DEST/debs" "$SYSROOT"
  (cd "$DEST/debs" && apt download libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libjavascriptcoregtk-4.1-dev >&2)
  for f in "$DEST"/debs/*.deb; do dpkg-deb -x "$f" "$SYSROOT"; done
  # Link the unversioned .so names to the runtime libraries already installed.
  for so in "$LIBDIR"/*.so; do
    ln -sf "/usr/lib/$MULTIARCH/$(readlink "$so")" "$so"
  done
  sed -i -e '/^Requires.private/d' -e "s#^prefix=/usr#prefix=$SYSROOT/usr#" -e "s#^libdir=/usr/lib/$MULTIARCH#libdir=$LIBDIR#" "$LIBDIR"/pkgconfig/*.pc
fi
echo "$LIBDIR/pkgconfig"
