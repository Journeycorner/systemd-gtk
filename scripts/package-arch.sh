#!/usr/bin/env bash
# Build a pacman-managed package from this checkout, including uncommitted changes.
set -euo pipefail

mode=${1:---build}
if [[ $# -gt 1 || ! $mode =~ ^(--build|--install|--prepare-only|--help)$ ]]; then
  echo "Usage: $0 [--build|--install|--prepare-only]" >&2
  exit 2
fi
if [[ $mode == --help ]]; then
  echo "Usage: $0 [--build|--install|--prepare-only]"
  echo "Default: build and test a local package. --install also invokes pacman."
  exit 0
fi

repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
mkdir -p "$repo/target/arch-packages"
stage=$(mktemp -d "$repo/target/arch-packages/build.XXXXXX")
cp "$repo/build/PKGBUILD" "$stage/PKGBUILD"
tar -czf "$stage/systemd-gtk.tar.gz" -C "$repo" \
  Cargo.toml Cargo.lock rust-toolchain.toml build.rs LICENSE README.md .cargo src data docs tests
echo "Package workspace: $stage"
if [[ $mode == --prepare-only ]]; then
  exit 0
fi

export CARGO_TARGET_DIR="$repo/target"
cd "$stage"
args=(--syncdeps)
if [[ $mode == --install ]]; then
  args+=(--install)
fi
exec makepkg "${args[@]}"
