#!/usr/bin/env bash
# Installe Paint depuis l'archive .tar.gz, sur n'importe quelle distribution Linux :
# programme, icône dans le menu des applications et « Ouvrir avec » pour les images.
#
#   sudo ./install.sh              pour tous les utilisateurs (/usr/local)
#   ./install.sh --user            pour vous seul (~/.local), sans droits administrateur
#   ./install.sh --uninstall       désinstalle (avec --user pour une installation personnelle)
#
# Paint a besoin de WebKitGTK 4.1, fourni par la distribution (voir le message en cas d'absence).
set -euo pipefail
cd "$(dirname "$0")"

prefix=/usr/local
action=install
for arg in "$@"; do
  case "$arg" in
    --user) prefix="$HOME/.local" ;;
    --uninstall) action=uninstall ;;
    -h|--help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "Option inconnue : $arg" >&2; exit 2 ;;
  esac
done

if [ "$prefix" = /usr/local ] && [ "$(id -u)" -ne 0 ]; then
  echo "Installation pour tous : relancez avec sudo, ou utilisez ./install.sh --user." >&2
  exit 1
fi

apps="$prefix/share/applications"
icons="$prefix/share/icons/hicolor"

refresh() {
  command -v update-desktop-database >/dev/null && update-desktop-database -q "$apps" 2>/dev/null || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$icons" 2>/dev/null || true
}

if [ "$action" = uninstall ]; then
  rm -f "$prefix/bin/paint" "$apps/paint.desktop"
  rm -f "$icons"/*/apps/paint.png
  refresh
  echo "Paint est désinstallé de $prefix."
  exit 0
fi

install -Dm755 bin/paint "$prefix/bin/paint"
for dir in share/icons/hicolor/*/apps; do
  install -Dm644 "$dir/paint.png" "$prefix/$dir/paint.png"
done
sed "s|^Exec=paint|Exec=$prefix/bin/paint|" share/applications/paint.desktop > /tmp/paint.desktop.$$
install -Dm644 /tmp/paint.desktop.$$ "$apps/paint.desktop"
rm -f /tmp/paint.desktop.$$
refresh
echo "Paint est installé dans $prefix : il apparaît dans le menu des applications."

if ! { ldconfig -p 2>/dev/null || true; } | grep -q 'libwebkit2gtk-4.1\.so'; then
  echo
  echo "Attention : WebKitGTK 4.1 manque. Installez-le avec le gestionnaire de paquets :"
  echo "  Debian, Ubuntu, Mint : sudo apt install libwebkit2gtk-4.1-0"
  echo "  Fedora               : sudo dnf install webkit2gtk4.1"
  echo "  Arch, Manjaro        : sudo pacman -S webkit2gtk-4.1"
  echo "  openSUSE             : sudo zypper install libwebkit2gtk-4_1-0"
fi
