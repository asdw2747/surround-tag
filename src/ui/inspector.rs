//! Bottom inspector: album/track edits, cover preview, target summary, and
//! the live tag + filename preview.

use egui::{Align, Layout, RichText, Stroke, Ui, Vec2};

use crate::app::SurroundTagApp;
use crate::audio;
use crate::matching::format_duration;
use crate::model::{CoverArt, SourceEntry, TrackDraft};
use crate::template::{self, TemplateContext};
use crate::theme;

pub fn inspector(app: &mut SurroundTagApp, root: &mut egui::Ui) {
    let ctx = root.ctx().clone();
    egui::Panel::bottom("surroundtag_inspector")
        .resizable(true)
        .default_size(340.0)
        .min_size(250.0)
        .max_size(480.0)
        .frame(
            egui::Frame::new()
                .fill(theme::CARD)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .stroke(Stroke::new(1.0, theme::BORDER)),
        )
        .show(root, |ui| {
            inspector_header(app, ui);
            ui.add_space(6.0);
            ui.separator();
            ui.add_space(6.0);

            let selected_source_id = app
                .selected_source
                .or_else(|| app.pairings.first().map(|pair| pair.source_id));
            let source = selected_source_id.and_then(|id| app.source(id)).cloned();
            let target = app
                .selected_target
                .or_else(|| selected_source_id.and_then(|id| app.paired_target(id)))
                .and_then(|id| app.target(id))
                .cloned();

            egui::ScrollArea::vertical()
                .id_salt("surroundtag_inspector_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let available = ui.available_width();
                    if available >= 1180.0 {
                        let cover_width = 180.0;
                        let target_width = 310.0;
                        let fields_width =
                            (available - cover_width - target_width - 28.0).max(340.0);
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(cover_width, ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    cover_column(app, &ctx, ui, source.as_ref(), selected_source_id)
                                },
                            );
                            ui.add_space(12.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(fields_width, ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    edit_column(app, ui, source.as_ref(), selected_source_id);
                                },
                            );
                            ui.add_space(8.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(target_width, ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| target_column(app, ui, source.as_ref(), target.as_ref()),
                            );
                        });
                    } else {
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(
                                Vec2::new(170.0, ui.available_height()),
                                Layout::top_down(Align::Min),
                                |ui| {
                                    cover_column(app, &ctx, ui, source.as_ref(), selected_source_id)
                                },
                            );
                            ui.add_space(12.0);
                            ui.allocate_ui_with_layout(
                                Vec2::new(
                                    (ui.available_width() - 8.0).max(300.0),
                                    ui.available_height(),
                                ),
                                Layout::top_down(Align::Min),
                                |ui| edit_column(app, ui, source.as_ref(), selected_source_id),
                            );
                        });
                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(6.0);
                        target_column(app, ui, source.as_ref(), target.as_ref());
                    }
                });
        });
}

fn inspector_header(app: &mut SurroundTagApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new("TAG & COVER PREVIEW")
                .size(12.0)
                .strong()
                .color(theme::MUTED),
        );
        if let Some(source_id) = app.selected_source {
            if let Some(source) = app.source(source_id) {
                ui.add(
                    egui::Label::new(
                        RichText::new(source.display_title())
                            .size(10.5)
                            .color(theme::CYAN),
                    )
                    .truncate(),
                );
            }
        }
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let enabled = app.job.is_none() && !app.pairings.is_empty() && app.tools.can_apply();
            let pair_count = app.pairings.len();
            let label = if pair_count == 1 {
                "Apply 1 Pair".to_owned()
            } else {
                format!("Apply {pair_count} Pairs")
            };
            let response = ui.add_enabled(
                enabled,
                egui::Button::new(
                    RichText::new(label)
                        .size(12.5)
                        .strong()
                        .color(theme::CYAN_INK),
                )
                .fill(theme::CYAN)
                .stroke(Stroke::new(1.0, theme::CYAN_BRIGHT))
                .corner_radius(egui::CornerRadius::same(7)),
            );
            if response.clicked() {
                app.apply_now();
            }
            if app.job.is_some() {
                ui.add(
                    egui::ProgressBar::new(
                        app.job.as_ref().map(|job| job.fraction()).unwrap_or(0.0),
                    )
                    .desired_width(140.0)
                    .text("Applying…"),
                );
            }
        });
    });

    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        let changed = ui
            .checkbox(&mut app.settings.rename_enabled, "Rename after apply")
            .changed();
        if changed {
            let _ = crate::config::save(&app.settings);
            app.refresh_rename_notes();
        }
        if app.settings.rename_enabled {
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut app.settings.rename_template)
                        .desired_width(220.0)
                        .font(egui::TextStyle::Monospace),
                )
                .changed();
            if changed {
                app.refresh_rename_notes();
            }
        }
        let kodi_changed = ui
            .checkbox(
                &mut app.settings.kodi_folder_art,
                "Also save folder.jpg in target folder (for Kodi)",
            )
            .changed();
        if kodi_changed {
            let _ = crate::config::save(&app.settings);
        }
    });
}

