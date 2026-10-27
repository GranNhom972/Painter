## Installer Paint sous Linux

Choisissez le fichier de votre distribution et de votre processeur : **x86_64** (PC Intel ou AMD) ou
**arm64** (Raspberry Pi 4/5, ordinateurs ARM). La commande `uname -m` vous le dit.

| Distribution | x86_64 | arm64 | Installation |
|---|---|---|---|
| Debian, Ubuntu, Linux Mint, Pop!_OS… | `Paint_amd64.deb` | `Paint_arm64.deb` | `sudo apt install ./Paint_amd64.deb` |
| Fedora, openSUSE, RHEL… | `Paint_x86_64.rpm` | `Paint_aarch64.rpm` | `sudo dnf install ./Paint_x86_64.rpm` |
| Arch, Manjaro, Gentoo, toutes les autres | `Paint_linux_x86_64.tar.gz` | `Paint_linux_aarch64.tar.gz` | voir ci-dessous |
| Sans installation | `Paint_x86_64.AppImage` | `Paint_aarch64.AppImage` | `chmod +x Paint_*.AppImage && ./Paint_*.AppImage` |

Archive `.tar.gz` (pose le programme, l'icône et le lanceur comme un paquet) :

```bash
tar xzf Paint_linux_x86_64.tar.gz
cd Paint-*/
sudo ./install.sh          # ou ./install.sh --user, sans droits administrateur
```

Paint apparaît ensuite dans le menu des applications, avec son icône, et dans « Ouvrir avec »
pour les images PNG, JPEG, BMP, GIF, TIFF, WEBP et ICO.

L'AppImage ne démarre pas (FUSE absent, fréquent sur les distributions récentes) ? Lancez-la avec
`--appimage-extract-and-run`. Sommes de contrôle : `sha256sum -c SHA256SUMS --ignore-missing`.
