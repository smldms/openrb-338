//! Musical-time waveform editor. Anchors keep their original source sample positions.

use eframe::egui::{self, Align2, Color32, Context, FontId, Rect, Sense, Stroke, Ui, Vec2};
use orb_engine::studio::sampler::{Lane, MAX_WARP_MARKERS};
use orb_player::Command;

use crate::app::App;

const ACID: Color32 = Color32::from_rgb(190, 215, 126);

/// Multi-resolution peaks are built on the UI side once per sample, never in the callback.
#[derive(Default)]
pub(super) struct WarpPeaks {
    key: Option<(u64, usize)>,
    levels: Vec<Vec<f32>>,
    maximum: f32,
}

impl WarpPeaks {
    fn prepare(&mut self, epoch: u64, frames: &[[f32; 2]]) {
        let key = (epoch, frames.as_ptr() as usize);
        if self.key == Some(key) {
            return;
        }
        self.levels.clear();
        let base: Vec<f32> = frames
            .chunks(64)
            .map(|chunk| chunk.iter().flatten().fold(0.0_f32, |m, v| m.max(v.abs())))
            .collect();
        self.maximum = base.iter().copied().fold(1e-12_f32, f32::max);
        self.levels.push(base);
        while self.levels.last().is_some_and(|level| level.len() > 1) {
            let next = self
                .levels
                .last()
                .unwrap()
                .chunks(2)
                .map(|pair| pair.iter().copied().fold(0.0_f32, f32::max))
                .collect();
            self.levels.push(next);
        }
        self.key = Some(key);
    }

    fn peak(&self, frames: &[[f32; 2]], start: usize, end: usize) -> f32 {
        let start = start.min(frames.len());
        let end = end.min(frames.len()).max(start);
        let length = end - start;
        let peak = if length <= 64 {
            frames[start..end]
                .iter()
                .flatten()
                .fold(0.0_f32, |m, v| m.max(v.abs()))
        } else {
            let mut level = 0;
            let mut bucket = 64;
            while bucket * 2 <= length && level + 1 < self.levels.len() {
                level += 1;
                bucket *= 2;
            }
            self.levels.get(level).map_or(0.0, |peaks| {
                peaks[start / bucket..end.div_ceil(bucket).min(peaks.len())]
                    .iter()
                    .copied()
                    .fold(0.0_f32, f32::max)
            })
        };
        peak / self.maximum.max(1e-12)
    }
}

fn snapped(target: f64, beats: u16, snap: bool) -> f64 {
    if snap {
        let divisions = f64::from(beats.max(1)) * 4.0;
        (target * divisions).round() / divisions
    } else {
        target
    }
}

/// Labels are bar.beat; a bar is four quarter notes, matching the sampler sequencer.
fn beat_label(beat: usize) -> String {
    format!("{}.{}", beat / 4 + 1, beat % 4 + 1)
}

fn draw_grid(ui: &Ui, rect: Rect, beats: u16) {
    let beats = usize::from(beats.max(1));
    let spacing = rect.width() / beats as f32;
    let label_every = (36.0 / spacing).ceil().max(1.0) as usize;
    let label_every = if label_every > 1 {
        label_every.div_ceil(4) * 4
    } else {
        1
    };
    for division in 0..=beats * 4 {
        let x = rect.left() + division as f32 * spacing / 4.0;
        if x < ui.clip_rect().left() - 2.0 || x > ui.clip_rect().right() + 2.0 {
            continue;
        }
        let bar = division % 16 == 0;
        let beat = division % 4 == 0;
        if !beat && spacing < 16.0 {
            continue;
        }
        let color = if bar {
            115
        } else if beat {
            68
        } else {
            35
        };
        ui.painter().line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            Stroke::new(if bar { 1.5 } else { 1.0 }, Color32::from_gray(color)),
        );
        if beat && (division / 4) % label_every == 0 {
            ui.painter().text(
                egui::pos2(x + 3.0, rect.top() + 3.0),
                Align2::LEFT_TOP,
                beat_label(division / 4),
                FontId::monospace(11.0),
                Color32::LIGHT_GRAY,
            );
        }
    }
}

