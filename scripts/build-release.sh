#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
windows_target="x86_64-pc-windows-msvc"
stage="$(mktemp -d)"
trap 'rm -rf "$stage"' EXIT

for tool in cargo cargo-xwin llvm-readobj tar zip sha256sum; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "ERROR: required release tool '$tool' is not installed or not on PATH." >&2
        exit 1
    fi
done

echo "==> Formatting"
cargo fmt --check
echo "==> Clippy"
cargo clippy --all-targets -- -D warnings
echo "==> Tests"
cargo test
echo "==> Linux release"
cargo build --release
echo "==> Windows static-CRT release"
cargo xwin build --release --target "$windows_target"

exe="target/${windows_target}/release/surround-tag.exe"
if llvm-readobj --coff-imports "$exe" 2>/dev/null | grep -Eiq 'VCRUNTIME|MSVCP'; then
    echo "ERROR: Windows binary still imports the dynamic VC runtime." >&2
    exit 1
fi
echo "Windows binary has no dynamic VC runtime imports."

mkdir -p dist
linux_dir="$stage/SurroundTag-${version}-linux-x86_64"
windows_dir="$stage/SurroundTag-${version}-windows-x86_64"
mkdir -p "$linux_dir/assets" "$linux_dir/packaging" "$windows_dir"

cp target/release/surround-tag "$linux_dir/"
cp LICENSE README.md THIRD_PARTY_NOTICES.md "$linux_dir/"
cp assets/icon.png "$linux_dir/assets/"
cp packaging/surround-tag.desktop packaging/surround-tag.appdata.xml "$linux_dir/packaging/"

cp "$exe" "$windows_dir/surround-tag.exe"
cp "$exe" "dist/SurroundTag-${version}-windows-x86_64.exe"
cp LICENSE README.md THIRD_PARTY_NOTICES.md "$windows_dir/"
cp assets/icon.png "$windows_dir/"

tar -C "$linux_dir" -czf "dist/SurroundTag-${version}-linux-x86_64.tar.gz" .
zip -q -j "dist/SurroundTag-${version}-windows-x86_64.zip" \
    "$windows_dir/surround-tag.exe" "$windows_dir/LICENSE" \
    "$windows_dir/README.md" "$windows_dir/THIRD_PARTY_NOTICES.md" \
    "$windows_dir/icon.png"

checksums=(
    "SurroundTag-${version}-linux-x86_64.tar.gz"
    "SurroundTag-${version}-windows-x86_64.zip"
    "SurroundTag-${version}-windows-x86_64.exe"
)
if [[ -f "dist/SurroundTag-${version}-x86_64.AppImage" ]]; then
    checksums+=("SurroundTag-${version}-x86_64.AppImage")
fi
(cd dist && sha256sum "${checksums[@]}" > "SurroundTag-${version}-SHA256SUMS")

echo
echo "Release archives written to dist/"
ls -lh dist/
