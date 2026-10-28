# Paint

A raster image editor inspired by Paint for Windows 11, written in **Rust**.
The drawing engine is compiled to **WebAssembly**. The cross-platform desktop app
uses **Tauri**. Linux is the primary target (.deb, .rpm, AppImage).

![Paint on Linux: a photo of Les Salines beach opened in the app](docs/capture.webp)

<sub>Desktop app on Linux. Photo: Les Salines beach (Martinique), Nicolas Bouthors,
public domain, [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Martinique_Beach_%28Salines%29.jpg).</sub>

## Architecture

```
crates/paint-core   Pure Rust engine: layers, tools, brushes, shapes, selection,
                    history (undo/redo), clipboard, codecs (PNG, JPEG, BMP,
                    GIF, TIFF, WEBP, ICO). No browser dependency, fully tested.
crates/paint-wasm   wasm-bindgen bindings: the engine compiled to WebAssembly (ui/pkg).
ui/                 User interface (HTML/CSS/JS) that forwards events to the engine and
                    displays the image directly from WebAssembly memory.
src-tauri/          Rust desktop app (Tauri 2): window, system clipboard
                    (GTK on Linux, arboard on Windows/macOS), files,
                    dialogs, opening a file passed on the command line.
```

All the logic (tools, selection, handles, history) lives in the Rust engine.
The interface simply forwards pointer events and commands to it.

## Features

- **Selection**: rectangle and free-form; move, copy (Ctrl+drag), resize with
  8 handles (Shift = keep proportions), arrow keys, select all, invert, delete,
  transparent selection, crop, context menu (right-click).
- **System clipboard**: cut/copy/paste images between Paint and other
  applications; pasting an image larger than the canvas enlarges it; "Paste from file".
- **Tools**: pencil, fill (tolerance), text (font, size, bold, italic, underline,
  strikethrough, opaque background), eraser (right-click = color replacement), color picker, magnifier.
- **Brushes**: brush, calligraphy 1 and 2, airbrush, oil, crayon, marker, natural pencil, watercolor.
- **Shapes (23)**: line, curve (2 bends), oval, rectangles, polygon, triangles, diamond,
  pentagon, hexagon, 4 arrows, 3 stars, 3 callouts, heart, lightning; outline/fill;
  shape stays editable (move, resize, recolor) until committed.
- **Image**: crop, resize and skew, rotations, flips, background
  removal, properties (canvas size), canvas resize handles on all 8 sides.
- **Layers**: add, duplicate, merge down, delete, hide, reorder
  (buttons or drag), rename, thumbnails.
- **Colors**: color 1 and 2 (left-click uses the selected color, right-click the other one), 20-color palette + 10 custom colors, HSV/RGB/hex editor.
- **View**: zoom from 10% to 800% (Ctrl + mouse wheel, slider, presets, fit to
  window), rulers, gridlines, status bar, full screen, light/dark theme, French/English.
- **Files**: open, save, save as (7 formats), print, drag and drop,
  unsaved-changes confirmation (including on close).
- **Undo / Redo**: arrows next to Save, Ctrl+Z / Ctrl+Y, mouse Back / Forward
  buttons, and in the web version the browser's Back / Forward buttons (the page
  is not left).
- **Window (Linux)**: built-in title bar like Windows 11 (icon, title, Minimize,
  Maximize/Restore, Close), drag by the title bar, resize from the edges.
- **Icons**: those of Windows 11 Paint as SVG — Microsoft Fluent UI System Icons glyphs
  (MIT license, see `ui/icons/LICENSE-fluent.txt`) and redrawn Paint-specific pictograms.

## Running Paint

```bash
./run --web        # web version: builds the WASM engine, serves the app and opens the browser
./run --linux      # Linux desktop app (add --release for the optimized build)
```

Other modes: `./run --package` (.deb/.rpm/AppImage packages), `./run --web-build [folder]`
(static site to host), `./run --test` (all test suites), `./run --help`.
`./run --web --host 0.0.0.0 --port 8080` makes the web version reachable on the local network.

The web version is an installable app (PWA): use the browser's "Install" button.
Once loaded, it works offline. It also shows up in "Open with" for images.

### With Docker

```bash
docker compose up -d      # builds the image and starts Paint: http://localhost:8088/
docker compose down       # stops Paint
```

Nothing but Docker is needed: the image compiles the Rust engine to
WebAssembly, then serves the web version with nginx. The container runs as non-root, read-only,
with no system capabilities, and restarts automatically with Docker (`restart: unless-stopped`).
`./run --docker` does the same, then opens the browser.

- Different port: `PAINT_PORT=9000 docker compose up -d`.
- By default, Paint only listens on the local machine. `PAINT_HOST=0.0.0.0` opens it to the network;
  the clipboard and offline mode then require HTTPS (a reverse proxy in front of the container),
  because browsers only allow them over HTTPS or on `localhost`.
- Without Compose: `docker build -t paint . && docker run -d -p 127.0.0.1:8088:8080 paint`.

## Installing on Linux

