#!/usr/bin/env bash
# Paquets Linux d'une version publiée, pour l'architecture de la machine (x86_64 ou arm64), rangés
# dans dist/ sous des noms stables : les liens « releases/latest/download/… » du README restent
# valables d'une version à l'autre.
#   x86_64 : Paint_amd64.deb  Paint_x86_64.rpm   Paint_x86_64.AppImage   Paint_linux_x86_64.tar.gz
#   arm64  : Paint_arm64.deb  Paint_aarch64.rpm  Paint_aarch64.AppImage  Paint_linux_aarch64.tar.gz
# L'archive .tar.gz sert aux autres distributions (Arch, Gentoo…) : son install.sh pose le programme,
# l'icône et le lanceur comme un paquet.
#
#   scripts/release-linux.sh            construit les paquets de la version en cours
#   scripts/release-linux.sh v1.2.0     vérifie d'abord que le tag correspond à la version
#
# Le tag doit être « v » suivi de la version écrite dans Cargo.toml, package.json et
# src-tauri/tauri.conf.json : les trois fichiers donnent le numéro affiché par les paquets.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(node -p "require('./src-tauri/tauri.conf.json').version")"
tag="${1:-}"
if [ -n "$tag" ]; then
  for fichier in Cargo.toml package.json src-tauri/tauri.conf.json; do
    case "$fichier" in
      Cargo.toml) v="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)" ;;
      *) v="$(node -p "require('./$fichier').version")" ;;
    esac
    if [ "v$v" != "$tag" ]; then
      echo "Le tag $tag ne correspond pas à la version $v de $fichier." >&2
      echo "Mettez la même version dans Cargo.toml, package.json et src-tauri/tauri.conf.json." >&2
      exit 1
    fi
  done
fi

case "$(uname -m)" in
  x86_64) deb=amd64 rpm=x86_64 appimage=amd64 arch=x86_64 ;;
  aarch64|arm64) deb=arm64 rpm=aarch64 appimage=aarch64 arch=aarch64 ;;
  *) echo "Architecture non prise en charge : $(uname -m)" >&2; exit 1 ;;
esac

scripts/build-linux.sh

bundle=target/release/bundle
rm -rf dist
mkdir dist
cp "$bundle/deb/Paint_${version}_${deb}.deb" "dist/Paint_${deb}.deb"
cp "$bundle/rpm/Paint-${version}-1.${rpm}.rpm" "dist/Paint_${rpm}.rpm"
cp "$bundle/appimage/Paint_${version}_${appimage}.AppImage" "dist/Paint_${arch}.AppImage"

# Archive universelle : mêmes fichiers que le .deb (programme, icônes), plus install.sh.
tmp="$(mktemp -d)"
racine="$tmp/Paint-$version-$arch"
mkdir -p "$racine"
dpkg-deb -x "dist/Paint_${deb}.deb" "$tmp/deb"
cp -r "$tmp/deb/usr/bin" "$racine/bin"
mkdir -p "$racine/share/applications"
cp -r "$tmp/deb/usr/share/icons" "$racine/share/icons"
sed -e 's/{{name}}/Paint/' -e 's/{{exec}}/paint/g' -e 's/{{icon}}/paint/' src-tauri/paint.desktop \
  > "$racine/share/applications/paint.desktop"
install -m755 scripts/linux-install.sh "$racine/install.sh"
tar -C "$tmp" -czf "dist/Paint_linux_${arch}.tar.gz" "Paint-$version-$arch"
rm -rf "$tmp"

(cd dist && sha256sum Paint_* > "SHA256SUMS_${arch}")
ls -l dist
