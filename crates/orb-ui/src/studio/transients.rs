//! Optional onset proposals, analyzed off the audio/UI threads and explicitly accepted by users.

use super::{Completed, StudioUi};
use crate::app::App;
use orb_engine::studio::{
    audio_file::Wave,
    sampler::{Lane, MAX_WARP_MARKERS},
};

#[derive(Clone)]
pub(super) struct Proposal {
    epoch: u64,
    start: f64,
    end: f64,
    pub sources: Vec<f64>,
    pub bpm: Option<f64>,
}

impl Proposal {
    pub fn matches(&self, studio: &StudioUi, lane: usize) -> bool {
        self.epoch == studio.waveform_epoch
            && studio.waves[lane].is_some()
            && self.start == studio.settings.lanes[lane].start
            && self.end == studio.settings.lanes[lane].end
    }
}

fn detect(wave: &Wave, start: f64, end: f64) -> (Vec<f64>, Option<f64>) {
    const WINDOW: usize = 256;
    let frames = wave.frames.len();
    let start = ((start * frames as f64) as usize).min(frames);
    let end = ((end * frames as f64) as usize).min(frames).max(start);
    let region = &wave.frames[start..end];
    let energy: Vec<f64> = region
        .chunks(WINDOW)
        .map(|chunk| {
            chunk
                .iter()
                .flatten()
                .map(|&x| f64::from(x).powi(2))
                .sum::<f64>()
                / (chunk.len() * 2) as f64
        })
        .collect();
    let maximum = energy.iter().copied().fold(0.0_f64, f64::max);
    if maximum < 1e-12 {
        return (Vec::new(), None);
    }
    let mut baseline = 0.0;
    let novelty: Vec<f64> = energy
        .iter()
        .map(|&e| {
            let score = (e - baseline).max(0.0);
            baseline += (e - baseline) * 0.2;
            score
        })
        .collect();
    let threshold = novelty.iter().copied().fold(0.0_f64, f64::max) * 0.2;
    let bpm = estimate_bpm(&novelty);
    let mut candidates: Vec<(usize, f64)> = (1..novelty.len().saturating_sub(1))
        .filter(|&i| {
            novelty[i] > threshold
                && energy[i] > maximum * 0.005
                && novelty[i] >= novelty[i - 1]
                && novelty[i] > novelty[i + 1]
        })
        .map(|i| {
            let a = i * WINDOW;
            let b = (a + WINDOW).min(region.len());
            let peak = region[a..b]
                .iter()
                .flatten()
                .fold(0.0_f32, |m, v| m.max(v.abs()));
            let offset = region[a..b]
                .iter()
                .position(|x| x[0].abs().max(x[1].abs()) >= peak * 0.25)
                .unwrap_or(0);
            (start + a + offset, novelty[i])
        })
        .filter(|(position, _)| *position > start + 661 && *position + 661 < end)
        .collect();
    candidates.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut chosen: Vec<usize> = Vec::new();
    for (position, _) in candidates {
        if chosen.iter().all(|&other| position.abs_diff(other) >= 2205) {
            chosen.push(position);
            if chosen.len() == MAX_WARP_MARKERS {
                break;
            }
        }
    }
    chosen.sort_unstable();
    let sources = chosen
        .into_iter()
        .map(|position| position as f64 / frames.max(1) as f64)
        .collect();
    (sources, bpm)
}