fn marker_waveform(
    ui: &mut Ui,
    lane: &mut Lane,
    frames: &[[f32; 2]],
    peaks: &WarpPeaks,
    width: f32,
    snap: bool,
    selected: &mut Option<usize>,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 190.0), Sense::click_and_drag());
    ui.painter().rect_filled(rect, 3.0, Color32::from_gray(16));
    draw_grid(ui, rect, lane.warp_beats);
    if !frames.is_empty() {
        let left = ((ui.clip_rect().left() - rect.left()).max(0.0)) as usize;
        let right = ((ui.clip_rect().right() - rect.left())
            .min(rect.width())
            .max(0.0)) as usize;
        for x in left..right {
            let a = lane
                .warp_markers
                .map(x as f64 / f64::from(width), lane.start, lane.end)
                .0;
            let b = lane
                .warp_markers
                .map((x + 1) as f64 / f64::from(width), lane.start, lane.end)
                .0;
            let a = ((a * frames.len() as f64) as usize).min(frames.len() - 1);
            let b = ((b * frames.len() as f64).ceil() as usize).clamp(a + 1, frames.len());
            let peak = peaks.peak(frames, a, b);
            let x = rect.left() + x as f32;
            ui.painter().line_segment(
                [
                    egui::pos2(x, rect.center().y - peak * 65.0),
                    egui::pos2(x, rect.center().y + peak * 65.0),
                ],
                Stroke::new(1.0, ACID),
            );
        }
    }
    if let Some(pointer) = response.interact_pointer_pos() {
        let target = f64::from((pointer.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
        let snap = snap && !ui.input(|i| i.modifiers.shift);
        if response.double_clicked_by(egui::PointerButton::Primary) {
            let source = lane.warp_markers.map(target, lane.start, lane.end).0;
            let target = snapped(target, lane.warp_beats, snap);
            if let Some(index) = lane
                .warp_markers
                .insert(source, target, lane.start, lane.end)
            {
                *selected = Some(index);
                lane.warp = true;
                lane.warp_grid = true;
            }
            response.request_focus();
        } else if response.clicked_by(egui::PointerButton::Primary)
            || response.drag_started_by(egui::PointerButton::Primary)
        {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(pointer);
            let origin = f64::from((origin.x - rect.left()) / rect.width());
            *selected = lane
                .warp_markers
                .points()
                .iter()
                .enumerate()
                .filter(|(_, p)| ((p.target - origin) * f64::from(width)).abs() <= 9.0)
                .min_by(|(_, a), (_, b)| {
                    (a.target - origin)
                        .abs()
                        .total_cmp(&(b.target - origin).abs())
                })
                .map(|(i, _)| i);
            response.request_focus();
        }
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(index) = *selected
        {
            lane.warp_markers
                .move_target(index, snapped(target, lane.warp_beats, snap));
        }
    }
    for (index, point) in lane.warp_markers.points().iter().enumerate() {
        let x = rect.left() + point.target as f32 * rect.width();
        let color = ACID;
        ui.painter().line_segment(
            [
                egui::pos2(x, rect.top() + 23.0),
                egui::pos2(x, rect.bottom()),
            ],
            Stroke::new(if *selected == Some(index) { 3.0 } else { 1.5 }, color),
        );
        ui.painter().text(
            egui::pos2(x + 4.0, rect.bottom() - 4.0),
            Align2::LEFT_BOTTOM,
            format!("W{}", index + 1),
            FontId::monospace(11.0),
            color,
        );
    }
    if response.has_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Delete))
        && let Some(index) = selected.take()
    {
        lane.warp_markers.remove(index);
    }
    response
}

