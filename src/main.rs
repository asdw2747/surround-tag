#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use eframe::egui;
use surround_tag::app::SurroundTagApp;
use surround_tag::icon;

fn main() -> eframe::Result {
    install_panic_hook();

    let startup_paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    let title = format!("SurroundTag v{}", env!("CARGO_PKG_VERSION"));
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_app_id("surround-tag")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([1120.0, 740.0])
            .with_icon(icon::load_icon()),
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };

    let result = eframe::run_native(
        &title,
        native_options,
        Box::new(move |cc| Ok(Box::new(SurroundTagApp::new(cc, startup_paths)))),
    );

    if let Err(error) = &result {
        let message = format!(
            "SurroundTag could not start the graphical interface.\n\n{error}\n\n\
             On Linux, check that an X11/Wayland OpenGL session is available. On Windows, \
             verify the executable was built with the included static-CRT configuration."
        );
        eprintln!("{message}");
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("SurroundTag - Startup Error")
            .set_description(&message)
            .show();
    }
    result
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        default_hook(panic_info);
        let message = format!("SurroundTag encountered an unexpected error:\n\n{panic_info}");
        let _ = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("SurroundTag - Unexpected Error")
            .set_description(&message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }));
}
