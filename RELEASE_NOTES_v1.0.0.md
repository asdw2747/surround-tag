# SurroundTag v1.0.0

**SurroundTag** is a native Rust desktop application for tagging and embedding cover art into finished 5.1 Matroska Audio (`.mka`) files without re-encoding. It pairs original stereo masters with SurroundPacker / MKVToolNix output, writes compliant Matroska album and track tags, embeds front cover art, sets the first audio track as the default English track, and optionally renames files with collision checks and undo safety.

## Highlights

- Two-pane stereo-master → 5.1 `.mka` workflow with automatic fuzzy matching.
- Title-first pairing with track numbers, duration comparison, ambiguity protection, and stitched-track detection.
- Pure-Rust metadata and cover extraction with `lofty` (FLAC, MP3, M4A, WAV, OGG).
- Matroska XML tags at `TargetTypeValue 50` (album) and `30` (track).
- `mkvpropedit` tagging without re-encoding.
- Cover attachment add/replace without duplicates.
- Kodi `folder.jpg` / `folder.png` sidecar writing, enabled by default.
- Configurable rename templates with Windows-safe sanitization, case-insensitive collision checks, and undo.
- Linux and Windows support with the Prisma Chill dark-slate interface.

## Requirements

- MKVToolNix (`mkvpropedit`, `mkvmerge`, `mkvextract`).
- Linux: X11 or Wayland with OpenGL.
- Windows: 64-bit Windows; the release binary uses the static MSVC CRT.
- `ffprobe` is optional and diagnostic only.

## Assets

- `SurroundTag-1.0.0-windows-x86_64.exe` — Windows x86_64 static-CRT executable.
- `SurroundTag-1.0.0-x86_64.AppImage` — Linux AppImage.
- `SurroundTag-1.0.0-windows-x86_64.zip` — Windows executable plus docs.
- `SurroundTag-1.0.0-linux-x86_64.tar.gz` — Linux binary plus packaging files.
- `SurroundTag-1.0.0-SHA256SUMS` — checksums for all release assets.

## Verification

- `cargo fmt --check` clean.
- `cargo clippy --all-targets -- -D warnings` clean.
- 43 tests passed, including a real MKVToolNix round-trip with special-character metadata, cover replacement, stream flags, rename, and Kodi folder art.
- Windows binary verified with `llvm-readobj` to have no `VCRUNTIME` or `MSVCP` imports.

## License

MIT — Copyright (c) 2026 Prisma Chill. See `LICENSE` and `THIRD_PARTY_NOTICES.md`.

MKVToolNix is an optional external GPL-2.0 tool and is not bundled with SurroundTag.
