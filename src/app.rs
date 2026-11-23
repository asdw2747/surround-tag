//! Application controller: loading, probing, pairing, editing, applying,
//! renaming, undo, and the egui frame.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui;
use egui::{Color32, TextureHandle, TextureOptions};

use crate::config::{self, AppConfig, CollisionPolicy};
use crate::matching;
use crate::mkv::{self, apply::ApplyJob, plan::TrackFlags, undo};
use crate::model::{
    AlbumDraft, ApplySummary, CoverArt, CoverMime, FolderArtWrite, Pairing, ProbeStatus,
    SourceEntry, TargetEntry, TrackDraft,
};
use crate::scan;
use crate::tags::{AlbumTags, TagPlan, TrackTags};
use crate::template::{self, TemplateContext};
use crate::theme;
use crate::ui;
use crate::worker::{self, Command, Event, ProbeKind, Worker};

#[derive(Clone, Debug)]
pub struct Notice {
    pub text: String,
    pub color: Color32,
    pub born: Instant,
}

impl Notice {
    pub fn new(text: impl Into<String>, color: Color32) -> Self {
        Self {
            text: text.into(),
            color,
            born: Instant::now(),
        }
    }

    pub fn expired(&self) -> bool {
        self.born.elapsed() > Duration::from_secs(10)
    }
}

#[derive(Clone, Debug)]
pub struct JobProgress {
    pub done: usize,
    pub total: usize,
    pub target: PathBuf,
}

impl JobProgress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

pub struct SurroundTagApp {
    pub sources: Vec<SourceEntry>,
    pub targets: Vec<TargetEntry>,
    pub pairings: Vec<Pairing>,
    pub selected_source: Option<u64>,
    pub selected_target: Option<u64>,
    pub album: AlbumDraft,
    pub album_dirty: bool,
    pub track_drafts: HashMap<u64, TrackDraft>,
    pub cover_overrides: HashMap<u64, CoverArt>,
    pub settings: AppConfig,
    pub settings_draft: AppConfig,
    pub tools: mkv::ToolSet,
    pub worker: Worker,
    pub next_id: u64,
    pub job: Option<JobProgress>,
    pub summary: Option<ApplySummary>,
    pub manifest_error: Option<String>,
    pub notice: Option<Notice>,
    pub show_settings: bool,
    pub show_about: bool,
    pub show_help: bool,
    pub show_report: bool,
    pub undo_available: bool,
    pub textures: HashMap<u64, TextureHandle>,
    pub source_search: String,
    pub target_search: String,
    pub rename_notes: Vec<String>,
    pub bootstrapped: bool,
}

impl SurroundTagApp {
    pub fn new(cc: &eframe::CreationContext<'_>, startup_paths: Vec<PathBuf>) -> Self {
        theme::apply(&cc.egui_ctx);
        let (settings, warning) = config::load();
        let tools = mkv::ToolSet::detect(&settings);
        let worker = worker::start(cc.egui_ctx.clone(), tools.clone());
        let mut app = Self {
            sources: Vec::new(),
            targets: Vec::new(),
            pairings: Vec::new(),
            selected_source: None,
            selected_target: None,
            album: AlbumDraft::default(),
            album_dirty: false,
            track_drafts: HashMap::new(),
            cover_overrides: HashMap::new(),
            settings_draft: settings.clone(),
            settings,
            tools,
            worker,
            next_id: 1,
            job: None,
            summary: None,
            manifest_error: None,
            notice: warning.map(|warning| Notice::new(warning, theme::WARNING)),
            show_settings: false,
            show_about: false,
            show_help: false,
            show_report: false,
            undo_available: undo::has_manifest(),
            textures: HashMap::new(),
            source_search: String::new(),
            target_search: String::new(),
            rename_notes: Vec::new(),
            bootstrapped: false,
        };

        if !startup_paths.is_empty() {
            app.add_paths(startup_paths, &cc.egui_ctx);
        }
        app
    }

