//! Studio UI, asynchronous preparation and portable projects.

mod editors;
mod history;
pub mod midi;
pub mod project;
mod rack;
#[cfg(test)]
mod tests;
mod transients;
mod warp_editor;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, mpsc};

use eframe::egui::Context;
use orb_engine::studio::{
    Settings, Studio,
    audio_file::{MAX_AUDIO_BYTES, MAX_FILE_BYTES, Wave},
    convolution::{self, Convolver},
};
use orb_player::Command;

use crate::app::App;
use project::{Metadata, Project};

enum Completed {
    History(
        Box<history::SamplerState>,
        [Option<Box<Wave>>; 8],
        [bool; 8],
    ),
    Transients(usize, transients::Proposal),
    Init(Box<Studio>, Arc<Wave>),
    Wave(usize, String, Arc<Wave>, Box<Wave>),
    Reverb(String, Arc<Wave>, Box<Convolver>),
    Open(PathBuf, Box<Project>, Box<Studio>),
    Save(
        PathBuf,
        Vec<u8>,
        u64,
        crate::sidechain::Settings,
        Option<u8>,
    ),
}

static ASSET_EPOCH: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(super) struct WaveformCache {
    key: Option<(u64, usize)>,
    peaks: Vec<f32>,
}

impl WaveformCache {
    pub(super) fn get(&mut self, epoch: u64, frames: &[[f32; 2]]) -> &[f32] {
        let key = (epoch, frames.as_ptr() as usize);
        if self.key != Some(key) {
            self.peaks.clear();
            for chunk in frames.chunks(frames.len().div_ceil(2048).max(1)) {
                self.peaks
                    .push(chunk.iter().flatten().fold(0.0f32, |m, v| m.max(v.abs())));
            }
            let maximum = self.peaks.iter().copied().fold(0.0f32, f32::max).max(1e-12);
            for peak in &mut self.peaks {
                *peak /= maximum;
            }
            self.key = Some(key);
        }
        &self.peaks
    }
}

pub struct StudioUi {
    history: history::History,
    history_backup: Option<history::History>,
    history_restoring: bool,
    proposals: [Option<transients::Proposal>; 8],
    wave_views: [WaveformCache; 9],
    warp_peaks: [warp_editor::WarpPeaks; 8],
    waveform_epoch: u64,
    pub(crate) context: Context,
    pub modern: bool,
    pub sampler_open: bool,
    pub warp_editor: Option<usize>,
    pub reverb_open: bool,
    pub settings: Settings,
    pub names: [String; 8],
    pub waves: [Option<Arc<Wave>>; 8],
    pub ir: Option<Arc<Wave>>,
    pub ir_name: String,
    pub selected: usize,
    pub bar: usize,
    pub follow: bool,
    pub preset: usize,
    pub decay: f64,
    pub fx: Option<String>,
    pub notice: Option<String>,
    pub content: bool,
    pub(crate) opening: bool,
    mix: Option<(crate::sidechain::Settings, Option<u8>)>,
    ready: bool,
    sent: Settings,
    revision: u64,
    worker: Option<mpsc::Receiver<Result<Completed, String>>>,
    clipboard: Option<[u8; 16]>,
}

impl Default for StudioUi {
    fn default() -> Self {
        let mut state = Self {
            history: Default::default(),
            history_backup: None,
            history_restoring: false,
            proposals: std::array::from_fn(|_| None),
            wave_views: std::array::from_fn(|_| WaveformCache::default()),
            warp_peaks: std::array::from_fn(|_| warp_editor::WarpPeaks::default()),
            waveform_epoch: ASSET_EPOCH.fetch_add(1, Ordering::Relaxed),
            context: Context::default(),
            modern: false,
            sampler_open: false,
            warp_editor: None,
            reverb_open: false,
            settings: Settings::default(),
            names: std::array::from_fn(|i| format!("Sample {:02}", i + 1)),
            waves: std::array::from_fn(|_| None),
            ir: None,
            ir_name: "Concrete chamber".into(),
            selected: 0,
            bar: 0,
            follow: true,
            preset: 1,
            decay: 1.8,
            fx: None,
            notice: None,
            content: false,
            opening: false,
            mix: None,
            ready: false,
            sent: Settings::default(),
            revision: 0,
            worker: None,
            clipboard: None,
        };
        state.history.reset(history::SamplerState::capture(&state));
        state
    }
}

