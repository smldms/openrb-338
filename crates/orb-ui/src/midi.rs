//! MIDI controller input, mapping and pickup, outside the audio callback.
//!
//! The first device backend is Windows WinMM (no extra driver or Rust dependency). Mapping
//! is platform independent and uses the same Panel events as mouse edits. Clock and live
//! instrument notes are deliberately not handled here.

use std::collections::HashMap;

use eframe::egui::{self, Context};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::behavior;
use crate::layout;
use crate::model::Panel;

pub mod preset;

#[cfg(windows)]
mod winmm;
#[cfg(windows)]
use winmm as backend;

#[cfg(not(windows))]
mod backend {
    use super::Port;

    pub struct Input;

    pub fn ports() -> Result<Vec<Port>, String> {
        Err("MIDI controller input is currently available on Windows only.".into())
    }

    impl Input {
        pub fn open(_id: u32) -> Result<Self, String> {
            Err("MIDI controller input is currently available on Windows only.".into())
        }

        pub fn drain(&self) -> Vec<u32> {
            Vec::new()
        }

        pub fn dropped(&self) -> usize {
            0
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Port {
    pub id: u32,
    pub name: String,
}

/// A complete MIDI Control Change, on a 1-based channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cc {
    pub channel: u8,
    pub controller: u8,
    pub value: u8,
}

impl Cc {
    /// WinMM's packed short message; reject malformed data and every non-CC status.
    pub fn decode(word: u32) -> Option<Self> {
        let [status, controller, value, _] = word.to_le_bytes();
        (status & 0xf0 == 0xb0 && controller < 128 && value < 128).then_some(Self {
            channel: (status & 0x0f) + 1,
            controller,
            value,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub channel: u8,
    pub controller: u8,
    pub target: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub enabled: bool,
    pub port: Option<String>,
    pub pickup: bool,
    pub bindings: Vec<Binding>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            port: None,
            pickup: true,
            bindings: Vec::new(),
        }
    }
}

impl Config {
    /// Keep valid assignments only. A source and a target each have one owner; last wins.
    pub fn sanitize(&mut self) {
        let bindings = std::mem::take(&mut self.bindings);
        for b in bindings {
            if (1..=16).contains(&b.channel) && b.controller < 128 && kind(&b.target).is_some() {
                self.assign(b);
            }
        }
        if self.port.as_ref().is_some_and(|p| p.trim().is_empty()) {
            self.port = None;
        }
    }

    pub fn assign(&mut self, b: Binding) {
        self.bindings.retain(|old| {
            old.target != b.target && (old.channel != b.channel || old.controller != b.controller)
        });
        self.bindings.push(b);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Knob,
    Fader,
    Switch,
    Button,
    Number,
}

/// These targets all have a working panel-to-player route, including the two effects selectors.
pub fn targets() -> Vec<(String, Kind)> {
    let mut out = Vec::new();
    for c in layout::controls() {
        let k = match c.kind.as_str() {
            "knob" => Kind::Knob,
            "fader" => Kind::Fader,
            "hswitch" | "vswitch" => Kind::Switch,
            "toggle" | "radio" | "trigger" | "grid"
                if c.section.starts_with("mixer")
                    || c.section.starts_with("pattern")
                    || matches!(
                        c.section.as_str(),
                        "master" | "delay" | "dist" | "pcf" | "comp"
                    )
                    || matches!(c.id.as_str(), "transport.play" | "transport.stop") =>
            {
                Kind::Button
            }
            _ => continue,
        };
        if !out.iter().any(|(id, _)| id == &c.id) {
            out.push((c.id, k));
        }
    }
    out.push(("delay.steps".into(), Kind::Number));
    out.push(("pcf.pattern".into(), Kind::Number));
    out
}

pub fn kind(id: &str) -> Option<Kind> {
    static KINDS: std::sync::OnceLock<HashMap<String, Kind>> = std::sync::OnceLock::new();
    KINDS
        .get_or_init(|| targets().into_iter().collect())
        .get(id)
        .copied()
}

pub fn label(id: &str) -> String {
    let (section, parameter) = id.split_once('.').unwrap_or((id, ""));
    let section = match section {
        "tb303_1" => "303 A",
        "tb303_2" => "303 B",
        "tr808" => "TR-808",
        "tr909" => "TR-909",
        "mixer1" => "Mixer / 303 A",
        "mixer2" => "Mixer / 303 B",
        "mixer3" => "Mixer / TR-808",
        "mixer4" => "Mixer / TR-909",
        "pcf" => "PCF",
        "comp" => "Compressor",
        "dist" => "Distortion",
        "delay" => "Delay",
        "master" => "Master",
        _ => section,
    };
    format!("{section} / {}", parameter.replace('_', " "))
}

#[derive(Default)]
struct Pickup {
    previous: Option<u8>,
    sent: Option<u8>,
    caught: bool,
}

impl Pickup {
    fn accepts(&mut self, value: u8, current: u8, enabled: bool) -> bool {
        // Mouse changes, song loads and automation re-arm pickup. Own changes do not.
        if self.sent != Some(current) {
            self.caught = false;
        }
        let crosses = self
            .previous
            .is_some_and(|p| p.min(value) <= current && current <= p.max(value));
        self.previous = Some(value);
        self.caught |= !enabled || value.abs_diff(current) <= 1 || crosses;
        self.caught
    }
}

/// Per-connection state, separate from saved assignments.
#[derive(Default)]
pub struct Router {
    pickup: HashMap<String, Pickup>,
    buttons: HashMap<(u8, u8), bool>,
}

impl Router {
    pub fn reset(&mut self) {
        self.pickup.clear();
        self.buttons.clear();
    }

