//! Background probing, applying, and undo jobs.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use eframe::egui;

use crate::audio;
use crate::matching;
use crate::mkv::{self, apply::ApplyJob, undo};
use crate::model::{ApplyItemResult, ApplySummary, ProbeStatus, SourceEntry, TargetEntry};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeKind {
    Sources,
    Targets,
}

pub enum Command {
    ProbeSources(Vec<(u64, PathBuf)>),
    ProbeTargets(Vec<(u64, PathBuf)>),
    Apply(Vec<ApplyJob>),
    Undo,
}

pub enum Event {
    SourceProbed(SourceEntry),
    TargetProbed(TargetEntry),
    ProbeFinished {
        kind: ProbeKind,
        errors: Vec<String>,
    },
    ApplyProgress {
        done: usize,
        total: usize,
        target: PathBuf,
    },
    ApplyItem(ApplyItemResult),
    ApplyFinished {
        summary: ApplySummary,
        manifest_error: Option<String>,
    },
    UndoFinished(Result<undo::UndoReport, String>),
}

pub struct Worker {
    pub tx: Sender<Command>,
    pub rx: Receiver<Event>,
}

pub fn start(ctx: egui::Context, tools: mkv::ToolSet) -> Worker {
    let (command_tx, command_rx) = mpsc::channel::<Command>();
    let (event_tx, event_rx) = mpsc::channel::<Event>();

    thread::Builder::new()
        .name("surroundtag-worker".to_owned())
        .spawn(move || {
            while let Ok(command) = command_rx.recv() {
                match command {
                    Command::ProbeSources(entries) => {
                        probe_sources(entries, &event_tx, &ctx);
                        let _ = event_tx.send(Event::ProbeFinished {
                            kind: ProbeKind::Sources,
                            errors: Vec::new(),
                        });
                    }
                    Command::ProbeTargets(entries) => {
                        probe_targets(entries, &tools, &event_tx, &ctx);
                        let _ = event_tx.send(Event::ProbeFinished {
                            kind: ProbeKind::Targets,
                            errors: Vec::new(),
                        });
                    }
                    Command::Apply(jobs) => {
                        apply_batch(jobs, &tools, &event_tx, &ctx);
                    }
                    Command::Undo => {
                        let result = undo::undo_last();
                        let _ = event_tx.send(Event::UndoFinished(result));
                        ctx.request_repaint();
                    }
                }
            }
        })
        .expect("spawn surroundtag worker");

    Worker {
        tx: command_tx,
        rx: event_rx,
    }
}

fn probe_sources(entries: Vec<(u64, PathBuf)>, events: &Sender<Event>, ctx: &egui::Context) {
    for (id, path) in entries {
        let mut source = SourceEntry::new(id, path.clone());
        source.status = ProbeStatus::Probing;
        let _ = events.send(Event::SourceProbed(source.clone()));
        ctx.request_repaint();

        match audio::probe_source(&path) {
            Ok(probed) => {
                source.title = if probed.title.trim().is_empty() {
                    source.title
                } else {
                    probed.title
                };
                source.artist = probed.artist;
                source.album = probed.album;
                source.album_artist = probed.album_artist;
                source.year = probed.year;
                source.genre = probed.genre;
                source.track_number = probed.track_number.or_else(|| {
                    path.file_stem()
                        .and_then(|stem| stem.to_str())
                        .and_then(matching::extract_track_number)
                });
                source.disc_number = probed.disc_number;
                source.total_tracks = probed.total_tracks;
                source.duration_secs = probed.duration_secs;
                source.cover = probed.cover;
                source.status = ProbeStatus::Ready;
                source.error = None;
            }
            Err(error) => {
                source.status = ProbeStatus::Error;
                source.error = Some(error);
            }
        }

        let _ = events.send(Event::SourceProbed(source));
        ctx.request_repaint();
    }
}

fn probe_targets(
    entries: Vec<(u64, PathBuf)>,
    tools: &mkv::ToolSet,
    events: &Sender<Event>,
    ctx: &egui::Context,
) {
    for (id, path) in entries {
        let mut target = TargetEntry::new(id, path.clone());
        target.status = ProbeStatus::Probing;
        let _ = events.send(Event::TargetProbed(target.clone()));
        ctx.request_repaint();

        match mkv::probe_target(&path, tools) {
            Ok(probe) => {
                target.duration_secs = probe.info.duration_secs;
                target.track_number = probe.track_number;
                target.info = Some(probe.info);
                target.existing_tags = probe.tags;
                target.status = ProbeStatus::Ready;
                target.error = None;
            }
            Err(error) => {
                target.status = ProbeStatus::Error;
                target.error = Some(error);
            }
        }

        let _ = events.send(Event::TargetProbed(target));
        ctx.request_repaint();
    }
}

fn apply_batch(
    jobs: Vec<ApplyJob>,
    tools: &mkv::ToolSet,
    events: &Sender<Event>,
    ctx: &egui::Context,
) {
    let total = jobs.len();
    let mut summary = ApplySummary::default();
    let mut rename_entries = Vec::new();
    let started = std::time::Instant::now();

    for (index, job) in jobs.into_iter().enumerate() {
        let target = job.target.clone();
        let _ = events.send(Event::ApplyProgress {
            done: index,
            total,
            target,
        });
        ctx.request_repaint();

        let outcome = mkv::apply_job(&job, tools);
        if let Some(entry) = outcome.rename_entry {
            rename_entries.push(entry);
        }
        summary.results.push(outcome.result.clone());
        let _ = events.send(Event::ApplyItem(outcome.result));
        ctx.request_repaint();
    }

    let manifest_error = if rename_entries.is_empty() {
        None
    } else {
        undo::save_manifest(rename_entries).err()
    };

    summary.elapsed_secs = started.elapsed().as_secs_f64();
    summary.recompute();
    let _ = events.send(Event::ApplyFinished {
        summary,
        manifest_error,
    });
    ctx.request_repaint();
}
