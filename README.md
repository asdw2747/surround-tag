# SurroundTag

**SurroundTag** is a native Rust desktop application for tagging and embedding cover art into finished 5.1 Matroska Audio (`.mka`) files without re-encoding. It pairs original stereo masters with SurroundPacker / MKVToolNix output, writes compliant Matroska album and track tags, embeds front cover art, sets the first audio track as the default English track, and optionally renames files with collision checks and undo safety.

Built with `eframe`/`egui` 0.37 for Linux and Windows. Source metadata is read with `lofty` (pure Rust). Matroska edits are applied with `mkvpropedit` from MKVToolNix.

![SurroundTag icon](assets/icon.png)

## Workflow

1. **Load stereo masters** into the left pane — FLAC, MP3, M4A, WAV, or OGG. SurroundTag reads title, artist, album, album artist, track/disc numbers, year, genre, duration, and embedded front cover art. If no embedded art exists, it looks for `cover.jpg`, `folder.jpg`, or `front.png` in the same folder.
2. **Load finished 5.1 `.mka` files** into the right pane.
3. **Review automatic pairings.** The matcher combines track numbers, normalized/fuzzy filename similarity, tag titles, and audio duration. Click a master and then a target to pair manually; lock or unpair rows at any time.
4. **Edit tags and cover art** in the inspector. Album-level fields are shared across the batch; track-level fields are per master. Override the cover with any JPEG, PNG, or WebP image.
5. **Apply.** SurroundTag writes Matroska tags, embeds the cover attachment, and sets the stream flags with `mkvpropedit`. By default it also writes `folder.jpg` / `folder.png` beside each target for Kodi over SMB; existing folder art is backed up under `.surroundtag-backup`.
6. **Optionally rename** the `.mka` files with a configurable template. Collisions are checked before any write; the last rename batch can be undone.

## What gets written

### Tags

| Field | TargetTypeValue | Matroska tag |
|---|---:|---|
| Album title | 50 | `TITLE` |
| Album artist | 50 | `ARTIST` |
| Year | 50 | `DATE_RELEASED` |
| Genre | 50 | `GENRE` |
| Total tracks | 50 | `TOTAL_PARTS` |
| Track title | 30 | `TITLE` |
| Track artist | 30 | `ARTIST` |
| Track number | 30 | `PART_NUMBER` |
| Disc number | 30 | `_DISC_NUMBER` (private tag; Matroska has no assigned disc tag) |
| Cover art | — | Attachment `cover.jpg` / `cover.png`, description `Front cover` |

The default tag policy is **Merge**: SurroundTag updates only the managed fields and preserves unrelated tags such as `ENCODER`, `BPS`, `DURATION`, and ReplayGain. A **Replace all tags** policy is available in Settings.

### Stream flags

The first audio track is set as:

- `flag-default=1`
- `language=eng` (configurable)
- `name=5.1 Surround (E-AC-3 640 kbps)` (configurable)

If the file has multiple audio tracks, the remaining tracks are set to `flag-default=0`.

## Matching

Filename normalization removes technical surround/format noise such as `5.1`, `E-AC-3`, `640 kbps`, `surround`, `mka`, and `remux`, then compares:

- normalized filename similarity (`strsim` Jaro-Winkler),
- track numbers from tags and filenames,
- tag titles,
- container/audio duration.

Pairs at 0.85+ are **High** confidence, 0.65–0.85 are **Medium**, and below that are **Low** suggestions. Only High/Medium candidates are auto-assigned. Manual pairs are locked and survive re-runs.

## Requirements

### Runtime

- **Linux**: X11 or Wayland with OpenGL, plus MKVToolNix.
- **Windows**: `x86_64-pc-windows-msvc` static-CRT binary, plus MKVToolNix.
- **MKVToolNix** provides `mkvpropedit` (required), `mkvmerge` and `mkvextract` (used for introspection/verification).
- **ffprobe** is detected as an optional diagnostic tool; `mkvmerge` and `mkvextract` provide the authoritative target introspection and verification.
- **Kodi folder art** is enabled by default and can be disabled in Settings or the inspector.

On Linux, SurroundTag detects a Flatpak MKVToolNix install automatically:

```sh
flatpak install flathub org.bunkus.mkvtoolnix-gui
```

The app invokes it as `flatpak run --command=mkvpropedit org.bunkus.mkvtoolnix-gui …`. If `mkvpropedit` is on `PATH`, that path is preferred. Explicit paths can be set in **Settings**.

> Flatpak sandbox note: working files are staged under `~/.cache/surround-tag/`. Targets should live in locations visible to the Flatpak (`filesystems=host` usually covers the home directory). Files in a private `/tmp` may not be reachable from the sandbox.

### Build

- Rust 1.99.1 (or 1.95+).
- Linux GUI dependencies for `rfd`/`eframe`:
  - Debian/Ubuntu: `libgtk-3-dev libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev`
  - Arch: `gtk3 libxkbcommon`
- Windows cross-build on Linux: `cargo-xwin` and an LLVM resource compiler.

## Build

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

Linux binary:

```text
target/release/surround-tag
```

Windows static-CRT binary:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc
```

`.cargo/config.toml` sets `-C target-feature=+crt-static` for the MSVC target, so the resulting `surround-tag.exe` does not require a separate Visual C++ runtime.

### Integration test

The round-trip test copies a small 5.1 fixture, applies tags and cover art with the real `mkvpropedit`, verifies the result with `mkvextract`/`mkvmerge`, and checks rename/attachment replacement behavior:

```bash
SURROUNDTAG_REQUIRE_MKV=1 cargo test --test roundtrip_mka
```

Without MKVToolNix, the test prints a skip message.

### Full release pipeline

```bash
./scripts/build-release.sh
```

This formats, lints, tests, builds both targets, verifies the Windows executable has no dynamic VC runtime imports, and writes archives and checksums to `dist/`.

## Configuration and data

SurroundTag stores settings in:

- Linux: `$XDG_CONFIG_HOME/surround-tag/config.json` or `~/.config/surround-tag/config.json`
- Windows: `%APPDATA%\surround-tag\config.json`

The rename undo manifest lives next to it:

```text
config.json
undo_last_batch.json
```

Job artifacts (generated tag XML, cover files, logs, and original tags for restoration) are kept under:

- Linux: `~/.cache/surround-tag/jobs/`
- Windows: `%LOCALAPPDATA%\surround-tag\cache\jobs\`

## Design

- Background `#131720`, cards `#1B2130`, matte `#0B0E14`.
- Cyan `#38BDF8` for primary actions and matching; amber `#F59E0B` for 5.1/target accents and warnings.
- Text `#E6EDF7`, muted `#8C9AB0`.
- No web view. No audio re-encoding. All container edits are performed in place by MKVToolNix.

## License

MIT — Copyright (c) 2026 Prisma Chill. See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

MKVToolNix is an optional external GPL-2.0 tool. It is not bundled or linked into SurroundTag.
