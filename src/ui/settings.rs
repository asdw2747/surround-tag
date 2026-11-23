//! Settings window.

use std::path::PathBuf;

use egui::RichText;

use crate::app::SurroundTagApp;
use crate::config::CollisionPolicy;
use crate::mkv::ToolSet;
use crate::tags::TagPolicy;
use crate::template;
use crate::theme;

pub fn settings_window(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut open = app.show_settings;
    let mut close = false;
    egui::Window::new("SurroundTag Settings")
        .open(&mut open)
        .default_width(680.0)
        .resizable(true)
        .frame(theme::card())
        .show(ctx, |ui| {
            ui.label(
                RichText::new("External tools")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            theme::hint(
                ui,
                "Leave a path empty for automatic detection. On Linux, a Flatpak MKVToolNix install is detected automatically.",
            );
            egui::Grid::new("tool_paths")
                .num_columns(2)
                .spacing([8.0, 5.0])
                .show(ui, |ui| {
                    path_field(ui, "mkvpropedit", &mut app.settings_draft.mkvpropedit_path);
                    ui.end_row();
                    path_field(ui, "mkvmerge", &mut app.settings_draft.mkvmerge_path);
                    ui.end_row();
                    path_field(ui, "mkvextract", &mut app.settings_draft.mkvextract_path);
                    ui.end_row();
                    path_field(ui, "ffprobe", &mut app.settings_draft.ffprobe_path);
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                if theme::ghost_button(ui, "Auto-detect now").clicked() {
                    let detected = ToolSet::detect(&crate::config::AppConfig::default());
                    assign_detected(&mut app.settings_draft.mkvpropedit_path, detected.mkvpropedit);
                    assign_detected(&mut app.settings_draft.mkvmerge_path, detected.mkvmerge);
                    assign_detected(&mut app.settings_draft.mkvextract_path, detected.mkvextract);
                    assign_detected(&mut app.settings_draft.ffprobe_path, detected.ffprobe);
                    app.set_notice("Tool paths refreshed from auto-detection.", theme::CYAN);
                }
                if theme::ghost_button(ui, "Clear overrides").clicked() {
                    app.settings_draft.mkvpropedit_path = None;
                    app.settings_draft.mkvmerge_path = None;
                    app.settings_draft.mkvextract_path = None;
                    app.settings_draft.ffprobe_path = None;
                }
                ui.label(
                    RichText::new(app.tools.summary())
                        .monospace()
                        .size(9.5)
                        .color(theme::MUTED),
                );
            });
            for note in &app.tools.notes {
                ui.label(RichText::new(note).size(9.5).color(theme::AMBER));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new("Track flags")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            egui::Grid::new("track_flags")
                .num_columns(2)
                .spacing([8.0, 5.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Track name").size(10.5).color(theme::MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings_draft.track_name)
                            .desired_width(420.0)
                            .hint_text(crate::config::DEFAULT_TRACK_NAME),
                    );
                    ui.end_row();
                    ui.label(RichText::new("Language").size(10.5).color(theme::MUTED));
                    ui.add(
                        egui::TextEdit::singleline(&mut app.settings_draft.language)
                            .desired_width(80.0)
                            .hint_text("eng"),
                    );
                    ui.end_row();
                });
            ui.checkbox(
                &mut app.settings_draft.set_default,
                "Mark audio track 1 as the default track",
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new("Tags & cover art")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Tag policy").size(10.5).color(theme::MUTED));
                egui::ComboBox::from_id_salt("tag_policy")
                    .selected_text(match app.settings_draft.tag_policy {
                        TagPolicy::Merge => "Merge (preserve unrelated tags)",
                        TagPolicy::Replace => "Replace all tags",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.settings_draft.tag_policy,
                            TagPolicy::Merge,
                            "Merge (preserve unrelated tags)",
                        );
                        ui.selectable_value(
                            &mut app.settings_draft.tag_policy,
                            TagPolicy::Replace,
                            "Replace all tags",
                        );
                    });
            });
            theme::hint(
                ui,
                "Merge updates the managed album/track fields and keeps existing encoder, BPS, ReplayGain, and other tags.",
            );
            ui.checkbox(
                &mut app.settings_draft.kodi_folder_art,
                "Also save folder.jpg in target folder (for Kodi)",
            );
            theme::hint(
                ui,
                "Writes the detected front cover as folder.jpg or folder.png beside the .mka file. Existing folder art is backed up under .surroundtag-backup.",
            );

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                RichText::new("Rename & safety")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            ui.checkbox(
                &mut app.settings_draft.rename_enabled,
                "Rename .mka files after a successful apply",
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("Template").size(10.5).color(theme::MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut app.settings_draft.rename_template)
                        .desired_width(420.0)
                        .font(egui::TextStyle::Monospace),
                );
                egui::ComboBox::from_id_salt("template_presets")
                    .selected_text("Presets…")
                    .show_ui(ui, |ui| {
                        for (label, value) in template::PRESETS {
                            if ui.selectable_label(false, *label).clicked() {
                                app.settings_draft.rename_template = (*value).to_owned();
                            }
                        }
                    });
            });
            if let Err(error) = template::validate(&app.settings_draft.rename_template) {
                ui.label(RichText::new(error).size(10.0).color(theme::ERROR));
            } else {
                ui.label(
                    RichText::new(
                        "Tokens: {track:02} {disc} {title} {artist} {album} {album_artist} {year} {ext}",
                    )
                    .monospace()
                    .size(9.5)
                    .color(theme::MUTED),
                );
            }
            ui.horizontal(|ui| {
                ui.label(RichText::new("Collision").size(10.5).color(theme::MUTED));
                egui::ComboBox::from_id_salt("collision_policy")
                    .selected_text(app.settings_draft.collision_policy.label())
                    .show_ui(ui, |ui| {
                        for policy in [
                            CollisionPolicy::Skip,
                            CollisionPolicy::Suffix,
                            CollisionPolicy::Overwrite,
                        ] {
                            ui.selectable_value(
                                &mut app.settings_draft.collision_policy,
                                policy,
                                policy.label(),
                            );
                        }
                    });
                ui.checkbox(
                    &mut app.settings_draft.keep_backup,
                    "Keep a .surroundtag-backup copy before tagging",
                );
            });
            ui.checkbox(
                &mut app.settings_draft.scan_recursive,
                "Scan dropped folders recursively",
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        app.job.is_none(),
                        egui::Button::new(
                            RichText::new("Save Settings")
                                .size(12.5)
                                .strong()
                                .color(theme::CYAN_INK),
                        )
                        .fill(theme::CYAN)
                        .stroke(egui::Stroke::new(1.0, theme::CYAN_BRIGHT))
                        .corner_radius(egui::CornerRadius::same(7)),
                    )
                    .on_disabled_hover_text("Wait for the current apply batch to finish.")
                    .clicked()
                {
                    close = true;
                    app.save_settings(ctx);
                }
                if theme::ghost_button(ui, "Cancel").clicked() {
                    close = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!(
                            "Audio: {}",
                            if app.tools.can_apply() {
                                "mkvpropedit ready"
                            } else {
                                "mkvpropedit missing"
                            }
                        ))
                        .size(9.5)
                        .color(if app.tools.can_apply() {
                            theme::SUCCESS
                        } else {
                            theme::ERROR
                        }),
                    );
                });
            });
        });
    app.show_settings = open && !close;
}

