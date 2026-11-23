//! Header and primary toolbar.

use egui::{RichText, Stroke, Ui};

use crate::app::SurroundTagApp;
use crate::theme;

pub fn top_bar(app: &mut SurroundTagApp, root: &mut egui::Ui) {
    let ctx = root.ctx().clone();
    egui::Panel::top("surroundtag_top")
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(14, 8))
                .stroke(Stroke::new(1.0, theme::BORDER_SOFT)),
        )
        .show(root, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("SurroundTag")
                        .size(20.0)
                        .strong()
                        .color(theme::TEXT),
                );
                theme::pill(ui, format!("v{}", crate::APP_VERSION), theme::CYAN);
                ui.label(
                    RichText::new("5.1 Matroska tagging & cover art")
                        .size(10.5)
                        .color(theme::MUTED),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::ghost_button(ui, "About").clicked() {
                        app.show_about = true;
                    }
                    if theme::ghost_button(ui, "Help").clicked() {
                        app.show_help = true;
                    }
                    if theme::ghost_button(ui, "Settings").clicked() {
                        app.open_settings();
                    }
                });
            });

            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if app.tools.can_apply() {
                    theme::pill(ui, "MKVPROPEDIT", theme::CYAN);
                    ui.add(
                        egui::Label::new(
                            RichText::new(app.tools.summary())
                                .monospace()
                                .size(10.0)
                                .color(theme::MUTED),
                        )
                        .truncate(),
                    );
                } else {
                    theme::pill(ui, "MKVPROPEDIT MISSING", theme::ERROR);
                    if theme::ghost_button(ui, "Locate…").clicked() {
                        app.open_settings();
                    }
                }
                ui.separator();
                theme::pill(ui, "SOURCE", theme::MUTED);
                if theme::primary_button(ui, "Load Masters").clicked() {
                    super::panes::open_source_dialog(app, &ctx);
                }
                if theme::ghost_button(ui, "Load Master Folder").clicked() {
                    super::panes::open_source_folder(app, &ctx);
                }
                ui.separator();

                theme::pill(ui, "TARGET", theme::MUTED);
                if theme::amber_button(ui, "Load .mka Files").clicked() {
                    super::panes::open_target_dialog(app, &ctx);
                }
                if theme::ghost_button(ui, "Load .mka Folder").clicked() {
                    super::panes::open_target_folder(app, &ctx);
                }
                ui.separator();

                theme::pill(ui, "MATCH", theme::MUTED);
                if theme::ghost_button(ui, "Auto-pair").clicked() {
                    app.auto_pair_now();
                }
                if theme::ghost_button(ui, "Clear Pairings").clicked() {
                    app.clear_pairings();
                }
                if theme::ghost_button(ui, "Clear Workspace").clicked() {
                    app.clear_all();
                }
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "{} masters · {} targets · {} pairs",
                        app.sources.len(),
                        app.targets.len(),
                        app.pairings.len()
                    ))
                    .monospace()
                    .size(10.0)
                    .color(theme::MUTED),
                );
            });

            for note in &app.tools.notes {
                ui.label(RichText::new(note).size(10.0).color(theme::AMBER));
            }
        });
}

pub fn tool_badge(ui: &mut Ui, app: &SurroundTagApp) {
    if app.tools.can_apply() {
        theme::pill(ui, "READY", theme::SUCCESS);
        ui.label(
            RichText::new(app.tools.summary())
                .monospace()
                .size(10.0)
                .color(theme::MUTED),
        );
    } else {
        theme::pill(ui, "TOOL MISSING", theme::ERROR);
    }
}
