#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

for tool in rsvg-convert magick; do
    command -v "$tool" >/dev/null || {
        echo "Missing required icon tool: $tool" >&2
        exit 1
    }
done

rsvg-convert -w 512 -h 512 assets/icon.svg -o assets/icon.png
magick assets/icon.png -define icon:auto-resize=256,128,64,48,32,24,16 resources/surround-tag.ico
echo "Refreshed assets/icon.png and resources/surround-tag.ico"