fn path_field(ui: &mut egui::Ui, label: &str, value: &mut Option<PathBuf>) {
    ui.label(RichText::new(label).size(10.5).color(theme::MUTED));
    let mut text = value
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_default();
    let response = ui.add(
        egui::TextEdit::singleline(&mut text)
            .desired_width(480.0)
            .hint_text("auto-detect"),
    );
    if response.changed() {
        *value = if text.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(text.trim()))
        };
    }
}

fn assign_detected(target: &mut Option<PathBuf>, tool: Option<crate::mkv::ToolCommand>) {
    if let Some(tool) = tool {
        if tool.prefix.is_empty() {
            *target = Some(tool.program);
        } else {
            *target = None;
        }
    }
}

pub fn about_window(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut open = app.show_about;
    let mut close = false;
    egui::Window::new("About SurroundTag")
        .open(&mut open)
        .default_width(520.0)
        .resizable(false)
        .frame(theme::card())
        .show(ctx, |ui| {
            ui.label(
                RichText::new(format!("SurroundTag v{}", crate::APP_VERSION))
                    .size(18.0)
                    .strong()
                    .color(theme::TEXT),
            );
            ui.label(
                RichText::new("Tag and embed cover art in 5.1 Matroska audio without re-encoding.")
                    .size(11.0)
                    .color(theme::MUTED),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "SurroundTag reads stereo masters with lofty (pure Rust), pairs them with finished .mka files, and writes Matroska tags and attachments with MKVToolNix mkvpropedit.",
                )
                .size(10.5)
                .color(theme::TEXT),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("MIT License · Copyright (c) 2026 Prisma Chill")
                    .size(10.5)
                    .color(theme::CYAN),
            );
            ui.label(
                RichText::new(
                    "MKVToolNix is an optional external GPL-2.0 tool and is not bundled with SurroundTag.",
                )
                .size(9.5)
                .color(theme::MUTED),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if theme::ghost_button(ui, "Open cache folder").clicked() {
                    let path = crate::config::cache_dir();
                    let _ = std::fs::create_dir_all(&path);
                    let _ = open::that(path);
                }
                if theme::primary_button(ui, "Close").clicked() {
                    close = true;
                }
            });
        });
    app.show_about = open && !close;
}

pub fn help_window(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut open = app.show_help;
    let mut close = false;
    egui::Window::new("SurroundTag Help")
        .open(&mut open)
        .default_width(620.0)
        .resizable(true)
        .frame(theme::card())
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Workflow")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            for line in [
                "1. Load stereo masters (FLAC, MP3, M4A, WAV, OGG) into the left pane.",
                "2. Load finished 5.1 .mka files into the right pane.",
                "3. Review the automatic pairings. Click a master, then a target, to pair manually.",
                "4. Edit album and track tags, and override the cover art if needed.",
                "5. Apply. SurroundTag writes Matroska tags, embeds the cover, and sets track flags.",
                "6. If rename is enabled, the file is renamed with collision checks and undo safety.",
            ] {
                ui.label(RichText::new(line).size(10.5).color(theme::TEXT));
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new("Matching")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            ui.label(
                RichText::new(
                    "Pairing combines track numbers, normalized/fuzzy filename similarity, tag titles, and audio duration. High/Medium pairs are auto-assigned; Low pairs are suggestions.",
                )
                .size(10.0)
                .color(theme::MUTED),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Keyboard")
                    .size(12.5)
                    .strong()
                    .color(theme::CYAN),
            );
            ui.label(
                RichText::new(
                    "Ctrl+O load masters · Ctrl+Shift+O load targets · Ctrl+Enter apply · Ctrl+Z undo last rename",
                )
                .monospace()
                .size(9.5)
                .color(theme::TEXT),
            );
            ui.add_space(8.0);
            if theme::primary_button(ui, "Close").clicked() {
                close = true;
            }
        });
    app.show_help = open && !close;
}
