//! Two-pane source/target lists and the pairing rail.

use std::collections::HashMap;

use egui::{Align, Layout, RichText, Sense, Stroke, TextureHandle, Ui, Vec2};

use crate::app::SurroundTagApp;
use crate::matching::{confidence_label, format_duration};
use crate::model::{CoverArt, Pairing, ProbeStatus, SourceEntry, TargetEntry};
use crate::theme;

pub fn open_source_dialog(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Load stereo masters")
        .add_filter("Stereo masters", &["flac", "mp3", "m4a", "wav", "ogg"]);
    if let Some(directory) = &app.settings.last_source_dir {
        dialog = dialog.set_directory(directory);
    }
    if let Some(files) = dialog.pick_files() {
        if let Some(parent) = files.first().and_then(|path| path.parent()) {
            app.settings.last_source_dir = Some(parent.to_path_buf());
        }
        app.add_paths(files, ctx);
    }
}

pub fn open_source_folder(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new().set_title("Load a folder of stereo masters");
    if let Some(directory) = &app.settings.last_source_dir {
        dialog = dialog.set_directory(directory);
    }
    if let Some(folder) = dialog.pick_folder() {
        app.settings.last_source_dir = Some(folder.clone());
        app.add_paths(vec![folder], ctx);
    }
}

pub fn open_target_dialog(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Load 5.1 Matroska Audio targets")
        .add_filter("Matroska Audio", &["mka"]);
    if let Some(directory) = &app.settings.last_target_dir {
        dialog = dialog.set_directory(directory);
    }
    if let Some(files) = dialog.pick_files() {
        if let Some(parent) = files.first().and_then(|path| path.parent()) {
            app.settings.last_target_dir = Some(parent.to_path_buf());
        }
        app.add_paths(files, ctx);
    }
}

pub fn open_target_folder(app: &mut SurroundTagApp, ctx: &egui::Context) {
    let mut dialog = rfd::FileDialog::new().set_title("Load a folder of .mka targets");
    if let Some(directory) = &app.settings.last_target_dir {
        dialog = dialog.set_directory(directory);
    }
    if let Some(folder) = dialog.pick_folder() {
        app.settings.last_target_dir = Some(folder.clone());
        app.add_paths(vec![folder], ctx);
    }
}

pub fn central_panes(app: &mut SurroundTagApp, root: &mut egui::Ui) {
    let ctx = root.ctx().clone();
    egui::CentralPanel::default()
        .frame(egui::Frame::new().fill(theme::BG).inner_margin(10))
        .show(root, |ui| {
            let available = ui.available_width();
            let rail = 96.0;
            let gap = 8.0;
            let pane_width = ((available - rail - gap * 2.0) / 2.0).max(280.0);
            let height = ui.available_height();

            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(pane_width, height),
                    Layout::top_down(Align::Min),
                    |ui| source_pane(app, &ctx, ui),
                );
                ui.add_space(gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(rail, height),
                    Layout::top_down(Align::Center),
                    |ui| pairing_rail(app, ui),
                );
                ui.add_space(gap);
                ui.allocate_ui_with_layout(
                    Vec2::new(pane_width, height),
                    Layout::top_down(Align::Min),
                    |ui| target_pane(app, &ctx, ui),
                );
            });
        });
}

fn pane_header(ui: &mut Ui, title: &str, subtitle: &str, count: usize, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).size(13.0).strong().color(color));
        theme::pill(ui, count.to_string(), color);
        if !subtitle.is_empty() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(subtitle).size(10.0).color(theme::MUTED));
            });
        }
    });
}