    pub fn apply(&mut self, cc: Cc, config: &Config, panel: &mut Panel, now: f64) {
        let Some(b) = config
            .bindings
            .iter()
            .find(|b| b.channel == cc.channel && b.controller == cc.controller)
        else {
            return;
        };
        let Some(k) = kind(&b.target) else { return };
        let id = b.target.as_str();
        match k {
            Kind::Button => {
                let down = cc.value >= 64;
                let previous = self.buttons.insert((cc.channel, cc.controller), down);
                if down && previous != Some(true) {
                    panel.press(id, now);
                }
            }
            Kind::Switch => panel.set_switch(id, u8::from(cc.value >= 64)),
            Kind::Knob | Kind::Fader | Kind::Number => {
                let current = normalized(panel, id, k);
                let state = self.pickup.entry(b.target.clone()).or_default();
                if !state.accepts(cc.value, current, config.pickup) {
                    return;
                }
                let p = behavior::knob_from_engine(i32::from(cc.value));
                match k {
                    Kind::Knob => panel.set_knob(id, p),
                    Kind::Fader => panel.set_fader(id, p),
                    Kind::Number => panel.set_midi_number(id, cc.value),
                    _ => unreachable!(),
                }
                state.sent = Some(normalized(panel, id, k));
            }
        }
    }
}

fn normalized(panel: &Panel, id: &str, k: Kind) -> u8 {
    if k == Kind::Number {
        let v = panel.display(id).unwrap_or(0);
        return if id == "delay.steps" {
            ((v - 1).clamp(0, 31) * 127 / 31) as u8
        } else {
            (v.clamp(0, 54) * 127 / 54) as u8
        };
    }
    panel.value(id).unwrap_or(0).min(127)
}

/// Runtime owned by the app. Opening the window is also possible without any MIDI hardware.
#[derive(Default)]
pub struct Midi {
    pub open: bool,
    started: bool,
    ports: Vec<Port>,
    input: Option<backend::Input>,
    active: Option<String>,
    pub router: Router,
    learning: Option<String>,
    selected: String,
    filter: String,
    last: Option<Cc>,
    error: Option<String>,
    dropped_seen: usize,
    notice: Option<String>,
}

impl Midi {
    fn connect(&mut self, config: &Config) {
        self.input = None;
        self.active = None;
        self.router.reset();
        self.learning = None;
        self.error = None;
        self.dropped_seen = 0;
        if !config.enabled && !self.open {
            return;
        }
        match backend::ports() {
            Ok(ports) => self.ports = ports,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        }
        if !config.enabled {
            return;
        }
        let Some(name) = &config.port else { return };
        let matches: Vec<_> = self.ports.iter().filter(|p| &p.name == name).collect();
        if matches.len() != 1 {
            self.error = Some(if matches.is_empty() {
                format!("MIDI input '{name}' is unavailable. Connect it, then Refresh / reconnect.")
            } else {
                "Several MIDI inputs have the same name. Give the device a unique name.".into()
            });
            return;
        }
        match backend::Input::open(matches[0].id) {
            Ok(input) => {
                self.input = Some(input);
                self.active = Some(name.clone());
            }
            Err(e) => self.error = Some(e),
        }
    }
}

impl App {
    pub(crate) fn midi_tick(&mut self, ctx: &Context) {
        // App::new is also used by headless tests. Only an enabled preference opens a device.
        if !self.midi.started {
            self.midi.started = true;
            self.midi.connect(&self.settings.midi);
        }
        let Some(input) = &self.midi.input else {
            return;
        };
        let words = input.drain();
        let dropped = input.dropped();
        // WinMM callbacks only enqueue data; all GUI calls happen here on the UI thread.
        ctx.request_repaint_after(std::time::Duration::from_millis(8));
        if dropped > self.midi.dropped_seen {
            self.midi.dropped_seen = dropped;
            self.midi.error = Some(format!(
                "MIDI input overflow: {dropped} messages dropped. Reconnect to reset the counter."
            ));
            self.midi.router.reset();
        }
        let now = ctx.input(|i| i.time);
        let mut learned = false;
        for cc in words.into_iter().filter_map(Cc::decode) {
            self.midi.last = Some(cc);
            if let Some(target) = self.midi.learning.take() {
                self.settings.midi.assign(Binding {
                    channel: cc.channel,
                    controller: cc.controller,
                    target,
                });
                self.midi.router.reset();
                learned = true;
                // The learning movement only assigns; it never jumps the target's value.
                continue;
            }
            self.midi
                .router
                .apply(cc, &self.settings.midi, &mut self.panel, now);
        }
        if learned {
            self.save_midi();
        }
    }

