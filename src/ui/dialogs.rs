//! Modal windows: settings, help, about, and the apply report.

use egui::{RichText, Stroke};

use crate::app::SurroundTagApp;
use crate::model::ApplyState;
use crate::theme;

pub fn windows(app: &mut SurroundTagApp, ctx: &egui::Context) {
    if app.show_settings {
        super::settings::settings_window(app, ctx);
    }
    if app.show_about {
        super::settings::about_window(app, ctx);
    }
    if app.show_help {
        super::settings::help_window(app, ctx);
    }
    if app.show_report {
        report_window(app, ctx);
    }
}

fn report_window(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let Some(summary) = app.summary.clone() else {
        app.show_report = false;
        return;
    };
    let mut open = app.show_report;
    let mut close = false;
    egui::Window::new("Apply Report")
        .open(&mut open)
        .default_width(760.0)
        .default_height(520.0)
        .resizable(true)
        .frame(theme::card())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Batch result")
                        .size(14.0)
                        .strong()
                        .color(theme::TEXT),
                );
                theme::pill(ui, format!("{} done", summary.succeeded), theme::SUCCESS);
                theme::pill(ui, format!("{} warnings", summary.warnings), theme::AMBER);
                theme::pill(ui, format!("{} failed", summary.failed), theme::ERROR);
                theme::pill(ui, format!("{} renamed", summary.renamed), theme::CYAN);
                ui.label(
                    RichText::new(format!("{:.1}s", summary.elapsed_secs))
                        .monospace()
                        .size(10.0)
                        .color(theme::MUTED),
                );
            });
            if let Some(error) = &app.manifest_error {
                ui.label(
                    RichText::new(format!("Undo manifest warning: {error}"))
                        .size(10.0)
                        .color(theme::AMBER),
                );
            }
            ui.add_space(6.0);
            ui.separator();

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for result in &summary.results {
                        egui::Frame::new()
                            .fill(theme::CANVAS_BG)
                            .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
                            .corner_radius(egui::CornerRadius::same(8))
                            .inner_margin(egui::Margin::same(8))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (label, color) = state_style(result.state);
                                    theme::pill(ui, label, color);
                                    ui.label(
                                        RichText::new(
                                            result
                                                .target
                                                .file_name()
                                                .and_then(|name| name.to_str())
                                                .unwrap_or("target"),
                                        )
                                        .size(11.5)
                                        .strong()
                                        .color(theme::TEXT),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                RichText::new(&result.message)
                                                    .size(9.5)
                                                    .color(color),
                                            );
                                        },
                                    );
                                });
                                if let Some(renamed) = &result.renamed_to {
                                    ui.label(
                                        RichText::new(format!(
                                            "Renamed → {}",
                                            renamed
                                                .file_name()
                                                .and_then(|name| name.to_str())
                                                .unwrap_or_default()
                                        ))
                                        .monospace()
                                        .size(9.5)
                                        .color(theme::CYAN),
                                    );
                                }
                                if let Some(folder_art) = &result.folder_art {
                                    ui.label(
                                        RichText::new(format!(
                                            "Kodi folder art → {}",
                                            folder_art.display()
                                        ))
                                        .monospace()
                                        .size(9.5)
                                        .color(theme::AMBER),
                                    );
                                }
                                for warning in &result.warnings {
                                    ui.label(
                                        RichText::new(format!("• {warning}"))
                                            .size(9.5)
                                            .color(theme::AMBER),
                                    );
                                }
                                if !result.log.trim().is_empty() {
                                    ui.collapsing("Log", |ui| {
                                        let mut log = result.log.clone();
                                        ui.add(
                                            egui::TextEdit::multiline(&mut log)
                                                .font(egui::TextStyle::Monospace)
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(8),
                                        );
                                    });
                                }
                            });
                        ui.add_space(4.0);
                    }
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::ghost_button(ui, "Copy Full Log").clicked() {
                    let mut log = String::new();
                    for result in &summary.results {
                        log.push_str(&format!(
                            "=== {} ===\n{}\n",
                            result.target.display(),
                            result.log
                        ));
                    }
                    ctx.copy_text(log);
                }
                if app.undo_available && theme::amber_button(ui, "Undo Last Rename").clicked() {
                    app.undo_last();
                    close = true;
                }
                if theme::primary_button(ui, "Close").clicked() {
                    close = true;
                }
            });
        });
    app.show_report = open && !close;
}

fn state_style(state: ApplyState) -> (&'static str, egui::Color32) {
    match state {
        ApplyState::Done => ("DONE", theme::SUCCESS),
        ApplyState::Warning => ("WARNING", theme::AMBER),
        ApplyState::Failed => ("FAILED", theme::ERROR),
        ApplyState::Applying => ("APPLYING", theme::CYAN),
        ApplyState::Verifying => ("VERIFYING", theme::CYAN),
        ApplyState::Pending => ("PENDING", theme::MUTED),
    }
}