impl App {
    pub(super) fn warp_editor(&mut self, ctx: &Context) {
        let Some(index) = self.studio.warp_editor.filter(|&i| i < 8) else {
            return;
        };
        let Some(wave) = self.studio.waves[index].clone() else {
            self.studio.warp_editor = None;
            return;
        };
        let id = egui::Id::unique(("warp-grid-state", index, self.studio.waveform_epoch));
        let (mut zoom, mut snap, mut selected) = ctx
            .data(|d| d.get_temp::<(f32, bool, Option<usize>)>(id))
            .unwrap_or((1.0, true, None));
        let before = self.studio.settings.lanes[index].clone();
        let suggestions = self.studio.proposals[index]
            .as_ref()
            .filter(|p| p.matches(&self.studio, index))
            .map(|p| p.sources.clone());
        let mut estimated_bpm = self.studio.proposals[index]
            .as_ref()
            .filter(|p| p.matches(&self.studio, index))
            .and_then(|p| p.bpm);
        let mut detect = false;
        let mut open = true;
        let mut audition = false;
        egui::Window::new(format!("WARP / {}", self.studio.names[index]))
            .id(egui::Id::unique(("warp-window", index)))
            .open(&mut open)
            .default_width(900.0)
            .min_width(480.0)
            .resizable(true)
            .show(ctx, |ui| {
                if self.studio.history_restoring {
                    ui.disable();
                }
                ui.horizontal_wrapped(|ui| {
                    self.sampler_history_buttons(ui);
                    detect = ui
                        .add_enabled(
                            !self.studio.busy(),
                            egui::Button::new("ANALYZE BPM / TRANSIENTS"),
                        )
                        .clicked();
                    if self.studio.busy() {
                        ui.spinner();
                    }
                });
                let lane = &mut self.studio.settings.lanes[index];
                if selected.is_some_and(|i| i >= lane.warp_markers.points().len()) {
                    selected = None;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.label(estimated_bpm.map_or_else(
                        || {
                            if suggestions.is_some() {
                                "Est. BPM: unavailable (too short or no regular pulse)".into()
                            } else {
                                "Est. BPM: — (analyze a rhythmic sample)".into()
                            }
                        },
                        |bpm| format!("Est. BPM: {bpm:.1}"),
                    ));
                    for (label, factor) in [("÷2", 0.5), ("×2", 2.0)] {
                        let valid = estimated_bpm
                            .is_some_and(|bpm| (20.0..=400.0).contains(&(bpm * factor)));
                        if ui.add_enabled(valid, egui::Button::new(label)).clicked() {
                            estimated_bpm = estimated_bpm.map(|bpm| bpm * factor);
                            if let Some(proposal) = &mut self.studio.proposals[index] {
                                proposal.bpm = estimated_bpm;
                            }
                        }
                    }
                    let beats = estimated_bpm.and_then(|bpm| {
                        super::transients::estimated_beats(bpm, wave.frames.len(), lane)
                    });
                    let label = beats.map_or_else(
                        || "USE ESTIMATE".into(),
                        |beats| format!("USE ~{beats} BEATS"),
                    );
                    if ui
                        .add_enabled(beats.is_some(), egui::Button::new(label))
                        .on_hover_text(
                            "Fit IN/OUT to the nearest whole beat count from the estimate. \
                            Existing anchors are kept; the song tempo stays unchanged.",
                        )
                        .clicked()
                    {
                        lane.warp_beats = beats.unwrap();
                        lane.fit_warp(Some(wave.frames.len()));
                    }
                });
                let peaks = &mut self.studio.warp_peaks[index];
                peaks.prepare(self.studio.waveform_epoch, &wave.frames);
                ui.label(
                    "Double-click a transient to add an anchor; \
                    drag its line to stretch the surrounding audio.",
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut lane.warp, "WARP").changed() && lane.warp {
                        lane.fit_warp(Some(wave.frames.len()));
                    }
                    if ui
                        .add(
                            egui::DragValue::new(&mut lane.warp_beats)
                                .range(1..=256)
                                .prefix("Beats "),
                        )
                        .changed()
                    {
                        lane.warp_grid = true;
                    }
                    ui.checkbox(&mut snap, "Snap 1/16");
                    ui.add(
                        egui::Slider::new(&mut zoom, 1.0..=16.0)
                            .text("Zoom")
                            .logarithmic(true),
                    );
                    audition = ui.button("AUDITION").clicked();
                });
                ui.small(
                    "Ruler: measure.beat (4/4). Strong lines = bars; \
                    lighter lines = beats and sixteenths. Shift = free placement.",
                );
                let mut grid_mode = lane.warp_grid || !lane.warp_markers.points().is_empty();
                if !grid_mode {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("FIT TO BEATS").clicked() {
                            lane.fit_warp(Some(wave.frames.len()));
                            grid_mode = true;
                        }
                        ui.label(
                            "Grid preview only: fit to Beats before editing markers. \
                            Existing BPM playback is preserved until then.",
                        );
                    });
                }
                if let Some(sources) = &suggestions {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} onset proposals (dim green)", sources.len()));
                        if ui
                            .add_enabled(
                                grid_mode
                                    && !sources.is_empty()
                                    && lane.warp_markers.points().len() < MAX_WARP_MARKERS,
                                egui::Button::new("APPLY SUGGESTIONS"),
                            )
                            .clicked()
                        {
                            super::transients::apply(lane, sources);
                            if let Some(proposal) = &mut self.studio.proposals[index] {
                                proposal.sources.clear();
                            }
                        }
                        if ui.button("Dismiss analysis").clicked() {
                            self.studio.proposals[index] = None;
                        }
                    });
                }
                let width = ui.available_width().max(440.0) * zoom;
                egui::ScrollArea::horizontal()
                    .id_salt(("warp-scroll", index))
                    .show(ui, |ui| {
                        ui.add_enabled_ui(grid_mode, |ui| {
                            let response = marker_waveform(
                                ui,
                                lane,
                                &wave.frames,
                                peaks,
                                width,
                                snap,
                                &mut selected,
                            );
                            if let Some(sources) = &suggestions {
                                for &source in sources {
                                    let target = lane
                                        .warp_markers
                                        .target_for_source(source, lane.start, lane.end);
                                    let x = response.rect.left()
                                        + target as f32 * response.rect.width();
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(x, response.rect.top()),
                                            egui::pos2(x, response.rect.bottom()),
                                        ],
                                        Stroke::new(1.0, ACID.gamma_multiply(0.35)),
                                    );
                                }
                            }
                            if let Some(head) = self.panel.sampler_playheads[index] {
                                super::editors::draw_playhead(ui, response.rect, head.musical);
                            }
                        });
                    });
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{} / {MAX_WARP_MARKERS} anchors",
                        lane.warp_markers.points().len()
                    ));
                    let point = selected
                        .and_then(|i| lane.warp_markers.points().get(i))
                        .copied();
                    if let Some(point) = point {
                        let seconds = point.source * wave.frames.len() as f64 / 44100.0;
                        ui.label(format!("Source {seconds:.3} s"));
                        let mut beat = point.target * f64::from(lane.warp_beats);
                        if ui
                            .add(
                                egui::DragValue::new(&mut beat)
                                    .speed(0.01)
                                    .prefix("Beat offset ")
                                    .fixed_decimals(3),
                            )
                            .changed()
                        {
                            let target = beat / f64::from(lane.warp_beats);
                            lane.warp_markers.move_target(selected.unwrap(), target);
                        }
                    }
                    if ui
                        .add_enabled(selected.is_some(), egui::Button::new("Remove selected"))
                        .clicked()
                        && let Some(i) = selected.take()
                    {
                        lane.warp_markers.remove(i);
                    }
                    if ui.button("Reset markers").clicked() {
                        lane.warp_markers = Default::default();
                        selected = None;
                    }
                });
                if lane.reverse {
                    ui.small("Reverse playback mirrors this source map in time.");
                }
                ui.small(
                    "Set Beats to the musical length of the trimmed IN/OUT region. \
                    Endpoints stay fixed; markers cannot cross.",
                );
            });
        if self.studio.settings.lanes[index] != before {
            self.studio.changed();
            self.doc.modified = true;
        }
        ctx.data_mut(|d| d.insert_temp(id, (zoom, snap, selected)));
        if audition {
            self.studio_command(Command::StudioAudition(index));
        }
        if detect {
            self.sampler_detect(ctx, index);
        }
        if !open {
            self.studio.warp_editor = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_uses_finer_peaks_and_cache_size_stays_proportional_to_sample_frames() {
        let mut frames = vec![[0.0; 2]; 100000];
        frames[54321] = [1.0, -1.0];
        let mut cache = WarpPeaks::default();
        cache.prepare(1, &frames);
        assert_eq!(cache.peak(&frames, 0, frames.len()), 1.0);
        assert_eq!(cache.peak(&frames, 54321, 54322), 1.0);
        assert_eq!(cache.peak(&frames, 54322, 54323), 0.0);
        assert!(cache.levels.iter().map(Vec::len).sum::<usize>() < frames.len() / 16);
    }

    #[test]
    fn waveform_double_click_adds_anchor_and_drag_moves_only_musical_time() {
        let ctx = Context::default();
        let mut lane = Lane::default();
        let mut selected = None;
        let mut time = 0.0;
        let frames = [[0.5; 2]; 100];
        let mut peaks = WarpPeaks::default();
        peaks.prepare(0, &frames);
        let mut frame = |lane: &mut Lane, selected: &mut Option<usize>, events| {
            time += 0.04;
            let mut rect = Rect::NOTHING;
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(
                        egui::Pos2::ZERO,
                        Vec2::new(900.0, 300.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    rect = marker_waveform(ui, lane, &frames, &peaks, 800.0, true, selected).rect;
                },
            );
            output.textures_delta.clear();
            rect
        };
        let rect = frame(&mut lane, &mut selected, vec![]);
        let start = egui::pos2(rect.left() + 400.0, rect.center().y);
        let event = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        for _ in 0..2 {
            frame(
                &mut lane,
                &mut selected,
                vec![egui::Event::PointerMoved(start), event(start, true)],
            );
            frame(&mut lane, &mut selected, vec![event(start, false)]);
        }
        assert_eq!(lane.warp_markers.points().len(), 1);
        assert_eq!(lane.warp_markers.points()[0].source, 0.5);
        let end = egui::pos2(rect.left() + 600.0, rect.center().y);
        frame(&mut lane, &mut selected, vec![event(start, true)]);
        frame(
            &mut lane,
            &mut selected,
            vec![egui::Event::PointerMoved(end)],
        );
        frame(&mut lane, &mut selected, vec![event(end, false)]);
        let point = lane.warp_markers.points()[0];
        assert_eq!((point.source, point.target), (0.5, 0.75));
        assert!(lane.warp);
        assert_eq!(beat_label(4), "2.1");
        assert_eq!(snapped(0.48, 4, true), 0.5);
        assert_eq!(snapped(0.48, 4, false), 0.48);
    }
}
