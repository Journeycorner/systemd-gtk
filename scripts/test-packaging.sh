#!/usr/bin/env bash
# Validate staged files only: no pacman invocation or host desktop installation.
set -euo pipefail
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo"
for script in build/PKGBUILD build/aur/PKGBUILD scripts/*.sh; do
  bash -n "$script"
done
diff -u build/aur/.SRCINFO <(cd build/aur && makepkg --printsrcinfo)
stage=$(mktemp -d)
echo "Packaging test workspace (retained): $stage"

(
  source build/PKGBUILD
  srcdir="$repo"
  pkgdir="$stage/package"
  CARGO_TARGET_DIR="$repo/target"
  package
)
test -x "$stage/package/usr/bin/systemd-gtk"
cmp data/app-icon.png "$stage/package/usr/share/icons/hicolor/512x512/apps/com.journeycorner.systemd-gtk.png"
desktop-file-validate "$stage/package/usr/share/applications/com.journeycorner.systemd-gtk.desktop"
bash scripts/package-arch.sh --prepare-only
cargo package --locked --allow-dirty --list >/dev/null
