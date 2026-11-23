#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

version="$(grep '^version' Cargo.toml | head -1 | cut -d'"' -f2)"
appdir="$repo_root/target/appimage/SurroundTag.AppDir"
app_name="SurroundTag-${version}-x86_64.AppImage"

cargo build --release

rm -rf "$appdir"
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/applications" "$appdir/usr/share/icons/hicolor/512x512/apps"
cp target/release/surround-tag "$appdir/usr/bin/surround-tag"
cp assets/icon.png "$appdir/surround-tag.png"
cp assets/icon.png "$appdir/usr/share/icons/hicolor/512x512/apps/surround-tag.png"
cp packaging/surround-tag.desktop "$appdir/surround-tag.desktop"
cp packaging/surround-tag.desktop "$appdir/usr/share/applications/surround-tag.desktop"
ln -sf surround-tag.png "$appdir/.DirIcon"

cat > "$appdir/AppRun" <<'EOF'
#!/bin/sh
HERE="$(dirname "$(readlink -f "${0}")")"
exec "${HERE}/usr/bin/surround-tag" "$@"
EOF
chmod +x "$appdir/AppRun"

tool="${APPIMAGETOOL:-$repo_root/target/appimage/appimagetool}"
if [[ ! -x "$tool" && -x "$HOME/.cache/comiccrate-appimage/appimagetool-x86_64.AppImage" ]]; then
    tool="$HOME/.cache/comiccrate-appimage/appimagetool-x86_64.AppImage"
fi
if [[ ! -x "$tool" ]]; then
    curl -fL -o "$tool" \
        https://github.com/AppImage/appimagetool/releases/download/1.9.0/appimagetool-x86_64.AppImage
    chmod +x "$tool"
fi

mkdir -p dist
ARCH=x86_64 "$tool" --appimage-extract-and-run "$appdir" "dist/$app_name"
sha256sum "dist/$app_name" > "dist/${app_name}.sha256"
echo "Wrote dist/$app_name"
