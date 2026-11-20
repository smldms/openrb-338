//! Sampler-only history. PCM is shared by Arc; restoration copies runtime buffers off-thread.

use std::collections::HashSet;
use std::sync::Arc;

use eframe::egui::{self, Context, Ui};
use orb_engine::studio::{
    SamplerBusSettings,
    audio_file::{MAX_AUDIO_BYTES, Wave},
    sampler::Lane,
};
use orb_player::Command;

use super::{ASSET_EPOCH, Completed, StudioUi};
use crate::app::App;

#[derive(Clone)]
pub(super) struct SamplerState {
    pub lanes: [Lane; 8],
    pub bus: SamplerBusSettings,
    pub names: [String; 8],
    pub waves: [Option<Arc<Wave>>; 8],
}

impl SamplerState {
    pub fn capture(ui: &StudioUi) -> Self {
        Self {
            lanes: ui.settings.lanes.clone(),
            bus: ui.settings.sampler_bus,
            names: ui.names.clone(),
            waves: ui.waves.clone(),
        }
    }
}

impl PartialEq for SamplerState {
    fn eq(&self, other: &Self) -> bool {
        self.lanes == other.lanes
            && self.bus == other.bus
            && self.names == other.names
            && self
                .waves
                .iter()
                .zip(&other.waves)
                .all(|(a, b)| same_wave(a, b))
    }
}

