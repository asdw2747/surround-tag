# Third-Party Notices

SurroundTag is MIT licensed. It depends on the following crates and external
tools. This file summarizes licenses; the authoritative license texts are in
each crate's source distribution and on crates.io.

## Rust dependencies

| Component | Version | License |
|---|---:|---|
| `eframe` / `egui` / `epaint` | 0.37.0 | MIT OR Apache-2.0 |
| `lofty` | 0.25.5 | MIT OR Apache-2.0 |
| `image` | 0.25.10 | MIT OR Apache-2.0 |
| `quick-xml` | 0.41.0 | MIT |
| `rfd` | 0.18.0 | MIT |
| `serde` / `serde_json` | 1.x | MIT OR Apache-2.0 |
| `strsim` | 0.11.1 | MIT |
| `unicode-normalization` | 0.1.26 | MIT OR Apache-2.0 AND Unicode-3.0 |
| `which` | 8.0.6 | MIT |
| `walkdir` | 2.5.1 | MIT OR Unlicense |
| `dirs` | 6.0.0 | MIT OR Apache-2.0 |
| `tempfile` | 3.27.0 | MIT OR Apache-2.0 |
| `open` | 5.x | MIT |
| `embed-resource` | 3.0.12 | MIT |

## Fonts

The default egui fonts (Ubuntu-Light, NotoEmoji, Hack) are distributed under
their respective licenses (Ubuntu Font License, SIL Open Font License, and the
Hack license). When the `system_fonts` feature is enabled, egui loads fonts
from the host operating system; those fonts remain under their system licenses.

## External tools

### MKVToolNix / mkvpropedit

SurroundTag invokes `mkvpropedit`, `mkvmerge`, and `mkvextract` at runtime when
they are installed by the user. MKVToolNix is licensed under **GPL-2.0-or-later**.
SurroundTag does not bundle, link, or redistribute MKVToolNix binaries or source
code. The tools are optional external dependencies, equivalent to invoking them
from a shell.

### ffprobe / FFmpeg

`ffprobe` is an optional diagnostic tool detected by SurroundTag. FFmpeg is licensed
under LGPL-2.1-or-later (with optional GPL components depending on the build).
SurroundTag does not bundle or link FFmpeg.

## Icons and artwork

The SurroundTag icon is original artwork created for this project and is
licensed under the same MIT license as the application.