    fn save_midi(&mut self) {
        if let Err(e) = self.settings.save() {
            self.midi.error = Some(format!("Cannot save MIDI settings: {e}"));
        }
    }

    pub(crate) fn midi_ui(&mut self, ctx: &Context) {
        if !self.midi.open {
            return;
        }
        let mut open = true;
        let mut changed = false;
        let mut reconnect = false;
        let mut remove = None;
        let mut import = false;
        let mut export = false;
        egui::Window::new("MIDI Controller")
            .open(&mut open)
            .default_width(620.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label("UC-33e / MIDI CC control - Windows");
                ui.horizontal(|ui| {
                    import = ui.button("Import routing...").clicked();
                    export = ui.button("Export routing...").clicked();
                });
                ui.small(
                    "Portable file: assignments + soft takeover. Import replaces assignments.",
                );
                ui.small("Choose the MIDI input locally on each computer.");
                if let Some(notice) = &self.midi.notice {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, notice);
                }
                if self.midi.ports.is_empty() && self.midi.error.is_none() {
                    match backend::ports() {
                        Ok(p) => self.midi.ports = p,
                        Err(e) => self.midi.error = Some(e),
                    }
                }
                let config = &mut self.settings.midi;
                ui.horizontal(|ui| {
                    if ui
                        .checkbox(&mut config.enabled, "Enable MIDI input")
                        .changed()
                    {
                        changed = true;
                        reconnect = true;
                    }
                    if ui.button("Refresh / reconnect").clicked() {
                        reconnect = true;
                    }
                });
                egui::ComboBox::from_id_salt("midi-port")
                    .selected_text(config.port.as_deref().unwrap_or("Choose MIDI input..."))
                    .show_ui(ui, |ui| {
                        for p in &self.midi.ports {
                            if ui
                                .selectable_value(&mut config.port, Some(p.name.clone()), &p.name)
                                .changed()
                            {
                                changed = true;
                                reconnect = true;
                            }
                        }
                    });
                if let Some(name) = &self.midi.active {
                    ui.colored_label(egui::Color32::LIGHT_GREEN, format!("Connected: {name}"));
                } else {
                    ui.label("Disconnected");
                }
                if ui
                    .checkbox(&mut config.pickup, "Soft takeover (prevent value jumps)")
                    .changed()
                {
                    self.midi.router.reset();
                    changed = true;
                }
                if let Some(cc) = self.midi.last {
                    ui.label(format!(
                        "Last input: channel {} / CC {} / value {}",
                        cc.channel, cc.controller, cc.value
                    ));
                }
                if let Some(error) = &self.midi.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
                ui.separator();
                ui.label("Choose a parameter, click Learn, then move a controller knob or fader.");
                ui.label("Buttons use a CC value of 64-127 to press, 0-63 to release.");
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.text_edit_singleline(&mut self.midi.filter);
                });
                let filter = self.midi.filter.to_lowercase();
                let targets: Vec<_> = targets()
                    .into_iter()
                    .filter(|(id, _)| {
                        label(id).to_lowercase().contains(&filter) || id.contains(&filter)
                    })
                    .collect();
                egui::ComboBox::from_id_salt("midi-target")
                    .width(370.0)
                    .selected_text(if self.midi.selected.is_empty() {
                        "Choose a parameter...".into()
                    } else {
                        label(&self.midi.selected)
                    })
                    .show_ui(ui, |ui| {
                        for (id, _) in &targets {
                            ui.selectable_value(&mut self.midi.selected, id.clone(), label(id));
                        }
                    });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.midi.input.is_some() && !self.midi.selected.is_empty(),
                            egui::Button::new("Learn"),
                        )
                        .clicked()
                    {
                        self.midi.learning = Some(self.midi.selected.clone());
                    }
                    if self.midi.learning.is_some() && ui.button("Cancel learn").clicked() {
                        self.midi.learning = None;
                    }
                });
                if let Some(id) = &self.midi.learning {
                    ui.colored_label(
                        egui::Color32::YELLOW,
                        format!("Waiting for CC: {}", label(id)),
                    );
                }
                ui.separator();
                ui.label("Assignments (saved automatically)");
                egui::ScrollArea::vertical()
                    .max_height(240.0)
                    .show(ui, |ui| {
                        for (i, b) in config.bindings.iter().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label(format!("Ch {} / CC {}", b.channel, b.controller));
                                ui.label(label(&b.target));
                                if ui.small_button("Remove").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                    });
                ui.separator();
                ui.label(
                    "Suggested faders 1-4: four mixer levels; 5-8: four delay sends; 9: master.",
                );
                ui.label("No factory CC preset is assumed: MIDI Learn follows your UC-33e setup.");
            });
        self.midi.open = open;
        if !open {
            self.midi.learning = None;
        }
        if let Some(i) = remove {
            self.settings.midi.bindings.remove(i);
            self.midi.router.reset();
            changed = true;
        }
        if reconnect {
            self.midi.connect(&self.settings.midi);
        }
        if changed {
            self.save_midi();
        }
        if import {
            self.import_midi_routing();
            ctx.request_repaint();
        }
        if export {
            self.export_midi_routing();
            ctx.request_repaint();
        }
    }

    fn import_midi_routing(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import OpenRB MIDI routing")
            .add_filter("OpenRB MIDI routing", &["toml"])
            .set_directory(crate::settings::documents_dir())
            .pick_file()
        else {
            return;
        };
        self.midi.notice = None;
        let result =
            preset::RoutingPreset::read(&path).and_then(|p| p.apply(&mut self.settings.midi));
        match result {
            Ok(count) => {
                self.midi.router.reset();
                self.midi.learning = None;
                self.midi.error = None;
                match self.settings.save() {
                    Ok(()) => self.midi.notice = Some(format!("Imported {count} assignments.")),
                    Err(e) => {
                        self.midi.error = Some(format!("Imported, but cannot save settings: {e}"))
                    }
                }
            }
            Err(e) => self.midi.error = Some(e),
        }
    }

    fn export_midi_routing(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Export OpenRB MIDI routing")
            .add_filter("OpenRB MIDI routing", &["toml"])
            .set_directory(crate::settings::documents_dir())
            .set_file_name("UC33-OpenRB-routing.toml")
            .save_file()
        else {
            return;
        };
        self.midi.notice = None;
        match preset::RoutingPreset::from_config(&self.settings.midi).write(&path) {
            Ok(()) => {
                self.midi.error = None;
                self.midi.notice = Some(format!(
                    "Exported {} assignments.",
                    self.settings.midi.bindings.len()
                ));
            }
            Err(e) => self.midi.error = Some(e),
        }
    }
}