fn cover_column(
    app: &mut SurroundTagApp,
    ctx: &egui::Context,
    ui: &mut Ui,
    source: Option<&SourceEntry>,
    source_id: Option<u64>,
) {
    let cover: Option<CoverArt> = source_id
        .and_then(|id| app.cover_overrides.get(&id).cloned())
        .or_else(|| source.and_then(|source| source.cover.clone()));
    let texture = cover.as_ref().and_then(|cover| app.texture_for(ctx, cover));

    ui.label(
        RichText::new("FRONT COVER")
            .size(10.0)
            .strong()
            .color(theme::MUTED),
    );
    egui::Frame::new()
        .fill(theme::CANVAS_BG)
        .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(6))
        .show(ui, |ui| {
            if let Some(texture) = texture {
                ui.add(
                    egui::Image::new(egui::load::SizedTexture::new(
                        texture.id(),
                        Vec2::splat(160.0),
                    ))
                    .corner_radius(egui::CornerRadius::same(6)),
                );
            } else {
                ui.allocate_ui(Vec2::splat(160.0), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("No cover art").size(11.0).color(theme::DIM));
                    });
                });
            }
        });

    if let Some(cover) = &cover {
        ui.label(
            RichText::new(cover.source.label())
                .size(9.5)
                .color(theme::MUTED),
        );
        theme::pill(
            ui,
            format!(
                "{} · {:.1} KB",
                cover.filename(),
                cover.bytes.len() as f64 / 1024.0
            ),
            theme::CYAN,
        );
    } else {
        theme::hint(ui, "Embedded front cover or folder artwork (cover.jpg / folder.jpg / front.png) will be attached.");
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let enabled = source_id.is_some();
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(RichText::new("Choose…").size(10.5)),
            )
            .clicked()
        {
            if let Some(id) = source_id {
                choose_cover(app, id);
            }
        }
        if app
            .cover_overrides
            .contains_key(&source_id.unwrap_or_default())
            && theme::ghost_button(ui, "Clear").clicked()
        {
            if let Some(id) = source_id {
                app.cover_overrides.remove(&id);
                app.set_notice("Cover override cleared.", theme::MUTED);
            }
        }
    });
}

fn choose_cover(app: &mut SurroundTagApp, source_id: u64) {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Choose front cover art")
        .add_filter("Images", &["jpg", "jpeg", "png", "webp"]);
    if let Some(source) = app.source(source_id) {
        if let Some(parent) = source.path.parent() {
            dialog = dialog.set_directory(parent);
        }
    }
    if let Some(path) = dialog.pick_file() {
        match audio::load_cover_file(&path) {
            Ok(mut cover) => {
                cover.source = crate::model::CoverSource::Override(path);
                app.cover_overrides.insert(source_id, cover);
                app.set_notice("Cover override selected.", theme::SUCCESS);
            }
            Err(error) => app.set_notice(error, theme::ERROR),
        }
    }
}