fn source_pane(app: &mut SurroundTagApp, ctx: &egui::Context, ui: &mut Ui) {
    pane_header(
        ui,
        "STEREO MASTERS",
        "FLAC · MP3 · M4A · WAV · OGG",
        app.sources.len(),
        theme::CYAN,
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.source_search)
                .hint_text("Filter masters…")
                .desired_width(ui.available_width() - 90.0),
        );
        if theme::ghost_button(ui, "Load").clicked() {
            open_source_dialog(app, ctx);
        }
    });
    ui.add_space(3.0);

    let covers: Vec<(u64, CoverArt)> = app
        .sources
        .iter()
        .filter_map(|source| {
            app.cover_overrides
                .get(&source.id)
                .cloned()
                .or_else(|| source.cover.clone())
                .map(|cover| (source.id, cover))
        })
        .collect();
    let mut textures: HashMap<u64, TextureHandle> = HashMap::new();
    for (id, cover) in covers {
        if let Some(texture) = app.texture_for(ctx, &cover) {
            textures.insert(id, texture);
        }
    }

    let query = app.source_search.to_ascii_lowercase();
    let mut indices: Vec<usize> = (0..app.sources.len()).collect();
    indices.sort_by(|left, right| {
        let left = &app.sources[*left];
        let right = &app.sources[*right];
        left.track_number
            .cmp(&right.track_number)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut clicked_source = None;
    let mut unpair = None;
    let mut remove_source = None;

    egui::ScrollArea::vertical()
        .id_salt("surroundtag_source_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.sources.is_empty() {
                empty_pane(
                    ui,
                    "Drop stereo masters here",
                    "FLAC, MP3, M4A, WAV, or OGG. Embedded and folder cover art are detected automatically.",
                    theme::CYAN,
                );
            }
            for index in indices {
                let source = &app.sources[index];
                if !query.is_empty()
                    && !source
                        .path
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&query)
                {
                    continue;
                }
                let pair = app.pairing_for_source(source.id).cloned();
                ui.push_id(("source", source.id), |ui| {
                    let result = source_row(
                        ui,
                        source,
                        pair.as_ref(),
                        textures.get(&source.id),
                        app.selected_source == Some(source.id),
                        app.cover_overrides.contains_key(&source.id),
                    );
                    if result.clicked {
                        clicked_source = Some(source.id);
                    }
                    if result.unpair {
                        unpair = Some(source.id);
                    }
                    if result.remove {
                        remove_source = Some(source.id);
                    }
                });
                ui.add_space(3.0);
            }
        });

    if let Some(id) = remove_source {
        app.remove_source(id);
    } else if let Some(id) = unpair {
        app.unpair_source(id);
    } else if let Some(id) = clicked_source {
        app.select_source(id);
    }
}

fn target_pane(app: &mut SurroundTagApp, ctx: &egui::Context, ui: &mut Ui) {
    pane_header(ui, "5.1 TARGETS", "MKA", app.targets.len(), theme::AMBER);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.target_search)
                .hint_text("Filter .mka targets…")
                .desired_width(ui.available_width() - 90.0),
        );
        if theme::ghost_button(ui, "Load").clicked() {
            open_target_dialog(app, ctx);
        }
    });
    ui.add_space(3.0);

    let query = app.target_search.to_ascii_lowercase();
    let mut indices: Vec<usize> = (0..app.targets.len()).collect();
    indices.sort_by(|left, right| {
        let left = &app.targets[*left];
        let right = &app.targets[*right];
        left.track_number
            .cmp(&right.track_number)
            .then_with(|| left.path.cmp(&right.path))
    });

    let mut clicked_target = None;
    let mut remove_target = None;
    let combined: HashMap<u64, bool> = app
        .targets
        .iter()
        .map(|target| {
            (
                target.id,
                app.pairing_for_target(target.id).is_none()
                    && crate::matching::combined_track_hint(&app.sources, target).is_some(),
            )
        })
        .collect();

    egui::ScrollArea::vertical()
        .id_salt("surroundtag_target_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if app.targets.is_empty() {
                empty_pane(
                    ui,
                    "Drop finished .mka files here",
                    "SurroundTag writes Matroska tags, cover art, and default-track flags with mkvpropedit.",
                    theme::AMBER,
                );
            }
            for index in indices {
                let target = &app.targets[index];
                if !query.is_empty()
                    && !target
                        .path
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .contains(&query)
                {
                    continue;
                }
                let pair = app.pairing_for_target(target.id).cloned();
                ui.push_id(("target", target.id), |ui| {
                    let result = target_row(
                        ui,
                        target,
                        pair.as_ref(),
                        app.selected_target == Some(target.id),
                        &app.settings.track_name,
                        &app.settings.language,
                        combined.get(&target.id).copied().unwrap_or(false),
                    );
                    if result.clicked {
                        clicked_target = Some(target.id);
                    }
                    if result.remove {
                        remove_target = Some(target.id);
                    }
                });
                ui.add_space(3.0);
            }
        });

    if let Some(id) = remove_target {
        app.remove_target(id);
    } else if let Some(id) = clicked_target {
        app.select_target(id);
    }
}

#[derive(Default)]
struct RowResult {
    clicked: bool,
    unpair: bool,
    remove: bool,
}

