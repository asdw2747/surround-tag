//! SurroundTag core library: audio probing, fuzzy pairing, Matroska tag
//! planning, mkvpropedit execution, verification, and the egui UI.

pub mod app;
pub mod audio;
pub mod config;
pub mod icon;
pub mod matching;
pub mod mkv;
pub mod model;
pub mod scan;
pub mod tags;
pub mod template;
pub mod theme;
pub mod ui;
pub mod worker;

pub const APP_NAME: &str = "SurroundTag";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
