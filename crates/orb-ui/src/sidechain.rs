//! Additional mix settings, persisted locally and included in audio exports.

use eframe::egui::{self, Align2, Color32, Context, FontId, Sense, Ui};
use orb_engine::sidechain::{Config, Source};
use orb_player::Command;
use serde::{Deserialize, Serialize};

use crate::app::{App, View};
use crate::midi::MidiMenu;
use crate::model::Event;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: bool,
    pub source: u8,
    pub instrument: u8,
    pub depth: [u8; 2],
    pub release_ms: u16,
    pub bypass: [bool; 2],
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            source: 2,
            instrument: 1,
            depth: [55, 55],
            release_ms: 180,
            bypass: [false; 2],
        }
    }
}

impl Settings {
    pub(crate) fn set_tb_enabled(&mut self, row: usize, on: bool) {
        if on && !self.enabled {
            self.bypass = [true; 2];
            self.enabled = true;
        }
        self.bypass[row] = !on;
    }

    pub fn sanitize(&mut self) {
        self.source = self.source.min(2);
        self.instrument = self.instrument.clamp(1, 11);
        self.depth = self.depth.map(|v| v.min(100));
        self.release_ms = self.release_ms.clamp(20, 1000);
    }

    pub fn engine_config(&self) -> Config {
        Config {
            enabled: self.enabled,
            instrument: self.instrument,
            source: match self.source {
                0 => Source::Tr808,
                1 => Source::Tr909,
                _ => Source::Both,
            },
            depth: std::array::from_fn(|i| if self.bypass[i] { 0 } else { self.depth[i] }),
            release_ms: self.release_ms,
        }
        .sanitized()
    }
}

pub struct SidechainUi {
    pub open: bool,
    sent: Option<Config>,
    hat_sent: Option<Option<u8>>,
    save_at: Option<f64>,
}

fn instrument_selector(ui: &mut Ui, settings: &mut Settings) {
    let labels = match settings.source {
        0 => [
            "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB", "CY", "OH", "CH",
        ],
        1 => [
            "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CH", "OH", "CC", "RC",
        ],
        _ => [
            "BD", "SD", "LT", "MT", "HT", "RS", "CP", "CB/CH", "CY/OH", "OH/CC", "CH/RC",
        ],
    };
    egui::ComboBox::from_id_salt("sidechain-instrument")
        .width(48.0)
        .selected_text(labels[usize::from(settings.instrument.clamp(1, 11) - 1)])
        .show_ui(ui, |ui| {
            for (i, label) in labels.into_iter().enumerate() {
                ui.selectable_value(&mut settings.instrument, i as u8 + 1, label);
            }
        })
        .response
        .on_hover_text("Trigger voice. BOTH labels show 808 / 909 native columns.");
}

impl Default for SidechainUi {
    fn default() -> Self {
        Self {
            open: false,
            // A fresh player already has these defaults. Avoid sending no-op commands
            // that would delay the first legacy snapshot behind a command barrier.
            sent: Some(Config::default()),
            hat_sent: Some(None),
            save_at: None,
        }
    }
}

impl App {
    pub(crate) fn sidechain_changed(&mut self, ctx: &Context) {
        if self.studio.content {
            self.doc.modified = true;
        }
        self.settings.sidechain.sanitize();
        self.sidechain.save_at = Some(ctx.input(|i| i.time) + 0.35);
        self.sync_effects(ctx);
        ctx.request_repaint();
    }