fn source_row(
    ui: &mut Ui,
    source: &SourceEntry,
    pair: Option<&Pairing>,
    texture: Option<&TextureHandle>,
    selected: bool,
    override_active: bool,
) -> RowResult {
    let mut result = RowResult::default();
    let frame = egui::Frame::new()
        .fill(if selected {
            theme::CARD_HIGH
        } else {
            theme::CARD
        })
        .stroke(Stroke::new(
            1.0,
            if selected { theme::CYAN } else { theme::BORDER },
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8));

    let response = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                cover_thumb(ui, texture, 44.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(track) = source.track_number {
                            theme::pill(ui, format!("{track:02}"), theme::CYAN);
                        }
                        ui.label(
                            RichText::new(source.display_title())
                                .size(12.5)
                                .strong()
                                .color(theme::TEXT),
                        );
                        if let Some(pair) = pair {
                            theme::pill(
                                ui,
                                confidence_label(pair.confidence),
                                confidence_color(pair.confidence),
                            );
                        }
                    });
                    let detail = if source.artist.trim().is_empty() {
                        source
                            .path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_owned()
                    } else {
                        format!("{} · {}", source.artist, source.album)
                    };
                    ui.label(RichText::new(detail).size(10.5).color(theme::MUTED));
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format_duration(source.duration_secs))
                                .monospace()
                                .size(10.0)
                                .color(theme::TEXT),
                        );
                        ui.label(
                            RichText::new(
                                source
                                    .path
                                    .extension()
                                    .and_then(|ext| ext.to_str())
                                    .unwrap_or_default()
                                    .to_ascii_uppercase(),
                            )
                            .monospace()
                            .size(9.5)
                            .color(theme::DIM),
                        );
                        if override_active {
                            ui.label(
                                RichText::new("Cover override")
                                    .size(9.5)
                                    .color(theme::AMBER),
                            );
                        } else if let Some(cover) = &source.cover {
                            ui.label(
                                RichText::new(cover.source.label())
                                    .size(9.5)
                                    .color(theme::CYAN),
                            );
                        }
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(12.0).color(theme::MUTED))
                                .fill(theme::BG)
                                .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
                                .corner_radius(egui::CornerRadius::same(5))
                                .min_size(Vec2::new(22.0, 22.0)),
                        )
                        .on_hover_text("Remove from workspace")
                        .clicked()
                    {
                        result.remove = true;
                    }
                    if pair.is_some()
                        && ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Unpair").size(9.5).color(theme::AMBER),
                                )
                                .fill(theme::BG)
                                .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
                                .corner_radius(egui::CornerRadius::same(5)),
                            )
                            .clicked()
                    {
                        result.unpair = true;
                    }
                    status_pill(ui, source.status);
                });
            });
        })
        .response
        .interact(Sense::click());

    if response.clicked() && !result.remove && !result.unpair {
        result.clicked = true;
    }
    result
}

fn target_row(
    ui: &mut Ui,
    target: &TargetEntry,
    pair: Option<&Pairing>,
    selected: bool,
    expected_name: &str,
    expected_language: &str,
    combined_candidate: bool,
) -> RowResult {
    let mut result = RowResult::default();
    let frame = egui::Frame::new()
        .fill(if selected {
            theme::CARD_HIGH
        } else {
            theme::CARD
        })
        .stroke(Stroke::new(
            1.0,
            if selected {
                theme::AMBER
            } else {
                theme::BORDER
            },
        ))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(8));

    let response = frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let info = target.info.as_ref();
                let audio = info.and_then(|info| info.first_audio());
                let channels = audio.map(|track| track.channels).unwrap_or_default();
                let codec = audio.map(|track| track.codec.as_str()).unwrap_or("--");
                let cover_count = info
                    .map(|info| {
                        info.attachments
                            .iter()
                            .filter(|attachment| attachment.looks_like_cover())
                            .count()
                    })
                    .unwrap_or_default();

                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        if let Some(track) = target.track_number {
                            theme::pill(ui, format!("{track:02}"), theme::AMBER);
                        }
                        ui.label(
                            RichText::new(target.display_title())
                                .size(12.5)
                                .strong()
                                .color(theme::TEXT),
                        );
                        if let Some(pair) = pair {
                            theme::pill(
                                ui,
                                confidence_label(pair.confidence),
                                confidence_color(pair.confidence),
                            );
                        } else if combined_candidate {
                            theme::pill(ui, "COMBINED?", theme::AMBER);
                        }
                    });
                    ui.label(
                        RichText::new(target.audio_summary())
                            .size(10.0)
                            .color(theme::MUTED),
                    );
                    ui.horizontal(|ui| {
                        theme::pill(ui, format!("{channels} ch"), theme::AMBER);
                        theme::pill(ui, codec, theme::CYAN);
                        ui.label(
                            RichText::new(format_duration(target.duration_secs))
                                .monospace()
                                .size(10.0)
                                .color(theme::TEXT),
                        );
                        if cover_count > 0 {
                            theme::pill(ui, format!("{cover_count} cover"), theme::SUCCESS);
                        }
                    });
                });
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(12.0).color(theme::MUTED))
                                .fill(theme::BG)
                                .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
                                .corner_radius(egui::CornerRadius::same(5))
                                .min_size(Vec2::new(22.0, 22.0)),
                        )
                        .on_hover_text("Remove from workspace")
                        .clicked()
                    {
                        result.remove = true;
                    }
                    if let Some(info) = info {
                        if info.first_audio().is_some_and(|track| {
                            track.default
                                && track.language.eq_ignore_ascii_case(expected_language)
                                && track.name.as_deref() == Some(expected_name)
                        }) {
                            theme::pill(ui, "FLAGS OK", theme::SUCCESS);
                        } else {
                            theme::pill(ui, "NEEDS FLAGS", theme::AMBER);
                        }
                    }
                    status_pill(ui, target.status);
                });
            });
        })
        .response
        .interact(Sense::click());

    if response.clicked() && !result.remove {
        result.clicked = true;
    }
    result
}

