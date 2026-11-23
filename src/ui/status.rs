//! Status footer with progress, notices, and undo access.

use egui::{RichText, Stroke};

use crate::app::SurroundTagApp;
use crate::theme;

pub fn status_bar(app: &mut SurroundTagApp, root: &mut egui::Ui) {
    egui::Panel::bottom("surroundtag_status")
        .exact_size(32.0)
        .frame(
            egui::Frame::new()
                .fill(theme::BG)
                .inner_margin(egui::Margin::symmetric(14, 5))
                .stroke(Stroke::new(1.0, theme::BORDER_SOFT)),
        )
        .show(root, |ui| {
            ui.horizontal(|ui| {
                let dot = if app.job.is_some() {
                    theme::CYAN
                } else if app.tools.can_apply() {
                    theme::SUCCESS
                } else {
                    theme::AMBER
                };
                ui.label(RichText::new("●").size(8.0).color(dot));

                if let Some(notice) = &app.notice {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&notice.text).size(10.5).color(notice.color),
                        )
                        .truncate(),
                    );
                } else {
                    ui.label(
                        RichText::new(format!(
                            "{} masters · {} targets · {} paired",
                            app.sources.len(),
                            app.targets.len(),
                            app.pairings.len()
                        ))
                        .size(10.5)
                        .color(theme::MUTED),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let undo = ui.add_enabled(
                        app.undo_available,
                        egui::Button::new(
                            RichText::new("Undo Last Rename")
                                .size(12.0)
                                .color(theme::TEXT),
                        )
                        .fill(theme::CARD_ALT)
                        .stroke(Stroke::new(1.0, theme::BORDER))
                        .corner_radius(egui::CornerRadius::same(7)),
                    );
                    if undo
                        .on_hover_text("Restore the file names from the last apply batch")
                        .clicked()
                    {
                        app.undo_last();
                    }
                    if app.summary.is_some() && theme::ghost_button(ui, "Last Report").clicked() {
                        app.show_report = true;
                    }
                    if let Some(job) = &app.job {
                        ui.add(
                            egui::ProgressBar::new(job.fraction())
                                .desired_width(160.0)
                                .text(format!("{}/{}", job.done, job.total)),
                        );
                        ui.label(
                            RichText::new(format!(
                                "Applying {}",
                                job.target
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .unwrap_or("target")
                            ))
                            .size(10.0)
                            .color(theme::CYAN),
                        );
                    } else if let Some(summary) = &app.summary {
                        ui.label(
                            RichText::new(format!(
                                "done {} · warnings {} · failed {}",
                                summary.succeeded, summary.warnings, summary.failed
                            ))
                            .monospace()
                            .size(10.0)
                            .color(if summary.failed > 0 {
                                theme::ERROR
                            } else if summary.warnings > 0 {
                                theme::AMBER
                            } else {
                                theme::SUCCESS
                            }),
                        );
                    }
                });
            });
        });
}