/// Positive onset-envelope autocorrelation. Half/double-time ambiguity is left to the user.
fn estimate_bpm(novelty: &[f64]) -> Option<f64> {
    const RATE: f64 = 44100.0 / 256.0;
    let first = (RATE * 60.0 / 200.0).floor() as usize;
    let last = (RATE * 60.0 / 50.0).ceil() as usize;
    if novelty.len() < first * 4 {
        return None;
    }
    let mean = novelty.iter().sum::<f64>() / novelty.len() as f64;
    let centered: Vec<f64> = novelty.iter().map(|v| v - mean).collect();
    let mut scores = vec![0.0; last + 2];
    for (lag, score) in scores.iter_mut().enumerate().take(last + 2).skip(first - 1) {
        if lag * 3 >= centered.len() {
            continue;
        }
        let (mut cross, mut a, mut b) = (0.0, 0.0, 0.0);
        for (&x, &y) in centered.iter().zip(&centered[lag..]) {
            cross += x * y;
            a += x * x;
            b += y * y;
        }
        if a * b > 1e-24 {
            *score = cross / (a * b).sqrt();
        }
    }
    let best = (first..=last)
        .filter(|&i| scores[i] > 0.2 && scores[i] >= scores[i - 1] && scores[i] > scores[i + 1])
        .max_by(|&a, &b| {
            let rank = |i: usize| {
                let bpm = RATE * 60.0 / i as f64;
                scores[i] * (1.0 - 0.12 * (bpm / 120.0).log2().abs())
            };
            rank(a).total_cmp(&rank(b))
        })?;
    let curve = scores[best - 1] - 2.0 * scores[best] + scores[best + 1];
    let offset = if curve.abs() > 1e-12 {
        (0.5 * (scores[best - 1] - scores[best + 1]) / curve).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    Some((RATE * 60.0 / (best as f64 + offset)).clamp(50.0, 200.0))
}

pub(super) fn estimated_beats(bpm: f64, frames: usize, lane: &Lane) -> Option<u16> {
    let beats = (frames as f64 * (lane.end - lane.start) / 44100.0 * bpm / 60.0).round();
    (beats.is_finite() && (1.0..=256.0).contains(&beats)).then_some(beats as u16)
}

pub(super) fn apply(lane: &mut Lane, sources: &[f64]) -> usize {
    let mut count = 0;
    for &source in sources {
        let target = lane
            .warp_markers
            .target_for_source(source, lane.start, lane.end);
        if lane
            .warp_markers
            .insert(source, target, lane.start, lane.end)
            .is_some()
        {
            count += 1;
        }
    }
    if count > 0 {
        lane.warp = true;
        lane.warp_grid = true;
    }
    count
}

impl App {
    pub(super) fn sampler_detect(&mut self, ctx: &eframe::egui::Context, lane: usize) {
        if lane >= 8 || self.studio.busy() {
            return;
        }
        let Some(wave) = self.studio.waves[lane].clone() else {
            return;
        };
        let start = self.studio.settings.lanes[lane].start;
        let end = self.studio.settings.lanes[lane].end;
        let epoch = self.studio.waveform_epoch;
        self.studio.work(ctx, move || {
            let (sources, bpm) = detect(&wave, start, end);
            Ok(Completed::Transients(
                lane,
                Proposal {
                    epoch,
                    start,
                    end,
                    sources,
                    bpm,
                },
            ))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_tempo_buttons_only_change_playback_on_confirm_and_support_undo() {
        use eframe::egui::{self, Context};
        use std::sync::Arc;
        fn find(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == label => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(items) => items.iter().find_map(|s| find(s, label)),
                _ => None,
            }
        }
        fn frame(
            app: &mut App,
            ctx: &Context,
            events: Vec<egui::Event>,
            label: &str,
        ) -> Option<egui::Pos2> {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1400.0, 900.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.warp_editor(ui.ctx()),
            );
            out.textures_delta.clear();
            out.shapes.iter().find_map(|s| find(&s.shape, label))
        }
        fn click(app: &mut App, ctx: &Context, label: &str) {
            let pos = frame(app, ctx, vec![], label).expect("button must be visible");
            for pressed in [true, false] {
                frame(
                    app,
                    ctx,
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Default::default(),
                        },
                    ],
                    "",
                );
            }
            app.studio_poll(ctx);
        }
        let ctx = Context::default();
        let mut app = App::new(crate::model::Panel::new(), None);
        app.studio.ready = true;
        app.studio.modern = true;
        app.studio.warp_editor = Some(0);
        app.studio.waves[0] = Some(Arc::new(Wave {
            frames: vec![[0.1; 2]; 44100 * 4],
        }));
        app.studio.settings.lanes[0]
            .warp_markers
            .insert(0.5, 0.6, 0.0, 1.0)
            .unwrap();
        app.studio.proposals[0] = Some(Proposal {
            epoch: app.studio.waveform_epoch,
            start: 0.0,
            end: 1.0,
            sources: vec![],
            bpm: Some(120.0),
        });
        app.sampler_history_reset();
        let previous = app.studio.settings.clone();
        frame(&mut app, &ctx, vec![], "");
        click(&mut app, &ctx, "÷2");
        assert_eq!(app.studio.proposals[0].as_ref().unwrap().bpm, Some(60.0));
        assert_eq!(app.studio.settings, previous);
        click(&mut app, &ctx, "×2");
        click(&mut app, &ctx, "×2");
        assert_eq!(app.studio.proposals[0].as_ref().unwrap().bpm, Some(240.0));
        assert_eq!(app.studio.settings, previous);
        click(&mut app, &ctx, "USE ~16 BEATS");
        assert_eq!(app.studio.settings.lanes[0].warp_beats, 16);
        assert_eq!(
            app.studio.settings.lanes[0].warp_markers,
            previous.lanes[0].warp_markers
        );
        app.sampler_history_step(&ctx, false);
        assert_eq!(app.studio.settings, previous);
    }

    #[test]
    fn tempo_analysis_tracks_rhythmic_audio_and_rejects_unreliable_signals() {
        for bpm in [90.0, 120.0, 150.0] {
            let mut wave = Wave {
                frames: vec![[0.0; 2]; 44100 * 12],
            };
            let period = 44100.0 * 60.0 / bpm;
            for beat in 0..((wave.frames.len() as f64 / period) as usize) {
                let start = (beat as f64 * period) as usize;
                for i in 0..500 {
                    wave.frames[start + i] = [(1.0 - i as f32 / 500.0) * 0.8; 2];
                }
            }
            let (_, estimate) = detect(&wave, 0.1, 0.9);
            let estimate = estimate.expect("periodic attacks should yield a tempo");
            assert!(
                (estimate - bpm).abs() < 1.0,
                "expected {bpm}, got {estimate}"
            );
            let lane = Lane {
                start: 0.1,
                end: 0.9,
                ..Lane::default()
            };
            assert_eq!(
                estimated_beats(estimate, wave.frames.len(), &lane),
                Some((9.6 * bpm / 60.0).round() as u16)
            );
        }
        assert!(
            detect(
                &Wave {
                    frames: vec![[0.0; 2]; 44100 * 4]
                },
                0.0,
                1.0
            )
            .1
            .is_none()
        );
        assert!(estimate_bpm(&[1.0; 1000]).is_none());
        let mut seed = 17_u32;
        let noise: Vec<f64> = (0..4000)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                f64::from(seed) / f64::from(u32::MAX)
            })
            .collect();
        assert!(estimate_bpm(&noise).is_none());
    }

    #[test]
    fn onsets_are_bounded_ordered_and_detected_only_in_the_selected_region() {
        let mut wave = Wave {
            frames: vec![[0.0; 2]; 88200],
        };
        for n in [0, 22050, 44100, 66150] {
            for i in 0..400 {
                wave.frames[n + i] = [(1.0 - i as f32 / 400.0) * 0.8; 2];
            }
        }
        let points = detect(&wave, 0.2, 0.9).0;
        assert_eq!(points.len(), 3);
        for (point, expected) in points.iter().zip([0.25, 0.5, 0.75]) {
            assert!((*point - expected).abs() < 0.006);
        }
        assert!(
            detect(
                &Wave {
                    frames: vec![[0.0; 2]; 44100]
                },
                0.0,
                1.0
            )
            .0
            .is_empty()
        );
        let mut lane = Lane::default();
        lane.warp_markers.insert(0.4, 0.6, 0.0, 1.0).unwrap();
        let previous = lane.warp_markers.map(0.3, 0.0, 1.0).0;
        assert_eq!(apply(&mut lane, &points), 3);
        assert_eq!(lane.warp_markers.points().len(), 4);
        assert!((lane.warp_markers.map(0.3, 0.0, 1.0).0 - previous).abs() < 1e-12);
    }
}