fn cover_thumb(ui: &mut Ui, texture: Option<&TextureHandle>, size: f32) {
    if let Some(texture) = texture {
        ui.add(
            egui::Image::new(egui::load::SizedTexture::new(
                texture.id(),
                Vec2::splat(size),
            ))
            .corner_radius(egui::CornerRadius::same(6)),
        );
    } else {
        egui::Frame::new()
            .fill(theme::CANVAS_BG)
            .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
            .corner_radius(egui::CornerRadius::same(6))
            .show(ui, |ui| {
                ui.allocate_ui(Vec2::splat(size), |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("♪").size(20.0).color(theme::DIM));
                    });
                });
            });
    }
}

fn status_pill(ui: &mut Ui, status: ProbeStatus) {
    let color = match status {
        ProbeStatus::Ready => theme::SUCCESS,
        ProbeStatus::Probing | ProbeStatus::Pending => theme::MUTED,
        ProbeStatus::Warning => theme::AMBER,
        ProbeStatus::Error => theme::ERROR,
    };
    theme::pill(ui, status.label(), color);
}

fn confidence_color(score: f32) -> egui::Color32 {
    if score >= 0.85 {
        theme::CYAN
    } else if score >= 0.65 {
        theme::AMBER
    } else {
        theme::ERROR
    }
}

fn pairing_rail(app: &mut SurroundTagApp, ui: &mut Ui) {
    ui.add_space(28.0);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("PAIRS")
                .size(11.0)
                .strong()
                .color(theme::MUTED),
        );
        ui.label(
            RichText::new(app.pairings.len().to_string())
                .size(28.0)
                .strong()
                .color(theme::CYAN),
        );
        ui.add_space(6.0);
        let high = app
            .pairings
            .iter()
            .filter(|pair| pair.confidence >= 0.85)
            .count();
        let medium = app
            .pairings
            .iter()
            .filter(|pair| pair.confidence >= 0.65 && pair.confidence < 0.85)
            .count();
        let low = app
            .pairings
            .iter()
            .filter(|pair| pair.confidence < 0.65)
            .count();
        theme::pill(ui, format!("High {high}"), theme::CYAN);
        theme::pill(ui, format!("Med {medium}"), theme::AMBER);
        theme::pill(ui, format!("Low {low}"), theme::ERROR);
        ui.add_space(8.0);
        if theme::ghost_button(ui, "Auto").clicked() {
            app.auto_pair_now();
        }
        if theme::ghost_button(ui, "Clear").clicked() {
            app.clear_pairings();
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new("Click a master\nthen a target\nto pair manually.")
                .size(9.0)
                .color(theme::DIM),
        );
    });
}

fn empty_pane(ui: &mut Ui, title: &str, detail: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(theme::CANVAS_BG)
        .stroke(Stroke::new(1.0, theme::BORDER_SOFT))
        .corner_radius(egui::CornerRadius::same(10))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_min_height(100.0);
            ui.vertical_centered(|ui| {
                ui.add_space(12.0);
                ui.label(RichText::new(title).size(13.0).strong().color(color));
                ui.add(
                    egui::Label::new(RichText::new(detail).size(10.5).color(theme::MUTED)).wrap(),
                );
            });
        });
}