fn edit_column(
    app: &mut SurroundTagApp,
    ui: &mut Ui,
    source: Option<&SourceEntry>,
    source_id: Option<u64>,
) {
    ui.label(
        RichText::new("ALBUM")
            .size(10.0)
            .strong()
            .color(theme::MUTED),
    );
    let mut album_changed = false;
    egui::Grid::new("album_fields")
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            album_changed |= text_field(ui, "Album", &mut app.album.title, 300.0);
            ui.end_row();
            album_changed |= text_field(ui, "Album Artist", &mut app.album.artist, 300.0);
            ui.end_row();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Year").size(10.5).color(theme::MUTED));
                album_changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut app.album.year)
                            .desired_width(56.0)
                            .hint_text("2026"),
                    )
                    .changed();
                ui.label(RichText::new("Total").size(10.5).color(theme::MUTED));
                album_changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut app.album.total_tracks)
                            .desired_width(44.0)
                            .hint_text("10"),
                    )
                    .changed();
            });
            ui.end_row();
            album_changed |= text_field(ui, "Genre", &mut app.album.genre, 300.0);
            ui.end_row();
        });
    if album_changed {
        app.album_dirty = true;
        app.refresh_pairing_scores();
    }

    ui.add_space(6.0);
    ui.label(
        RichText::new("TRACK")
            .size(10.0)
            .strong()
            .color(theme::MUTED),
    );
    if let (Some(source), Some(source_id)) = (source, source_id) {
        app.track_drafts
            .entry(source_id)
            .or_insert_with(|| TrackDraft::from_source(source));
        let mut changed = false;
        if let Some(draft) = app.track_drafts.get_mut(&source_id) {
            egui::Grid::new("track_fields")
                .num_columns(2)
                .spacing([8.0, 4.0])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Track #").size(10.5).color(theme::MUTED));
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut draft.track_number)
                                    .desired_width(46.0),
                            )
                            .changed();
                        ui.label(RichText::new("Disc #").size(10.5).color(theme::MUTED));
                        changed |= ui
                            .add(
                                egui::TextEdit::singleline(&mut draft.disc_number)
                                    .desired_width(46.0),
                            )
                            .changed();
                    });
                    ui.end_row();
                    changed |= text_field(ui, "Title", &mut draft.title, 300.0);
                    ui.end_row();
                    changed |= text_field(ui, "Artist", &mut draft.artist, 300.0);
                    ui.end_row();
                });
        }
        if changed {
            app.refresh_rename_notes();
        }
        ui.label(
            RichText::new(format!(
                "Source: {}",
                source
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
            ))
            .monospace()
            .size(9.5)
            .color(theme::DIM),
        );
    } else {
        theme::hint(ui, "Select a stereo master to edit its track tags.");
    }
}