Download the package for the latest version from the
[Releases](https://github.com/GranNhom972/Painter/releases/latest) page, then:

| Distribution | x86_64 (Intel/AMD PC) | arm64 (Raspberry Pi, ARM) | Installation |
|---|---|---|---|
| Debian, Ubuntu, Linux Mint… | [`Paint_amd64.deb`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_amd64.deb) | [`Paint_arm64.deb`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_arm64.deb) | `sudo apt install ./Paint_amd64.deb` |
| Fedora, openSUSE, RHEL… | [`Paint_x86_64.rpm`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_x86_64.rpm) | [`Paint_aarch64.rpm`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_aarch64.rpm) | `sudo dnf install ./Paint_x86_64.rpm` |
| Arch, Manjaro, Gentoo, others | [`Paint_linux_x86_64.tar.gz`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_linux_x86_64.tar.gz) | [`Paint_linux_aarch64.tar.gz`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_linux_aarch64.tar.gz) | `tar xzf Paint_linux_*.tar.gz && sudo Paint-*/install.sh` |
| No installation | [`Paint_x86_64.AppImage`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_x86_64.AppImage) | [`Paint_aarch64.AppImage`](https://github.com/GranNhom972/Painter/releases/latest/download/Paint_aarch64.AppImage) | `chmod +x Paint_*.AppImage && ./Paint_*.AppImage` |

`uname -m` tells you which processor you have. The packages are built on Ubuntu 22.04, so they also
install on newer distributions. Paint shows up in the applications menu with its
icon, and under "Open with" for PNG, JPEG, BMP, GIF, TIFF, WEBP and ICO images. The archive
can also be installed without admin rights (`install.sh --user`) and uninstalled with
`install.sh --uninstall`. After a local build (see below), the same packages are in
`target/release/bundle/`.

### Publishing a release

1. Put the new version number in `Cargo.toml`, `package.json` and `src-tauri/tauri.conf.json`, then commit.
2. Push the matching tag:

   ```bash
   git tag v1.0.0
   git push origin v1.0.0
   ```

GitHub Actions (`.github/workflows/release.yml`) tests the engine, builds the .deb, the .rpm and
the AppImage, then publishes the Release with their checksums. A tag that doesn't match the
version is rejected. `scripts/release-linux.sh` produces the same files locally, in `dist/`.

## Building

Prerequisites: Rust (`wasm32-unknown-unknown` target), `wasm-pack`, Node.js.
The WebKitGTK headers are taken from the system if installed
(`sudo apt install libwebkit2gtk-4.1-dev`). Otherwise, `scripts/linux-devlibs.sh` downloads them
without admin rights.

```bash
scripts/build-linux.sh        # app + .deb, .rpm, AppImage packages
npm run dev                   # runs the desktop app in development mode
npm run build:wasm && npm run serve   # browser version at http://127.0.0.1:4173
```

On Windows and macOS, `npm run build` produces the native installers (MSI/NSIS, DMG).

## Tests

| Suite | Command | Coverage |
|---|---|---|
| Engine (Rust) | `cargo test -p paint-core` | 183 tests: every tool, every brush and every shape, the whole selection system, clipboard, history, layers, formats, and 26 regression tests from audits |
| Engine in WebAssembly | `npm run test:wasm` | the API compiled to WASM, run in Node |
| Interface (browser) | `npm run build:wasm && npm run test:e2e` | 102 Playwright tests driving the real interface: buttons, menus, shortcuts (AZERTY keyboard included), keyboard navigation, narrow windows, touch, mouse selection, system clipboard, files, layers, browser Back/Forward, offline mode |
| Desktop (native) | `cargo test -p paint` | files (concurrent saves, permissions, links), command line, GTK clipboard against a real X server (copy to another application and paste from another application) |
| Desktop app | `scripts/test-desktop.sh` | 11 WebDriver tests on the real Tauri app (WebKitGTK): title bar, mouse (Back/Forward buttons included), keyboard, native clipboard, saving and opening, format from extension, confirmation on close |
| Docker image | `scripts/test-docker.sh` | builds and starts the image with Compose, checks that it runs as non-root, read-only, with no capabilities and healthy, then runs the 102 Playwright tests against the container (MIME types, content security policy and offline mode included) |

`./run --test` runs all these suites (desktop and Docker ones when the tools are available).
`PAINT_URL=http://localhost:8088/ npx playwright test` targets an already running server. `cargo run --release -p paint-core --example bench` benchmarks
interactive operations on a 12-megapixel image.

## Limitations

- Maximum image size: 16,384 pixels per side and 64 megapixels. Operations that
  would exceed these limits are refused with a message instead of exhausting memory.
- Web version: browsers reserve Ctrl+N and Ctrl+W; use the
  File › New and Image › Resize menus instead.
- The text box can't be moved and doesn't wrap automatically (press Enter
  for a new line).

The GTK clipboard and desktop app tests need a display:
`DISPLAY` is used if set, otherwise `scripts/test-desktop.sh` starts Xvfb.

## Keyboard shortcuts

| Action | Shortcut |
|---|---|
| New / Open / Save | Ctrl+N / Ctrl+O / Ctrl+S |
| Save as (PNG) | F12, Ctrl+Shift+S |
| Print | Ctrl+P |
| Image properties | Ctrl+E |
| Resize and skew | Ctrl+W |
| Undo / Redo | Ctrl+Z / Ctrl+Y (or Ctrl+Shift+Z) |
| Cut / Copy / Paste | Ctrl+X / Ctrl+C / Ctrl+V |
| Select all | Ctrl+A |
| Delete selection | Delete |
| Crop | Ctrl+Shift+X |
| Move selection | Arrow keys (Shift = 10 px) |
| Commit / cancel (shape, selection) | Enter / Esc |
| Zoom in / out | Ctrl++ / Ctrl+-, Ctrl+mouse wheel |
| 100% / Fit to window | Ctrl+1 / Ctrl+0 |
| Rulers / Gridlines | Ctrl+R / Ctrl+G |
| New layer | Ctrl+Shift+N |
| Full screen | F11 |
| Text: bold / italic / underline, commit | Ctrl+B / Ctrl+I / Ctrl+U, Ctrl+Enter |
