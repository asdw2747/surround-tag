//! Window icon loading with a deterministic generated fallback.

pub fn load_icon() -> egui::IconData {
    const ICON_BYTES: &[u8] = include_bytes!("../assets/icon.png");
    if let Ok(image) = image::load_from_memory(ICON_BYTES) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        return egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        };
    }
    fallback_icon()
}

fn fallback_icon() -> egui::IconData {
    const SIZE: usize = 256;
    let mut rgba = vec![0u8; SIZE * SIZE * 4];
    let background = [0x13, 0x17, 0x20, 255];
    let cyan = [0x38, 0xBD, 0xF8, 255];
    let amber = [0xF5, 0x9E, 0x0B, 255];
    let mut put = |x: usize, y: usize, color: [u8; 4]| {
        if x < SIZE && y < SIZE {
            let index = (y * SIZE + x) * 4;
            rgba[index..index + 4].copy_from_slice(&color);
        }
    };
    for y in 0..SIZE {
        for x in 0..SIZE {
            put(x, y, background);
        }
    }
    for offset in 0..SIZE {
        for thickness in 0..2 {
            put(offset, 74 + thickness, cyan);
            put(offset, 166 + thickness, cyan);
            put(74 + thickness, offset, cyan);
            put(166 + thickness, offset, cyan);
        }
    }
    for y in 120..=148 {
        for x in 120..=148 {
            put(x, y, amber);
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