fn target_column(
    app: &SurroundTagApp,
    ui: &mut Ui,
    source: Option<&SourceEntry>,
    target: Option<&crate::model::TargetEntry>,
) {
    ui.label(
        RichText::new("TARGET & PREVIEW")
            .size(10.0)
            .strong()
            .color(theme::MUTED),
    );
    if let Some(target) = target {
        egui::Frame::new()
            .fill(theme::CANVAS_BG)
            .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            target
                                .path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("target"),
                        )
                        .size(11.0)
                        .strong()
                        .color(theme::TEXT),
                    )
                    .truncate(),
                );
                ui.label(
                    RichText::new(target.audio_summary())
                        .size(9.5)
                        .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    theme::pill(ui, format_duration(target.duration_secs), theme::TEXT);
                    if let Some(info) = &target.info {
                        theme::pill(
                            ui,
                            if info.tracks.len() == 1 {
                                "1 track".to_owned()
                            } else {
                                format!("{} tracks", info.tracks.len())
                            },
                            theme::MUTED,
                        );
                        theme::pill(
                            ui,
                            if info.attachments.len() == 1 {
                                "1 attachment".to_owned()
                            } else {
                                format!("{} attachments", info.attachments.len())
                            },
                            if info.attachments.is_empty() {
                                theme::MUTED
                            } else {
                                theme::AMBER
                            },
                        );
                    }
                });
            });
    } else {
        theme::hint(
            ui,
            "Select or pair a .mka target to preview its stream flags.",
        );
    }

    ui.add_space(6.0);
    ui.label(
        RichText::new("TAG PREVIEW")
            .size(9.5)
            .strong()
            .color(theme::MUTED),
    );
    let album_title = if app.album.title.trim().is_empty() {
        source
            .map(|source| source.album.as_str())
            .filter(|album| !album.trim().is_empty())
            .unwrap_or("Unknown Album")
    } else {
        app.album.title.trim()
    };
    let track_title = source
        .and_then(|source| app.track_drafts.get(&source.id))
        .map(|draft| draft.title.trim())
        .filter(|title| !title.is_empty())
        .or_else(|| source.map(|source| source.display_title()))
        .unwrap_or("Untitled");
    let track_number = source
        .and_then(|source| app.track_drafts.get(&source.id))
        .and_then(|draft| draft.track_number_value())
        .or_else(|| source.and_then(|source| source.track_number));

    egui::Frame::new()
        .fill(theme::CANVAS_BG)
        .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
        .corner_radius(egui::CornerRadius::same(7))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!("TargetTypeValue 50 · {album_title}"))
                    .size(9.5)
                    .color(theme::CYAN),
            );
            ui.label(
                RichText::new(format!(
                    "TargetTypeValue 30 · {} · {}",
                    track_number
                        .map(|number| format!("#{number:02}"))
                        .unwrap_or_else(|| "#--".to_owned()),
                    track_title
                ))
                .size(9.5)
                .color(theme::TEXT),
            );
            let cover = source.and_then(|source| {
                app.cover_overrides
                    .get(&source.id)
                    .or(source.cover.as_ref())
            });
            ui.label(
                RichText::new(format!(
                    "Cover: {}",
                    cover
                        .map(|cover| cover.filename())
                        .unwrap_or_else(|| "none".to_owned())
                ))
                .size(9.5)
                .color(if cover.is_some() {
                    theme::AMBER
                } else {
                    theme::DIM
                }),
            );
        });

    ui.add_space(6.0);
    ui.label(
        RichText::new("FILENAME PREVIEW")
            .size(9.5)
            .strong()
            .color(theme::MUTED),
    );
    if !app.settings.rename_enabled {
        theme::hint(ui, "Rename is off. Enable it above or in Settings.");
    } else if let (Some(source), Some(target)) = (source, target) {
        let draft = app.track_drafts.get(&source.id);
        let context = TemplateContext {
            track: draft
                .and_then(TrackDraft::track_number_value)
                .or(source.track_number),
            disc: draft
                .and_then(TrackDraft::disc_number_value)
                .or(source.disc_number),
            title: track_title.to_owned(),
            artist: source.artist.clone(),
            album: album_title.to_owned(),
            album_artist: app.album.artist.clone(),
            year: app.album.year.clone(),
            ext: "mka".to_owned(),
        };
        let parent = target
            .path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default();
        match template::target_path(&parent, &app.settings.rename_template, &context) {
            Ok(path) => {
                let collision = app
                    .rename_notes
                    .iter()
                    .any(|note| note.contains("Collision"));
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or_default(),
                        )
                        .monospace()
                        .size(9.5)
                        .color(if collision {
                            theme::AMBER
                        } else {
                            theme::SUCCESS
                        }),
                    )
                    .truncate(),
                );
            }
            Err(error) => {
                ui.label(RichText::new(error).size(9.5).color(theme::ERROR));
            }
        }
        for note in app.rename_notes.iter().take(2) {
            ui.add(egui::Label::new(RichText::new(note).size(9.0).color(theme::AMBER)).truncate());
        }
    } else {
        theme::hint(
            ui,
            "Pair a source and target to preview the output filename.",
        );
    }
}

fn text_field(ui: &mut Ui, label: &str, value: &mut String, width: f32) -> bool {
    ui.label(RichText::new(label).size(10.5).color(theme::MUTED));
    ui.add(egui::TextEdit::singleline(value).desired_width(width))
        .changed()
}