    pub(crate) fn graphite_sidechain(&mut self, ui: &mut Ui, rect: egui::Rect, row: usize) {
        let before = self.settings.sidechain.clone();
        ui.scope_builder(
            egui::UiBuilder::new()
                .max_rect(rect)
                .id_salt(("tb-duck", row)),
            |ui| {
                ui.set_clip_rect(ui.clip_rect().intersect(rect));
                ui.style_mut().override_font_id = Some(FontId::proportional(10.5));
                ui.visuals_mut().selection.bg_fill = Color32::from_rgb(67, 78, 46);
                ui.spacing_mut().button_padding = egui::vec2(3.0, 1.0);
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 2.0);
                ui.spacing_mut().interact_size.y = 14.0;
                let s = &mut self.settings.sidechain;
                ui.horizontal(|ui| {
                    let on = s.enabled && !s.bypass[row];
                    if ui
                        .selectable_label(on, "SIDECHAIN")
                        .midi_menu(ui, &format!("duck{}.on", row + 1))
                        .clicked()
                    {
                        s.set_tb_enabled(row, !on);
                    }
                    ui.add(
                        crate::midi::drag_value(ui, &mut s.depth[row])
                            .range(0..=100)
                            .suffix("%"),
                    )
                    .midi_menu(ui, &format!("duck{}.depth", row + 1))
                    .on_hover_text("Depth for this TB only; bypass retains this value");
                    if ui.small_button("...").clicked() {
                        self.sidechain.open = true;
                    }
                });
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("kick-source")
                        .width(48.0)
                        .selected_text(["808", "909", "BOTH"][usize::from(s.source.min(2))])
                        .show_ui(ui, |ui| {
                            for (value, label) in [(0, "808"), (1, "909"), (2, "BOTH")] {
                                ui.selectable_value(&mut s.source, value, label);
                            }
                        });
                    instrument_selector(ui, s);
                    ui.add(
                        crate::midi::drag_value(ui, &mut s.release_ms)
                            .range(20..=1000)
                            .suffix("ms"),
                    )
                    .midi_menu(ui, "duck.release")
                    .on_hover_text(
                        "Trigger voice and return time are shared by both TBs and sampler",
                    );
                });
            },
        );
        if self.settings.sidechain != before {
            self.sidechain_changed(ui.ctx());
        }
    }

    /// CH edits use the normal mouse/MIDI panel route; these extensions are sent through
    /// the same ordered queue and snapshot barrier as the original controls.
    pub fn sync_effects(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        let level = self
            .panel
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                Event::Set { id, value } if id == "tr909.ch_level" => {
                    Some((*value).clamp(0, 127) as u8)
                }
                _ => None,
            });
        // Editing either hat opts into independence. Until then, legacy songs retain
        // their original shared-level behavior, including existing automation.
        let level =
            level.or_else(|| {
                (self.settings.closed_hat_level.is_none()
                    && self.panel.events.iter().any(
                        |event| matches!(event, Event::Set { id, .. } if id == "tr909.hh_level"),
                    ))
                .then(|| self.panel.value("tr909.ch_level").unwrap_or(100))
            });
        if let Some(level) = level
            && self.settings.closed_hat_level != Some(level)
        {
            self.settings.closed_hat_level = Some(level);
            if self.studio.content {
                self.doc.modified = true;
            }
            self.sidechain.save_at = Some(now + 0.35);
        }
        if let Some(link) = &mut self.link {
            let config = self.settings.sidechain.engine_config();
            if self.sidechain.sent != Some(config) {
                link.effect_command(Command::Sidechain(config));
                self.sidechain.sent = Some(config);
            }
            if self.sidechain.hat_sent != Some(self.settings.closed_hat_level) {
                link.effect_command(Command::ClosedHatLevel(self.settings.closed_hat_level));
                self.sidechain.hat_sent = Some(self.settings.closed_hat_level);
            }
        }
        if let Some(level) = self.settings.closed_hat_level {
            self.panel.adopt("tr909.ch_level", i32::from(level));
        }
        if let Some(at) = self.sidechain.save_at {
            if (now >= at && !ctx.input(|i| i.pointer.any_down()))
                || ctx.input(|i| i.viewport().close_requested())
            {
                self.sidechain.save_at = None;
                if let Err(e) = self.settings.save() {
                    self.fail(format!("Cannot save mix settings: {e}"));
                }
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(40));
            }
        }
    }

    pub(crate) fn sidechain_button(&mut self, ui: &mut Ui, view: &View) {
        if self.browsing() || self.prompting() || self.error_open() {
            return;
        }
        let rect = view.rect(454.0, 28.0, 18.0, 14.0);
        let response = ui.interact(rect, egui::Id::unique("kick-sidechain"), Sense::click());
        let fill = if self.settings.sidechain.enabled {
            Color32::from_rgb(0x4a, 0x53, 0x32)
        } else {
            Color32::from_rgb(0x30, 0x30, 0x34)
        };
        ui.painter().rect_filled(rect, 2.0 * view.scale, fill);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "SC",
            FontId::proportional(6.5 * view.scale),
            crate::palette::INK,
        );
        if response
            .on_hover_text("TR sidechain: make room for the selected 808/909 voice")
            .clicked()
        {
            self.sidechain.open = true;
        }
    }

    pub(crate) fn sidechain_ui(&mut self, ctx: &Context) {
        if !self.sidechain.open {
            return;
        }
        let reduction = self
            .link
            .as_mut()
            .map(|l| l.player().snapshot().sidechain_reduction)
            .unwrap_or([0; 2]);
        let mut open = true;
        let mut changed = false;
        egui::Window::new("TR Sidechain")
            .open(&mut open)
            .default_width(420.0)
            .show(ctx, |ui| {
                let s = &mut self.settings.sidechain;
                changed |= ui.checkbox(&mut s.enabled, "Enable ducking").changed();
                ui.label("Each selected TR hit briefly lowers the TBs.");
                ui.horizontal(|ui| {
                    ui.label("Trigger");
                    for (source, label) in [(0, "TR-808"), (1, "TR-909"), (2, "Both")] {
                        changed |= ui.selectable_value(&mut s.source, source, label).changed();
                    }
                    let before = s.instrument;
                    instrument_selector(ui, s);
                    changed |= before != s.instrument;
                });
                for (row, label) in ["TB-303 A", "TB-303 B"].into_iter().enumerate() {
                    let mut active = !s.bypass[row];
                    if ui
                        .checkbox(&mut active, format!("{label} active"))
                        .changed()
                    {
                        s.bypass[row] = !active;
                        changed = true;
                    }
                    changed |= ui
                        .add(
                            crate::midi::slider(ui, &mut s.depth[row], 0..=100)
                                .text(format!("{label} depth"))
                                .suffix(" %"),
                        )
                        .changed();
                    ui.add(
                        egui::ProgressBar::new(f32::from(reduction[row]) / 100.0)
                            .text(format!("{label}: -{} %", reduction[row])),
                    );
                }
                changed |= ui
                    .add(
                        crate::midi::slider(ui, &mut s.release_ms, 20..=1000)
                            .text("Return")
                            .suffix(" ms"),
                    )
                    .changed();
                ui.small("Depth 0 leaves that TB unchanged. A smoothed attack avoids clicks.");
                ui.separator();
                ui.small("Saved with this app and included in audio exports.");
                ui.small("Standard RBS files do not store these additional mix settings.");
            });
        self.sidechain.open = open;
        if changed {
            if self.studio.content {
                self.doc.modified = true;
            }
            self.settings.sidechain.sanitize();
            self.sidechain.save_at = Some(ctx.input(|i| i.time) + 0.35);
            self.sync_effects(ctx);
            ctx.request_repaint();
        }
        if self.settings.sidechain.enabled {
            ctx.request_repaint_after(crate::app::METER_FRAME);
        }
    }
}