impl StudioUi {
    pub(crate) fn busy(&self) -> bool {
        self.worker.is_some()
    }
    fn work(
        &mut self,
        ctx: &Context,
        job: impl FnOnce() -> Result<Completed, String> + Send + 'static,
    ) {
        let (tx, rx) = mpsc::sync_channel(1);
        self.worker = Some(rx);
        self.notice = None;
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job());
            ctx.request_repaint();
        });
    }

    pub fn project(
        &self,
        song: Vec<u8>,
        sidechain: crate::sidechain::Settings,
        closed_hat: Option<u8>,
    ) -> Option<Project> {
        Some(Project {
            metadata: Metadata {
                settings: self.settings.clone(),
                names: self.names.clone(),
                ir_name: self.ir_name.clone(),
                sidechain,
                closed_hat,
            },
            song,
            waves: self.waves.clone(),
            ir: self.ir.clone()?,
        })
    }

    pub(crate) fn changed(&mut self) {
        self.content = true;
        self.revision = self.revision.wrapping_add(1);
    }
}

impl App {
    fn studio_command(&mut self, command: Command) {
        if let Some(link) = &mut self.link {
            link.effect_command(command);
        }
    }

    pub(crate) fn reset_studio(&mut self) {
        if self.studio.ready {
            self.studio_command(Command::StudioClear);
        }
        let modern = self.studio.modern;
        self.studio = StudioUi {
            modern,
            ..StudioUi::default()
        };
        self.studio_mirror();
    }