    pub fn add_paths(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let outcome = scan::expand_paths(&paths, self.settings.scan_recursive);
        let mut source_paths = Vec::new();
        let mut target_paths = Vec::new();

        let existing_sources: HashSet<PathBuf> = self
            .sources
            .iter()
            .map(|source| {
                source
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| source.path.clone())
            })
            .collect();
        let existing_targets: HashSet<PathBuf> = self
            .targets
            .iter()
            .map(|target| {
                target
                    .path
                    .canonicalize()
                    .unwrap_or_else(|_| target.path.clone())
            })
            .collect();

        for path in outcome.sources {
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            if existing_sources.contains(&key) {
                continue;
            }
            let id = self.allocate_id();
            self.sources.push(SourceEntry::new(id, path.clone()));
            source_paths.push((id, path));
        }

        for path in outcome.targets {
            let key = path.canonicalize().unwrap_or_else(|_| path.clone());
            if existing_targets.contains(&key) {
                continue;
            }
            let id = self.allocate_id();
            self.targets.push(TargetEntry::new(id, path.clone()));
            target_paths.push((id, path));
        }

        if let Some(parent) = source_paths.first().and_then(|(_, path)| path.parent()) {
            self.settings.last_source_dir = Some(parent.to_path_buf());
        }
        if let Some(parent) = target_paths.first().and_then(|(_, path)| path.parent()) {
            self.settings.last_target_dir = Some(parent.to_path_buf());
        }
        if !source_paths.is_empty() {
            let _ = self.worker.tx.send(Command::ProbeSources(source_paths));
            ctx.request_repaint();
        }
        if !target_paths.is_empty() {
            let _ = self.worker.tx.send(Command::ProbeTargets(target_paths));
            ctx.request_repaint();
        }
        if !outcome.notes.is_empty() {
            self.set_notice(outcome.notes.join(" · "), theme::WARNING);
        }
    }

    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn set_notice(&mut self, text: impl Into<String>, color: Color32) {
        self.notice = Some(Notice::new(text, color));
    }

    pub fn source(&self, id: u64) -> Option<&SourceEntry> {
        self.sources.iter().find(|source| source.id == id)
    }

    pub fn target(&self, id: u64) -> Option<&TargetEntry> {
        self.targets.iter().find(|target| target.id == id)
    }

    pub fn paired_target(&self, source_id: u64) -> Option<u64> {
        self.pairings
            .iter()
            .find(|pair| pair.source_id == source_id)
            .map(|pair| pair.target_id)
    }

    pub fn paired_source(&self, target_id: u64) -> Option<u64> {
        self.pairings
            .iter()
            .find(|pair| pair.target_id == target_id)
            .map(|pair| pair.source_id)
    }

    pub fn pairing_for_source(&self, source_id: u64) -> Option<&Pairing> {
        self.pairings
            .iter()
            .find(|pair| pair.source_id == source_id)
    }

    pub fn pairing_for_target(&self, target_id: u64) -> Option<&Pairing> {
        self.pairings
            .iter()
            .find(|pair| pair.target_id == target_id)
    }

    pub fn select_source(&mut self, id: u64) {
        if self.selected_target.is_some() {
            self.selected_source = Some(id);
            self.pair_selected();
        } else {
            self.selected_source = Some(id);
        }
    }

    pub fn select_target(&mut self, id: u64) {
        if self.selected_source.is_some() {
            self.selected_target = Some(id);
            self.pair_selected();
        } else {
            self.selected_target = Some(id);
        }
    }

    pub fn pair_selected(&mut self) {
        let (Some(source_id), Some(target_id)) = (self.selected_source, self.selected_target)
        else {
            return;
        };
        let Some(source) = self.source(source_id) else {
            return;
        };
        let Some(target) = self.target(target_id) else {
            return;
        };
        let candidate = matching::score_pair(source, target);
        self.pairings
            .retain(|pair| pair.source_id != source_id && pair.target_id != target_id);
        self.pairings.push(Pairing {
            source_id,
            target_id,
            confidence: candidate.score,
            reasons: candidate.reasons,
            locked: true,
        });
        self.ensure_drafts();
        self.selected_source = None;
        self.selected_target = None;
    }

    pub fn unpair_source(&mut self, source_id: u64) {
        self.pairings.retain(|pair| pair.source_id != source_id);
    }

    pub fn toggle_lock(&mut self, source_id: u64) {
        if let Some(pair) = self
            .pairings
            .iter_mut()
            .find(|pair| pair.source_id == source_id)
        {
            pair.locked = !pair.locked;
        }
    }

    pub fn auto_pair_now(&mut self) {
        self.pairings = matching::auto_pair(&self.sources, &self.targets, &self.pairings);
        self.ensure_drafts();
        let count = self.pairings.len();
        self.refresh_rename_notes();
        if count > 0 {
            self.set_notice(
                format!("Auto-paired {count} track(s). Review before applying."),
                theme::CYAN,
            );
        }
    }

    pub fn clear_pairings(&mut self) {
        self.pairings.clear();
        self.selected_source = None;
        self.selected_target = None;
        self.set_notice("Pairings cleared.", theme::MUTED);
    }

    pub fn ensure_drafts(&mut self) {
        for source in &self.sources {
            self.track_drafts
                .entry(source.id)
                .or_insert_with(|| TrackDraft::from_source(source));
        }
        if !self.album_dirty && self.album.title.trim().is_empty() {
            if let Some(source) = self
                .sources
                .iter()
                .find(|source| source.status == ProbeStatus::Ready)
                .or_else(|| self.sources.first())
            {
                self.album = AlbumDraft::from_source(source);
            }
        }
    }

    pub fn all_probed(&self) -> bool {
        !self.sources.is_empty()
            && !self.targets.is_empty()
            && self
                .sources
                .iter()
                .all(|source| !matches!(source.status, ProbeStatus::Pending | ProbeStatus::Probing))
            && self
                .targets
                .iter()
                .all(|target| !matches!(target.status, ProbeStatus::Pending | ProbeStatus::Probing))
    }

    pub fn selected_track_draft(&self) -> Option<&TrackDraft> {
        self.selected_source
            .and_then(|id| self.track_drafts.get(&id))
    }

    pub fn selected_track_draft_mut(&mut self) -> Option<&mut TrackDraft> {
        self.selected_source
            .and_then(|id| self.track_drafts.get_mut(&id))
    }

    pub fn texture_for(&mut self, ctx: &egui::Context, cover: &CoverArt) -> Option<TextureHandle> {
        let key = cover.cache_key();
        if let Some(texture) = self.textures.get(&key) {
            return Some(texture.clone());
        }
        let image = image::load_from_memory(&cover.bytes).ok()?;
        let image = image.thumbnail(256, 256).to_rgba8();
        let size = [image.width() as usize, image.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
        let texture = ctx.load_texture(
            format!("surroundtag-cover-{key}"),
            color,
            TextureOptions::LINEAR,
        );
        self.textures.insert(key, texture.clone());
        Some(texture)
    }

    pub fn pump_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.worker.rx.try_recv() {
            match event {
                Event::SourceProbed(source) => {
                    if let Some(existing) =
                        self.sources.iter_mut().find(|entry| entry.id == source.id)
                    {
                        *existing = source.clone();
                    } else {
                        self.sources.push(source.clone());
                    }
                    if source.status == ProbeStatus::Ready {
                        self.track_drafts
                            .entry(source.id)
                            .or_insert_with(|| TrackDraft::from_source(&source));
                        if !self.album_dirty && self.album.title.trim().is_empty() {
                            self.album = AlbumDraft::from_source(&source);
                        }
                    }
                    ctx.request_repaint();
                }
                Event::TargetProbed(target) => {
                    if let Some(existing) =
                        self.targets.iter_mut().find(|entry| entry.id == target.id)
                    {
                        *existing = target;
                    } else {
                        self.targets.push(target);
                    }
                    ctx.request_repaint();
                }
                Event::ProbeFinished { kind, errors } => {
                    if let Some(error) = errors.first() {
                        self.set_notice(error.clone(), theme::ERROR);
                    }
                    if self.all_probed() {
                        self.auto_pair_now();
                    } else {
                        self.ensure_drafts();
                    }
                    if kind == ProbeKind::Targets && self.settings.rename_enabled {
                        self.refresh_rename_notes();
                    }
                    ctx.request_repaint();
                }
                Event::ApplyProgress {
                    done,
                    total,
                    target,
                } => {
                    self.job = Some(JobProgress {
                        done,
                        total,
                        target,
                    });
                    ctx.request_repaint();
                }
                Event::ApplyItem(result) => {
                    if let Some(summary) = self.summary.as_mut() {
                        summary.results.push(result);
                        summary.recompute();
                    }
                    ctx.request_repaint();
                }
                Event::ApplyFinished {
                    summary,
                    manifest_error,
                } => {
                    let mut refresh = Vec::new();
                    for result in &summary.results {
                        if result.state == crate::model::ApplyState::Failed {
                            continue;
                        }
                        if let Some(target) = self.targets.iter_mut().find(|target| {
                            target.path == result.target
                                || result
                                    .renamed_to
                                    .as_ref()
                                    .is_some_and(|renamed| &target.path == renamed)
                        }) {
                            if let Some(renamed) = &result.renamed_to {
                                target.path = renamed.clone();
                            }
                            target.status = ProbeStatus::Probing;
                            refresh.push((target.id, target.path.clone()));
                        }
                    }
                    self.summary = Some(summary);
                    self.manifest_error = manifest_error.clone();
                    self.job = None;
                    self.undo_available = undo::has_manifest();
                    self.show_report = true;
                    if !refresh.is_empty() {
                        let _ = self.worker.tx.send(Command::ProbeTargets(refresh));
                    }
                    if let Some(error) = manifest_error {
                        self.set_notice(
                            format!("Rename undo manifest could not be saved: {error}"),
                            theme::WARNING,
                        );
                    } else {
                        self.set_notice("Batch apply finished.", theme::SUCCESS);
                    }
                    ctx.request_repaint();
                }
                Event::UndoFinished(result) => {
                    match result {
                        Ok(report) => {
                            self.undo_available = undo::has_manifest();
                            let text = if report.failed.is_empty() {
                                format!("Undo complete: {} file(s) restored.", report.restored)
                            } else {
                                format!(
                                    "Undo restored {} file(s); {} failed.",
                                    report.restored,
                                    report.failed.len()
                                )
                            };
                            self.set_notice(text, theme::SUCCESS);
                        }
                        Err(error) => self.set_notice(error, theme::ERROR),
                    }
                    ctx.request_repaint();
                }
            }
        }

        if let Some(notice) = &self.notice {
            if notice.expired() {
                self.notice = None;
            }
        }
    }

    pub fn undo_last(&mut self) {
        let _ = self.worker.tx.send(Command::Undo);
    }

    pub fn refresh_pairing_scores(&mut self) {
        for pair in &mut self.pairings {
            let Some(source) = self
                .sources
                .iter()
                .find(|source| source.id == pair.source_id)
            else {
                continue;
            };
            let Some(target) = self
                .targets
                .iter()
                .find(|target| target.id == pair.target_id)
            else {
                continue;
            };
            let candidate = matching::score_pair(source, target);
            pair.confidence = candidate.score;
            pair.reasons = candidate.reasons;
        }
    }

    pub fn build_jobs(&mut self) -> Result<Vec<ApplyJob>, String> {
        self.refresh_pairing_scores();
        if self.pairings.is_empty() {
            return Err("There are no paired tracks to apply.".to_owned());
        }
        if !self.tools.can_apply() {
            return Err(
                "MKVToolNix mkvpropedit was not found. Open Settings to locate it.".to_owned(),
            );
        }
        if self.settings.rename_enabled {
            template::validate(&self.settings.rename_template)?;
        }

        self.ensure_drafts();
        let mut jobs = Vec::new();
        let mut used_targets: HashSet<String> = HashSet::new();
        let mut folder_art_seen: HashMap<String, (u64, PathBuf)> = HashMap::new();
        let mut notes = Vec::new();

        for pairing in self.pairings.clone() {
            let Some(source) = self.source(pairing.source_id).cloned() else {
                continue;
            };
            let Some(target) = self.target(pairing.target_id).cloned() else {
                continue;
            };
            if source.status != ProbeStatus::Ready || target.status != ProbeStatus::Ready {
                notes.push(format!(
                    "Skipped {}: not fully probed.",
                    target.path.display()
                ));
                continue;
            }
            let Some(info) = target.info.clone() else {
                notes.push(format!(
                    "Skipped {}: no Matroska track information.",
                    target.path.display()
                ));
                continue;
            };
            if info.first_audio().is_none() {
                notes.push(format!(
                    "Skipped {}: no audio track was found.",
                    target.path.display()
                ));
                continue;
            }
            if info
                .first_audio()
                .is_some_and(|track| track.channels != 0 && track.channels != 6)
            {
                notes.push(format!(
                    "Warning for {}: first audio track has {} channels, expected 6.",
                    target.path.display(),
                    info.first_audio().map(|track| track.channels).unwrap_or(0)
                ));
            }

            let draft = self
                .track_drafts
                .get(&source.id)
                .cloned()
                .unwrap_or_else(|| TrackDraft::from_source(&source));

            let track_title = if draft.title.trim().is_empty() {
                source.display_title().to_owned()
            } else {
                draft.title.trim().to_owned()
            };
            let track_artist = if draft.artist.trim().is_empty() {
                source.artist.clone()
            } else {
                draft.artist.trim().to_owned()
            };
            let album_title = if !self.album.title.trim().is_empty() {
                self.album.title.trim().to_owned()
            } else if !source.album.trim().is_empty() {
                source.album.trim().to_owned()
            } else {
                "Unknown Album".to_owned()
            };
            let album_artist = if !self.album.artist.trim().is_empty() {
                self.album.artist.trim().to_owned()
            } else if !source.album_artist.trim().is_empty() {
                source.album_artist.trim().to_owned()
            } else {
                source.artist.clone()
            };

            let track_number = draft.track_number_value().or(source.track_number);
            let disc_number = draft.disc_number_value().or(source.disc_number);

            let tag_plan = TagPlan {
                album: AlbumTags {
                    title: album_title.clone(),
                    artist: album_artist.clone(),
                    year: non_empty(self.album.year.clone()),
                    genre: non_empty(self.album.genre.clone()),
                    total_tracks: self.album.total_tracks_value(),
                },
                track: TrackTags {
                    title: track_title.clone(),
                    artist: track_artist.clone(),
                    track_number,
                    disc_number,
                    disc_offset: None,
                },
                track_uid: info
                    .first_audio()
                    .map(|track| track.uid)
                    .filter(|uid| *uid > 0),
            };

            let cover = self
                .cover_overrides
                .get(&source.id)
                .cloned()
                .or_else(|| source.cover.clone());

            let rename_to = if self.settings.rename_enabled {
                let context = TemplateContext {
                    track: track_number,
                    disc: disc_number,
                    title: track_title.clone(),
                    artist: track_artist,
                    album: album_title,
                    album_artist,
                    year: self.album.year.clone(),
                    ext: "mka".to_owned(),
                };
                let parent = target
                    .path
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."));
                let proposed =
                    template::target_path(&parent, &self.settings.rename_template, &context)?;
                self.resolve_rename(&proposed, &target.path, &mut used_targets, &mut notes)
            } else {
                None
            };

            let folder_art = if self.settings.kodi_folder_art {
                cover.as_ref().and_then(|cover| {
                    let directory = rename_to
                        .as_ref()
                        .and_then(|path| path.parent())
                        .or_else(|| target.path.parent())
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from("."));
                    let directory_key = path_key(&directory);
                    let cover_key = cover.cache_key();
                    if let Some((existing_key, _)) = folder_art_seen.get(&directory_key) {
                        if *existing_key != cover_key {
                            notes.push(format!(
                                "Conflicting Kodi folder art for {}; keeping the first cover.",
                                directory.display()
                            ));
                        }
                        return None;
                    }
                    folder_art_seen.insert(directory_key, (cover_key, directory.clone()));
                    Some(FolderArtWrite {
                        directory,
                        filename: match cover.mime {
                            CoverMime::Jpeg => "folder.jpg".to_owned(),
                            CoverMime::Png => "folder.png".to_owned(),
                        },
                        bytes: cover.bytes.clone(),
                    })
                })
            } else {
                None
            };

            jobs.push(ApplyJob {
                target: target.path.clone(),
                target_info: info,
                existing_tags: target.existing_tags.clone(),
                tag_plan,
                cover,
                flags: TrackFlags {
                    set_default: self.settings.set_default,
                    language: self.settings.language.clone(),
                    name: self.settings.track_name.clone(),
                },
                tag_policy: self.settings.tag_policy,
                rename_to,
                overwrite_rename: self.settings.collision_policy == CollisionPolicy::Overwrite,
                keep_backup: self.settings.keep_backup,
                folder_art,
            });
        }

        self.rename_notes = notes.clone();
        if jobs.is_empty() {
            return Err(if notes.is_empty() {
                "No valid pairs were available.".to_owned()
            } else {
                notes.join("\n")
            });
        }
        Ok(jobs)
    }

    fn resolve_rename(
        &self,
        proposed: &Path,
        original: &Path,
        used: &mut HashSet<String>,
        notes: &mut Vec<String>,
    ) -> Option<PathBuf> {
        if proposed == original {
            return None;
        }
        let mut candidate = proposed.to_path_buf();
        let mut key = path_key(&candidate);
        if used.contains(&key) || candidate.exists() {
            match self.settings.collision_policy {
                CollisionPolicy::Skip => {
                    notes.push(format!(
                        "Rename skipped for {}: destination exists.",
                        original.display()
                    ));
                    return None;
                }
                CollisionPolicy::Overwrite => {
                    // The explicit overwrite policy is allowed to replace the
                    // existing destination. In-batch duplicates still get a
                    // suffix so two jobs never target the same file.
                    if used.contains(&key) {
                        let mut number = 2;
                        while used.contains(&key) {
                            candidate = template::unique_suffix(proposed, number);
                            key = path_key(&candidate);
                            number += 1;
                        }
                    }
                }
                CollisionPolicy::Suffix => {
                    let mut number = 2;
                    while used.contains(&key) || candidate.exists() {
                        candidate = template::unique_suffix(proposed, number);
                        key = path_key(&candidate);
                        number += 1;
                        if number > 9999 {
                            notes.push(format!(
                                "Rename skipped for {}: could not find a free name.",
                                original.display()
                            ));
                            return None;
                        }
                    }
                }
            }
        }
        used.insert(key);
        Some(candidate)
    }

    pub fn apply_now(&mut self) {
        match self.build_jobs() {
            Ok(jobs) => {
                let total = jobs.len();
                self.summary = Some(ApplySummary::default());
                self.manifest_error = None;
                self.job = Some(JobProgress {
                    done: 0,
                    total,
                    target: jobs
                        .first()
                        .map(|job| job.target.clone())
                        .unwrap_or_default(),
                });
                let _ = self.worker.tx.send(Command::Apply(jobs));
                self.set_notice(format!("Applying {total} target(s)…"), theme::CYAN);
            }
            Err(error) => self.set_notice(error, theme::ERROR),
        }
    }

    pub fn refresh_rename_notes(&mut self) {
        self.refresh_pairing_scores();
        let mut notes = Vec::new();
        if !self.settings.rename_enabled {
            self.rename_notes.clear();
            return;
        }
        if let Err(error) = template::validate(&self.settings.rename_template) {
            self.rename_notes = vec![error];
            return;
        }
        let mut used = HashSet::new();
        for pairing in self.pairings.clone() {
            let Some(source) = self.source(pairing.source_id) else {
                continue;
            };
            let Some(target) = self.target(pairing.target_id) else {
                continue;
            };
            let draft = self
                .track_drafts
                .get(&source.id)
                .cloned()
                .unwrap_or_else(|| TrackDraft::from_source(source));
            let context = TemplateContext {
                track: draft.track_number_value().or(source.track_number),
                disc: draft.disc_number_value().or(source.disc_number),
                title: if draft.title.trim().is_empty() {
                    source.display_title().to_owned()
                } else {
                    draft.title.clone()
                },
                artist: if draft.artist.trim().is_empty() {
                    source.artist.clone()
                } else {
                    draft.artist.clone()
                },
                album: self.album.title.clone(),
                album_artist: self.album.artist.clone(),
                year: self.album.year.clone(),
                ext: "mka".to_owned(),
            };
            let parent = target
                .path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."));
            match template::target_path(&parent, &self.settings.rename_template, &context) {
                Ok(proposed) => {
                    if proposed != target.path {
                        let key = path_key(&proposed);
                        if used.contains(&key) || proposed.exists() {
                            notes.push(format!("Collision: {}", proposed.display()));
                        }
                        used.insert(key);
                    }
                }
                Err(error) => notes.push(error),
            }
        }
        self.rename_notes = notes;
    }

    pub fn save_settings(&mut self, ctx: &egui::Context) {
        if self.job.is_some() {
            self.set_notice(
                "Wait for the current apply batch to finish before saving settings.",
                theme::WARNING,
            );
            return;
        }
        if let Err(error) = template::validate(&self.settings_draft.rename_template) {
            self.set_notice(error, theme::ERROR);
            return;
        }
        if self.settings_draft.track_name.trim().is_empty() {
            self.settings_draft.track_name = config::DEFAULT_TRACK_NAME.to_owned();
        }
        if self.settings_draft.language.trim().is_empty() {
            self.settings_draft.language = "eng".to_owned();
        }
        self.settings = self.settings_draft.clone();
        if let Err(error) = config::save(&self.settings) {
            self.set_notice(error, theme::ERROR);
            return;
        }
        self.tools = mkv::ToolSet::detect(&self.settings);
        self.worker = worker::start(ctx.clone(), self.tools.clone());
        self.show_settings = false;
        self.refresh_rename_notes();
        self.set_notice("Settings saved.", theme::SUCCESS);
    }

    pub fn open_settings(&mut self) {
        self.settings_draft = self.settings.clone();
        self.show_settings = true;
    }

    pub fn remove_source(&mut self, id: u64) {
        self.sources.retain(|source| source.id != id);
        self.pairings.retain(|pair| pair.source_id != id);
        self.track_drafts.remove(&id);
        self.cover_overrides.remove(&id);
        if self.selected_source == Some(id) {
            self.selected_source = None;
        }
    }

    pub fn remove_target(&mut self, id: u64) {
        self.targets.retain(|target| target.id != id);
        self.pairings.retain(|pair| pair.target_id != id);
        if self.selected_target == Some(id) {
            self.selected_target = None;
        }
    }

    pub fn clear_all(&mut self) {
        self.sources.clear();
        self.targets.clear();
        self.pairings.clear();
        self.track_drafts.clear();
        self.cover_overrides.clear();
        self.textures.clear();
        self.selected_source = None;
        self.selected_target = None;
        self.summary = None;
        self.set_notice("Workspace cleared.", theme::MUTED);
    }

    pub fn apply_result_color(state: crate::model::ApplyState) -> Color32 {
        use crate::model::ApplyState;
        match state {
            ApplyState::Done => theme::SUCCESS,
            ApplyState::Warning => theme::AMBER,
            ApplyState::Failed => theme::ERROR,
            _ => theme::MUTED,
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = dropped
            .into_iter()
            .filter_map(|file| {
                let path = file.path();
                (!path.as_os_str().is_empty()).then(|| path.to_path_buf())
            })
            .collect();
        if !paths.is_empty() {
            self.add_paths(paths, ctx);
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::O)) {
            if ctx.input(|input| input.modifiers.shift) {
                ui::panes::open_target_dialog(self, ctx);
            } else {
                ui::panes::open_source_dialog(self, ctx);
            }
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Enter)) {
            self.apply_now();
        }
        if ctx.input(|input| input.modifiers.command && input.key_pressed(egui::Key::Z))
            && self.undo_available
        {
            self.undo_last();
        }
    }
}

impl eframe::App for SurroundTagApp {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.pump_events(&ctx);
        self.handle_dropped_files(&ctx);
        self.handle_shortcuts(&ctx);

        if !self.bootstrapped {
            self.bootstrapped = true;
            if !self.tools.can_apply() {
                self.set_notice(
                    "MKVToolNix mkvpropedit was not found. Open Settings to locate it.",
                    theme::WARNING,
                );
            }
        }

        ui::top::top_bar(self, root);
        ui::status::status_bar(self, root);
        ui::inspector::inspector(self, root);
        ui::panes::central_panes(self, root);
        ui::dialogs::windows(self, &ctx);
    }
}

fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value.trim().to_owned())
    }
}

fn path_key(path: &Path) -> String {
    // Compare case-insensitively even on Linux: targets commonly live on SMB
    // shares and removable media that are case-insensitive on the other side.
    path.to_string_lossy().to_ascii_lowercase()
}

pub use crate::model::ApplyState;