fn same_wave(a: &Option<Arc<Wave>>, b: &Option<Arc<Wave>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

#[derive(Clone, Default)]
pub(super) struct History {
    current: Option<SamplerState>,
    undo: Vec<SamplerState>,
    redo: Vec<SamplerState>,
    gesture: bool,
}

impl History {
    pub fn reset(&mut self, current: SamplerState) {
        *self = Self {
            current: Some(current),
            ..Self::default()
        };
    }

    pub fn observe(&mut self, state: SamplerState, dragging: bool) {
        if self.current.as_ref() != Some(&state) {
            if !self.gesture
                && let Some(previous) = self.current.take()
            {
                self.undo.push(previous);
            }
            self.redo.clear();
            self.current = Some(state);
            self.gesture = dragging;
            self.trim();
        }
        if !dragging {
            self.gesture = false;
        }
    }

    fn bytes(&self) -> usize {
        let mut seen = HashSet::new();
        self.current
            .iter()
            .chain(&self.undo)
            .chain(&self.redo)
            .flat_map(|s| s.waves.iter().flatten())
            .filter(|w| seen.insert(Arc::as_ptr(w) as usize))
            .map(|w| w.frames.len() * 8)
            .sum()
    }

    fn trim(&mut self) {
        while self.undo.len() + self.redo.len() > 32 || self.bytes() > MAX_AUDIO_BYTES * 2 {
            if !self.undo.is_empty() {
                self.undo.remove(0);
            } else if !self.redo.is_empty() {
                self.redo.remove(0);
            } else {
                break;
            }
        }
    }

    fn step(&mut self, redo: bool) -> Option<SamplerState> {
        let next = if redo {
            self.redo.pop()?
        } else {
            self.undo.pop()?
        };
        if let Some(current) = self.current.replace(next.clone()) {
            if redo {
                self.undo.push(current);
            } else {
                self.redo.push(current);
            }
        }
        self.gesture = false;
        Some(next)
    }
}

impl App {
    pub(super) fn sampler_history_reset(&mut self) {
        self.studio
            .history
            .reset(SamplerState::capture(&self.studio));
    }

    pub(super) fn sampler_history_poll(&mut self, ctx: &Context) {
        if !self.studio.history_restoring {
            let state = SamplerState::capture(&self.studio);
            self.studio
                .history
                .observe(state, ctx.input(|i| i.pointer.any_down()));
        }
    }

    pub(super) fn sampler_history_buttons(&mut self, ui: &mut Ui) {
        let ready = self.studio.ready && !self.studio.busy();
        if ui
            .add_enabled(
                ready && !self.studio.history.undo.is_empty(),
                egui::Button::new("UNDO"),
            )
            .on_hover_text("Undo sampler / WARP edit (Ctrl/Cmd+Z)")
            .clicked()
        {
            self.sampler_history_step(ui.ctx(), false);
        }
        if ui
            .add_enabled(
                ready && !self.studio.history.redo.is_empty(),
                egui::Button::new("REDO"),
            )
            .on_hover_text("Redo sampler / WARP edit (Ctrl/Cmd+Shift+Z or Ctrl+Y)")
            .clicked()
        {
            self.sampler_history_step(ui.ctx(), true);
        }
    }

    pub(super) fn sampler_history_keys(&mut self, ctx: &Context) {
        if ctx.text_edit_focused()
            || self.browsing()
            || self.prompting()
            || self.error_open()
            || !self.studio.modern
        {
            return;
        }
        let redo = ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::Z,
            ) || i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y)
        });
        let undo =
            !redo && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z));
        if undo || redo {
            self.sampler_history_step(ctx, redo);
        }
    }

    pub(super) fn sampler_history_step(&mut self, ctx: &Context, redo: bool) {
        if self.studio.busy() || !self.studio.ready {
            return;
        }
        self.sampler_history_poll(ctx);
        let backup = self.studio.history.clone();
        let Some(state) = self.studio.history.step(redo) else {
            return;
        };
        let changed = std::array::from_fn(|i| !same_wave(&state.waves[i], &self.studio.waves[i]));
        if changed.iter().any(|&v| v) {
            self.studio.history_backup = Some(backup);
            self.studio.history_restoring = true;
            self.studio.work(ctx, move || {
                let runtime = std::array::from_fn(|i| {
                    if changed[i] {
                        state.waves[i].as_ref().map(|w| {
                            Box::new(Wave {
                                frames: w.frames.clone(),
                            })
                        })
                    } else {
                        None
                    }
                });
                Ok(Completed::History(Box::new(state), runtime, changed))
            });
        } else {
            self.sampler_history_apply(state, std::array::from_fn(|_| None), changed);
        }
    }

    pub(super) fn sampler_history_apply(
        &mut self,
        state: SamplerState,
        runtime: [Option<Box<Wave>>; 8],
        changed: [bool; 8],
    ) {
        self.studio.settings.lanes = state.lanes;
        self.studio.settings.sampler_bus = state.bus;
        self.studio.names = state.names;
        self.studio.waves = state.waves;
        if changed.iter().any(|&v| v) {
            self.studio.waveform_epoch =
                ASSET_EPOCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.studio.proposals = std::array::from_fn(|_| None);
        }
        for (lane, wave) in runtime.into_iter().enumerate() {
            if changed[lane] {
                self.panel.sampler_playheads[lane] = None;
                self.studio_command(Command::StudioWave { lane, wave });
            }
        }
        self.studio.changed();
        self.doc.modified = true;
        self.studio_mirror();
    }

    pub(super) fn sampler_unload(&mut self, lane: usize) {
        if lane >= 8 || self.studio.busy() || self.studio.waves[lane].is_none() {
            return;
        }
        self.sampler_history_poll(&self.studio.context.clone());
        self.studio.waves[lane] = None;
        self.studio.names[lane] = format!("Sample {:02}", lane + 1);
        self.studio.proposals[lane] = None;
        self.panel.sampler_playheads[lane] = None;
        self.studio.changed();
        self.doc.modified = true;
        self.studio_command(Command::StudioWave { lane, wave: None });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warp_undo_button_preserves_stored_bpm_redo_and_view_epoch() {
        fn button(shape: &egui::Shape) -> Option<egui::Pos2> {
            match shape {
                egui::Shape::Text(t) if t.galley.text() == "UNDO" => {
                    Some(t.pos + t.galley.size() * 0.5)
                }
                egui::Shape::Vec(items) => items.iter().find_map(button),
                _ => None,
            }
        }
        let ctx = Context::default();
        let mut app = App::new(crate::model::Panel::new(), None);
        app.studio.ready = true;
        app.studio.modern = true;
        app.studio.warp_editor = Some(0);
        app.studio.waves[0] = Some(Arc::new(Wave {
            frames: vec![[0.1; 2]; 88200],
        }));
        app.studio.settings.lanes[0].warp = true;
        app.studio.settings.lanes[0].warp_grid = true;
        app.studio.settings.lanes[0].warp_bpm = 111.0;
        app.sampler_history_reset();
        let epoch = app.studio.waveform_epoch;
        app.studio.settings.lanes[0]
            .warp_markers
            .insert(0.5, 0.6, 0.0, 1.0)
            .unwrap();
        app.sampler_history_poll(&ctx);
        let mut frame = |events| {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1200.0, 800.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| app.warp_editor(ui.ctx()),
            );
            out.textures_delta.clear();
            app.studio_poll(&ctx);
            out.shapes.iter().find_map(|s| button(&s.shape))
        };
        let _ = frame(vec![]);
        let pos = frame(vec![]).expect("Undo button");
        for pressed in [true, false] {
            frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
        assert!(
            app.studio.settings.lanes[0]
                .warp_markers
                .points()
                .is_empty()
        );
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 111.0);
        assert_eq!(app.studio.waveform_epoch, epoch);
        assert_eq!(app.studio.history.redo.len(), 1);
    }

    #[test]
    fn undo_shortcut_works_with_waveform_focus_but_leaves_text_editing_alone() {
        let ctx = Context::default();
        let mut app = App::new(crate::model::Panel::new(), None);
        app.studio.ready = true;
        app.studio.modern = true;
        app.studio.settings.lanes[0].pitch = 7;
        app.sampler_history_poll(&ctx);
        let key = || egui::Event::Key {
            key: egui::Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        };
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![key()],
                ..Default::default()
            },
            |ui| {
                let (_, response) =
                    ui.allocate_exact_size(egui::vec2(100.0, 30.0), egui::Sense::click());
                response.request_focus();
                app.sampler_history_keys(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.studio.settings.lanes[0].pitch, 0);
        app.studio.settings.lanes[0].pitch = 12;
        app.sampler_history_poll(&ctx);
        let mut text = String::from("sample name");
        let mut output = ctx.run_ui(
            egui::RawInput {
                events: vec![key()],
                ..Default::default()
            },
            |ui| {
                ui.text_edit_singleline(&mut text).request_focus();
                app.sampler_history_keys(ui.ctx());
            },
        );
        output.textures_delta.clear();
        assert_eq!(app.studio.settings.lanes[0].pitch, 12);
    }

    #[test]
    fn drags_coalesce_redo_is_invalidated_and_assets_are_shared_and_bounded() {
        let mut ui = StudioUi::default();
        let original = SamplerState::capture(&ui);
        for i in 0..100 {
            ui.settings.lanes[0].gain = i as f64 / 100.0;
            ui.history.observe(SamplerState::capture(&ui), true);
        }
        ui.history.observe(SamplerState::capture(&ui), false);
        assert_eq!(ui.history.undo.len(), 1);
        assert!(ui.history.step(false).as_ref() == Some(&original));
        assert!(ui.history.step(true).unwrap().lanes[0].gain > 0.98);
        let _ = ui.history.step(false);
        ui.waves[0] = Some(Arc::new(Wave {
            frames: vec![[0.1; 2]; 1000],
        }));
        ui.history.observe(SamplerState::capture(&ui), false);
        assert!(ui.history.redo.is_empty());
        for i in 0..50 {
            ui.settings.lanes[0].gain = i as f64 / 50.0;
            ui.history.observe(SamplerState::capture(&ui), false);
        }
        assert!(ui.history.undo.len() <= 32);
        assert_eq!(ui.history.bytes(), 8000);
    }
}