    pub(crate) fn studio_poll(&mut self, ctx: &Context) {
        let result = self
            .studio
            .worker
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(result),
                Err(mpsc::TryRecvError::Disconnected) => {
                    Some(Err("Studio worker stopped unexpectedly".into()))
                }
                Err(mpsc::TryRecvError::Empty) => None,
            });
        if let Some(result) = result {
            self.studio.worker = None;
            self.studio.opening = false;
            match result {
                Err(e) => {
                    if let Some(history) = self.studio.history_backup.take() {
                        self.studio.history = history;
                    }
                    self.studio.history_restoring = false;
                    self.after_save = None;
                    self.studio.notice = Some(e);
                }
                Ok(Completed::History(state, runtime, changed)) => {
                    self.studio.history_restoring = false;
                    self.studio.history_backup = None;
                    self.sampler_history_apply(*state, runtime, changed);
                }
                Ok(Completed::Transients(lane, proposal)) => {
                    if proposal.matches(&self.studio, lane) {
                        self.studio.proposals[lane] = Some(proposal);
                    } else {
                        self.studio.notice =
                            Some("Sample region changed; detect transients again.".into());
                    }
                }
                Ok(Completed::Init(studio, ir)) => {
                    self.studio.ir = Some(ir);
                    self.studio.ready = true;
                    self.studio_command(Command::Studio(studio));
                }
                Ok(Completed::Wave(lane, name, wave, runtime)) => {
                    self.studio.waveform_epoch = ASSET_EPOCH.fetch_add(1, Ordering::Relaxed);
                    self.studio.names[lane] = name;
                    self.studio.waves[lane] = Some(wave);
                    self.studio.settings.lanes[lane].start = 0.0;
                    self.studio.settings.lanes[lane].end = 1.0;
                    self.studio.settings.lanes[lane].warp = false;
                    self.studio.settings.lanes[lane].warp_bpm = 120.0;
                    self.studio.settings.lanes[lane].warp_beats = 4;
                    self.studio.settings.lanes[lane].warp_grid = false;
                    self.studio.settings.lanes[lane].warp_markers = Default::default();
                    self.studio.changed();
                    self.doc.modified = true;
                    self.studio_command(Command::StudioWave {
                        lane,
                        wave: Some(runtime),
                    });
                }
                Ok(Completed::Reverb(name, ir, processor)) => {
                    self.studio.waveform_epoch = ASSET_EPOCH.fetch_add(1, Ordering::Relaxed);
                    self.studio.ir_name = name;
                    self.studio.ir = Some(ir);
                    self.studio.changed();
                    self.doc.modified = true;
                    self.studio_command(Command::StudioReverb(processor));
                }
                Ok(Completed::Open(path, project, runtime)) => {
                    match crate::session::open_bytes(&project.song, &path.display().to_string()) {
                        Err(e) => self.studio.notice = Some(e),
                        Ok(opened) => {
                            let settlement = self.settlement;
                            self.adopt(&path, opened);
                            if self.settlement == settlement {
                                return;
                            }
                            self.studio.settings = project.metadata.settings.clone();
                            self.studio.sent = self.studio.settings.clone();
                            self.studio.names = project.metadata.names.clone();
                            self.studio.ir_name = project.metadata.ir_name.clone();
                            self.studio.ir = Some(project.ir.clone());
                            self.studio.waves = project.waves.clone();
                            self.settings.sidechain = project.metadata.sidechain.clone();
                            self.settings.closed_hat_level = project.metadata.closed_hat;
                            self.studio.content = true;
                            self.studio.ready = true;
                            self.sampler_history_reset();
                            self.studio_command(Command::Studio(runtime));
                            self.studio_mirror();
                            self.doc.modified = false;
                        }
                    }
                }
                Ok(Completed::Save(path, song, revision, sidechain, closed_hat)) => {
                    let unchanged = self.studio.revision == revision
                        && self.settings.sidechain == sidechain
                        && self.settings.closed_hat_level == closed_hat
                        && self.song_bytes().ok().as_ref() == Some(&song);
                    self.doc.path = Some(path.clone());
                    if unchanged {
                        self.doc.modified = false;
                    }
                    self.status = Some(format!("Saved Studio project {}", path.display()));
                    if unchanged && let Some(then) = self.after_save.take() {
                        self.proceed(then);
                    } else {
                        self.after_save = None;
                    }
                }
            }
        }
        let mix = (
            self.settings.sidechain.clone(),
            self.settings.closed_hat_level,
        );
        if self.studio.content && self.studio.mix.as_ref().is_some_and(|was| was != &mix) {
            self.studio.changed();
            self.doc.modified = true;
        }
        self.studio.mix = Some(mix);
        if self.studio.worker.is_none() && !self.studio.ready && self.studio.modern {
            self.studio.work(ctx, || {
                let ir = Arc::new(Wave {
                    frames: convolution::synthetic(1, 1.8),
                });
                let studio = Box::new(Studio::new(Box::new(Convolver::new(&ir.frames)?)));
                Ok(Completed::Init(studio, ir))
            });
        }
        if self.studio.ready && self.studio.settings != self.studio.sent {
            self.studio.settings.sanitize();
            self.studio.sent = self.studio.settings.clone();
            self.studio_command(Command::StudioSettings(Box::new(self.studio.sent.clone())));
        }
        if self.studio.worker.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        self.sampler_history_poll(ctx);
    }

    pub(crate) fn studio_open(&mut self, path: &Path, ctx: &Context) {
        if self.studio.worker.is_some() {
            self.fail("Studio is busy; wait for the current operation.");
            return;
        }
        let path = path.to_path_buf();
        self.studio.opening = true;
        self.studio.work(ctx, move || {
            if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 256 * 1024 * 1024 {
                return Err("Studio project exceeds 256 MiB".into());
            }
            let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
            let project = Box::new(Project::decode(&bytes)?);
            let runtime = project.prepare()?;
            Ok(Completed::Open(path, project, runtime))
        });
    }

    pub(crate) fn studio_save(&mut self, path: &Path, ctx: &Context) {
        if self.studio.worker.is_some() {
            self.after_save = None;
            self.fail("Studio is busy; wait before saving.");
            return;
        }
        let song = match self.song_bytes() {
            Ok(song) => song,
            Err(e) => {
                self.after_save = None;
                self.fail(e);
                return;
            }
        };
        let Some(project) = self.studio.project(
            song.clone(),
            self.settings.sidechain.clone(),
            self.settings.closed_hat_level,
        ) else {
            self.after_save = None;
            self.fail("Studio is still preparing; try again.");
            return;
        };
        let mut path = path.to_path_buf();
        path.set_extension("orb338");
        let revision = self.studio.revision;
        let sidechain = project.metadata.sidechain.clone();
        let closed_hat = project.metadata.closed_hat;
        self.studio.work(ctx, move || {
            let bytes = project.encode()?;
            crate::session::write_file(&path, &bytes)?;
            Ok(Completed::Save(path, song, revision, sidechain, closed_hat))
        });
    }

    fn studio_load_wave(&mut self, ctx: &Context, lane: usize) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Audio", &["wav", "aif", "aiff", "mp3"])
            .pick_file()
        else {
            return;
        };
        let existing: usize = self
            .studio
            .waves
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != lane)
            .filter_map(|(_, w)| w.as_ref())
            .map(|w| w.frames.len() * 8)
            .sum();
        self.studio.work(ctx, move || {
            let wave = load_wave(&path)?;
            if existing + wave.frames.len() * 8 > MAX_AUDIO_BYTES {
                return Err(
                    "Sampler bank exceeds 240 MiB; shorten or remove another sample".into(),
                );
            }
            let runtime = Box::new(wave.clone());
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            Ok(Completed::Wave(lane, name, Arc::new(wave), runtime))
        });
    }

    fn studio_build_ir(&mut self, ctx: &Context, imported: bool) {
        let path = if imported {
            rfd::FileDialog::new()
                .add_filter("Impulse response", &["wav", "aif", "aiff"])
                .pick_file()
        } else {
            None
        };
        if imported && path.is_none() {
            return;
        }
        let preset = self.studio.preset;
        let decay = self.studio.decay;
        self.studio.work(ctx, move || {
            let (name, mut wave) = match path {
                Some(path) => {
                    let wave = load_wave(&path)?;
                    if wave.frames.len() > 6 * 44100 {
                        return Err("IR must be at most 6 seconds".into());
                    }
                    (
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        wave,
                    )
                }
                None => (
                    format!("{} / {decay:.1} s", IR_NAMES[preset]),
                    Wave {
                        frames: convolution::synthetic(preset, decay),
                    },
                ),
            };
            // Energy normalization prevents arbitrary file gain from overloading the return.
            let energy = wave
                .frames
                .iter()
                .flatten()
                .map(|&x| f64::from(x).powi(2))
                .sum::<f64>()
                * 0.5;
            let gain = (1.0 / energy.sqrt().max(1.0)) as f32;
            for x in wave.frames.iter_mut().flatten() {
                *x *= gain;
            }
            let processor = Box::new(Convolver::new(&wave.frames)?);
            Ok(Completed::Reverb(name, Arc::new(wave), processor))
        });
    }
}

fn load_wave(path: &Path) -> Result<Wave, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES as u64 {
        return Err("Audio file exceeds 256 MiB".into());
    }
    Wave::decode(&std::fs::read(path).map_err(|e| e.to_string())?)
}

const IR_NAMES: [&str; 4] = [
    "Tight room",
    "Concrete chamber",
    "Warehouse",
    "Diffuse plate",
];
