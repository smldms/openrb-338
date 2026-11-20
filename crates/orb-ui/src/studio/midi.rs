//! Studio controls share MIDI pickup and per-device calibration.
use crate::{app::App, model::Event};
use orb_engine::studio::{
    Settings,
    sampler::{Lane, Mode},
};
use orb_player::Command;

pub fn controls() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, label) in [
        ("enabled", "Enabled"),
        ("return", "Return"),
        ("predelay", "Pre-delay"),
        ("width", "Stereo width"),
        ("low_cut", "Low cut"),
        ("high_cut", "High cut"),
        ("trim808", "TR-808 gain trim"),
        ("trim909", "TR-909 gain trim"),
    ] {
        out.push((format!("studio.{name}"), format!("SPACE / {label}")));
    }
    for (i, label) in ["TB-303 A", "TB-303 B", "TR-808", "TR-909"]
        .iter()
        .enumerate()
    {
        out.push((
            format!("studio.send{}", i + 1),
            format!("SPACE / {label} send"),
        ));
    }
    for (id, label) in [
        ("duck1.on", "TB-303 A / Sidechain enabled"),
        ("duck1.depth", "TB-303 A / Sidechain depth"),
        ("duck2.on", "TB-303 B / Sidechain enabled"),
        ("duck2.depth", "TB-303 B / Sidechain depth"),
        ("duck.release", "Sidechain / Release"),
        ("duck.source", "Sidechain / Kick source"),
        ("duck.instrument", "Sidechain / TR instrument"),
    ] {
        out.push((id.into(), label.into()));
    }
    for (name, label) in [
        ("on", "On / Off"),
        ("level", "Mix"),
        ("pan", "Global pan"),
        ("dist", "Distortion routing"),
        ("delay", "Delay send"),
        ("send", "Reverb send"),
    ] {
        out.push((format!("sampler.{name}"), format!("Sampler / {label}")));
    }
    for i in 1..=8 {
        for name in [
            "level",
            "pan",
            "pitch",
            "send",
            "start",
            "end",
            "attack",
            "release",
            "gate",
            "mode",
            "reverse",
            "warp",
            "warp_bpm",
            "warp_beats",
            "warp_fit",
            "mute",
            "solo",
            "audition",
            "choke",
            "gater_source",
            "gater_instrument",
            "gater_hold",
            "gater_depth",
            "gater_attack_ms",
            "gater_release_ms",
            "sidechain",
        ] {
            out.push((
                format!("sampler{i}.{name}"),
                format!("Sampler / Lane {i} / {}", name.replace('_', " ")),
            ));
        }
    }
    out
}

pub fn is_action(id: &str) -> bool {
    id.starts_with("sampler") && (id.ends_with(".audition") || id.ends_with(".warp_fit"))
}

pub fn is_boolean(id: &str) -> bool {
    id == "studio.enabled"
        || matches!(id, "duck1.on" | "duck2.on" | "sampler.dist" | "sampler.on")
        || id.starts_with("sampler")
            && [".reverse", ".mute", ".solo", ".sidechain", ".warp"]
                .iter()
                .any(|s| id.ends_with(s))
}

fn duck_value(settings: &crate::sidechain::Settings, id: &str) -> Option<f64> {
    Some(match id {
        "duck1.on" => f64::from(u8::from(settings.enabled && !settings.bypass[0])),
        "duck2.on" => f64::from(u8::from(settings.enabled && !settings.bypass[1])),
        "duck1.depth" => f64::from(settings.depth[0]) / 100.0,
        "duck2.depth" => f64::from(settings.depth[1]) / 100.0,
        "duck.release" => (f64::from(settings.release_ms) - 20.0) / 980.0,
        "duck.source" => f64::from(settings.source) / 2.0,
        "duck.instrument" => f64::from(settings.instrument.saturating_sub(1)) / 10.0,
        _ => return None,
    })
}

fn lane_field(id: &str) -> Option<(usize, &str)> {
    let (lane, name) = id.strip_prefix("sampler")?.split_once('.')?;
    let i = lane.parse::<usize>().ok()?.checked_sub(1)?;
    (i < 8).then_some((i, name))
}

fn mode(v: f64) -> Mode {
    match (v * 3.0).round() as u8 {
        0 => Mode::OneShot,
        1 => Mode::AutoCut,
        2 => Mode::Gate,
        _ => Mode::Loop,
    }
}

fn gater_instrument(value: f64) -> u8 {
    // Accent (index 0) is not a playable voice on either drum machine.
    1 + (value * 10.0).round() as u8
}

const WARP_BEATS: [u16; 16] = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64, 96, 128, 192, 256];

fn warp_beat_count(value: f64) -> u16 {
    WARP_BEATS[((value * 15.0).round() as usize).min(15)]
}

fn lane_value(lane: &Lane, name: &str) -> f64 {
    match name {
        "level" => lane.gain / 2.0,
        "pan" => (lane.pan + 1.0) / 2.0,
        "pitch" => (f64::from(lane.pitch) + 24.0) / 48.0,
        "send" => lane.send,
        "start" => lane.start / (1.0 - 1e-9),
        "end" => (lane.end - 1e-9) / (1.0 - 1e-9),
        "attack" => (lane.attack_ms - 0.1) / 1999.9,
        "release" => (lane.release_ms - 1.0) / 2999.0,
        "gate" => (lane.gate - 0.01) / 15.99,
        "mode" => match lane.mode {
            Mode::OneShot => 0.0,
            Mode::AutoCut => 1.0 / 3.0,
            Mode::Gate => 2.0 / 3.0,
            Mode::Loop => 1.0,
        },
        "choke" => f64::from(lane.choke) / 8.0,
        "gater_source" => f64::from(lane.gater_source) / 4.0,
        "gater_instrument" => f64::from(lane.gater_instrument.saturating_sub(1)) / 10.0,
        "gater_hold" => (lane.gater_hold - 0.125) / 15.875,
        "gater_depth" => lane.gater_depth,
        "gater_attack_ms" => (lane.gater_attack_ms - 0.1) / 99.9,
        "gater_release_ms" => (lane.gater_release_ms - 1.0) / 499.0,
        "reverse" => f64::from(u8::from(lane.reverse)),
        "warp" => f64::from(u8::from(lane.warp)),
        "warp_bpm" => (lane.warp_bpm - 20.0) / 380.0,
        "warp_beats" => WARP_BEATS
            .iter()
            .enumerate()
            .min_by_key(|(_, beats)| beats.abs_diff(lane.warp_beats))
            .map_or(0.0, |(index, _)| index as f64 / 15.0),
        "mute" => f64::from(u8::from(lane.muted)),
        "solo" => f64::from(u8::from(lane.solo)),
        "sidechain" => f64::from(u8::from(lane.sidechain)),
        _ => 0.0,
    }
}

fn value(settings: &Settings, id: &str) -> f64 {
    match id {
        "studio.enabled" => f64::from(u8::from(settings.enabled)),
        "studio.return" => settings.return_gain,
        "studio.predelay" => settings.predelay_ms / 200.0,
        "studio.width" => settings.width / 2.0,
        "studio.low_cut" => (settings.low_cut - 20.0) / 1980.0,
        "studio.high_cut" => (settings.high_cut - 2000.0) / 18000.0,
        "studio.trim808" => (settings.drum_gain_db[0] + 12.0) / 24.0,
        "studio.trim909" => (settings.drum_gain_db[1] + 12.0) / 24.0,
        "sampler.on" => f64::from(u8::from(settings.sampler_bus.enabled)),
        "sampler.level" => settings.sampler_bus.gain / 2.0,
        "sampler.pan" => (settings.sampler_bus.pan + 1.0) / 2.0,
        "sampler.dist" => f64::from(u8::from(settings.sampler_bus.dist)),
        "sampler.delay" => settings.sampler_bus.delay,
        "sampler.send" => settings.sampler_bus.reverb_send,
        _ => {
            if let Some(i) = id
                .strip_prefix("studio.send")
                .and_then(|s| s.parse::<usize>().ok())
                .and_then(|n| n.checked_sub(1))
            {
                settings.sends.get(i).copied().unwrap_or(0.0)
            } else if let Some((i, name)) = lane_field(id) {
                lane_value(&settings.lanes[i], name)
            } else {
                0.0
            }
        }
    }
}

pub fn defaults() -> Vec<(String, u8)> {
    let settings = Settings::default();
    let duck = crate::sidechain::Settings::default();
    controls()
        .into_iter()
        .map(|(id, _)| {
            let v = duck_value(&duck, &id).unwrap_or_else(|| value(&settings, &id));
            let cc = (v.clamp(0.0, 1.0) * 127.0).round() as u8;
            (id, cc)
        })
        .collect()
}

impl App {
    pub(crate) fn studio_midi(&mut self) {
        if self.studio.history_restoring {
            return;
        }
        let before = self.studio.settings.clone();
        let duck_before = self.settings.sidechain.clone();
        let mut auditions = Vec::new();
        for event in &self.panel.events {
            if let Event::Press { id } = event {
                if let Some((i, "audition")) = lane_field(id) {
                    auditions.push(i);
                }
                if let Some((i, "warp_fit")) = lane_field(id)
                    && let Some(wave) = &self.studio.waves[i]
                {
                    let lane = &mut self.studio.settings.lanes[i];
                    lane.fit_warp(Some(wave.frames.len()));
                }
                continue;
            }
            let Event::Set { id, value } = event else {
                continue;
            };
            let raw = (*value).clamp(0, 127);
            let v = f64::from(raw) / 127.0;
            let duck = &mut self.settings.sidechain;
            match id.as_str() {
                "duck1.on" => duck.set_tb_enabled(0, raw >= 64),
                "duck2.on" => duck.set_tb_enabled(1, raw >= 64),
                "duck1.depth" => duck.depth[0] = (v * 100.0).round() as u8,
                "duck2.depth" => duck.depth[1] = (v * 100.0).round() as u8,
                "duck.release" => duck.release_ms = (20.0 + v * 980.0).round() as u16,
                "duck.source" => duck.source = (v * 2.0).round() as u8,
                "duck.instrument" => duck.instrument = gater_instrument(v),
                _ => {}
            }
            if id.starts_with("duck") {
                continue;
            }
            let s = &mut self.studio.settings;
            match id.as_str() {
                "studio.enabled" => s.enabled = raw >= 64,
                "studio.return" => s.return_gain = v,
                "studio.predelay" => s.predelay_ms = v * 200.0,
                "studio.width" => s.width = v * 2.0,
                "studio.low_cut" => s.low_cut = 20.0 + v * 1980.0,
                "studio.high_cut" => s.high_cut = 2000.0 + v * 18000.0,
                "studio.trim808" => s.drum_gain_db[0] = -12.0 + v * 24.0,
                "studio.trim909" => s.drum_gain_db[1] = -12.0 + v * 24.0,
                "sampler.on" => s.sampler_bus.enabled = raw >= 64,
                "sampler.level" => s.sampler_bus.gain = v * 2.0,
                "sampler.pan" => s.sampler_bus.pan = v * 2.0 - 1.0,
                "sampler.dist" => s.sampler_bus.dist = raw >= 64,
                "sampler.delay" => s.sampler_bus.delay = v,
                "sampler.send" => s.sampler_bus.reverb_send = v,
                _ => {
                    if let Some(index) = id
                        .strip_prefix("studio.send")
                        .and_then(|s| s.parse::<usize>().ok())
                    {
                        if (1..=4).contains(&index) {
                            s.sends[index - 1] = v;
                        }
                    } else if let Some((i, name)) = lane_field(id) {
                        let lane = &mut s.lanes[i];
                        match name {
                            "level" => lane.gain = v * 2.0,
                            "pan" => lane.pan = v * 2.0 - 1.0,
                            "pitch" => lane.pitch = (v * 48.0 - 24.0).round() as i8,
                            "send" => lane.send = v,
                            "start" => {
                                lane.start = v * (1.0 - 1e-9);
                                lane.end = lane.end.max(lane.start + 1e-9);
                            }
                            "end" => lane.end = (1e-9 + v * (1.0 - 1e-9)).max(lane.start + 1e-9),
                            "attack" => lane.attack_ms = 0.1 + v * 1999.9,
                            "release" => lane.release_ms = 1.0 + v * 2999.0,
                            "gate" => lane.gate = 0.01 + v * 15.99,
                            "mode" => lane.mode = mode(v),
                            "choke" => lane.choke = (v * 8.0).round() as u8,
                            "gater_source" => lane.gater_source = (v * 4.0).round() as u8,
                            "gater_instrument" => lane.gater_instrument = gater_instrument(v),
                            "gater_hold" => lane.gater_hold = 0.125 + v * 15.875,
                            "gater_depth" => lane.gater_depth = v,
                            "gater_attack_ms" => lane.gater_attack_ms = 0.1 + v * 99.9,
                            "gater_release_ms" => lane.gater_release_ms = 1.0 + v * 499.0,
                            "reverse" => lane.reverse = raw >= 64,
                            "warp" => {
                                if raw >= 64 && !lane.warp {
                                    let frames =
                                        self.studio.waves[i].as_ref().map(|w| w.frames.len());
                                    lane.fit_warp(frames);
                                } else if raw < 64 {
                                    lane.warp = false;
                                }
                            }
                            "warp_bpm" => {
                                lane.warp_bpm = 20.0 + v * 380.0;
                                if lane.warp_markers.points().is_empty() {
                                    lane.warp_grid = false;
                                }
                            }
                            "warp_beats" => {
                                lane.warp_beats = warp_beat_count(v);
                                lane.warp_grid = true;
                            }
                            "mute" => lane.muted = raw >= 64,
                            "solo" => lane.solo = raw >= 64,
                            "sidechain" => lane.sidechain = raw >= 64,
                            _ => {}
                        }
                    }
                }
            }
        }
        if self.studio.settings != before {
            self.studio.changed();
            self.doc.modified = true;
        }
        if self.settings.sidechain != duck_before {
            self.sidechain_changed(&self.studio.context.clone());
        }
        if !auditions.is_empty() {
            self.studio_poll(&self.studio.context.clone());
            for lane in auditions {
                self.studio_command(Command::StudioAudition(lane));
            }
        }
        self.studio_mirror();
    }

    pub(crate) fn studio_mirror(&mut self) {
        let s = &self.studio.settings;
        for (id, _) in controls() {
            if is_action(&id) {
                continue;
            }
            if let Some(v) = duck_value(&self.settings.sidechain, &id) {
                let duck = &self.settings.sidechain;
                if let Some(raw) = self.panel.value(&id) {
                    let normalized = f64::from(raw) / 127.0;
                    let same = match id.as_str() {
                        "duck1.depth" => (normalized * 100.0).round() as u8 == duck.depth[0],
                        "duck2.depth" => (normalized * 100.0).round() as u8 == duck.depth[1],
                        "duck.source" => (normalized * 2.0).round() as u8 == duck.source,
                        "duck.instrument" => gater_instrument(normalized) == duck.instrument,
                        "duck.release" => {
                            (20.0 + normalized * 980.0).round() as u16 == duck.release_ms
                        }
                        _ => false,
                    };
                    if same {
                        continue;
                    }
                }
                self.panel
                    .adopt(&id, (v.clamp(0.0, 1.0) * 127.0).round() as i32);
                continue;
            }
            if let Some((i, name)) = lane_field(&id)
                && let Some(raw) = self.panel.value(&id)
            {
                let v = f64::from(raw) / 127.0;
                let lane = &s.lanes[i];
                // Quantization and trim constraints must not re-arm pickup on our own edits.
                let same = match name {
                    "pitch" => (v * 48.0 - 24.0).round() as i8 == lane.pitch,
                    "mode" => mode(v) == lane.mode,
                    "choke" => (v * 8.0).round() as u8 == lane.choke,
                    "gater_source" => (v * 4.0).round() as u8 == lane.gater_source,
                    "gater_instrument" => gater_instrument(v) == lane.gater_instrument,
                    "warp_beats" => warp_beat_count(v) == lane.warp_beats,
                    "end" => {
                        ((1e-9 + v * (1.0 - 1e-9)).max(lane.start + 1e-9) - lane.end).abs() < 1e-12
                    }
                    _ => false,
                };
                if same {
                    continue;
                }
            }
            self.panel
                .adopt(&id, (value(s, &id).clamp(0.0, 1.0) * 127.0).round() as i32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        midi::{Binding, Cc, Config, Kind, Router, kind},
        model::Panel,
    };

    fn move_cc(app: &mut App, router: &mut Router, id: &str, value: u8, pickup: bool) {
        let config = Config {
            pickup,
            bindings: vec![Binding {
                input: 0,
                channel: 1,
                controller: 74,
                target: id.into(),
            }],
            ..Config::default()
        };
        router.apply(
            Cc {
                channel: 1,
                controller: 74,
                value,
            },
            &config,
            &mut app.panel,
            0.0,
        );
        app.studio_midi();
        app.panel.events.clear();
    }

    #[test]
    fn sampler_global_midi_controls_preserve_each_lanes_mix_and_send() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        app.studio.settings.lanes[0].send = 0.3;
        let lanes = app.studio.settings.lanes.clone();
        for id in [
            "sampler.level",
            "sampler.pan",
            "sampler.dist",
            "sampler.delay",
            "sampler.send",
        ] {
            move_cc(&mut app, &mut router, id, 127, false);
        }
        let bus = &app.studio.settings.sampler_bus;
        assert_eq!(
            (bus.gain, bus.pan, bus.delay, bus.reverb_send),
            (2.0, 1.0, 1.0, 1.0)
        );
        assert!(bus.dist);
        for id in [
            "sampler.level",
            "sampler.dist",
            "sampler.delay",
            "sampler.send",
        ] {
            for raw in (0..=127).rev() {
                move_cc(&mut app, &mut router, id, raw, true);
            }
        }
        let bus = &app.studio.settings.sampler_bus;
        assert_eq!((bus.gain, bus.delay, bus.reverb_send), (0.0, 0.0, 0.0));
        assert!(!bus.dist);
        assert_eq!(app.studio.settings.lanes, lanes);
    }

    #[test]
    fn reverb_and_delay_sends_follow_binary_button_states_without_affecting_other_channels() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        for row in 0..4 {
            let id = format!("studio.send{}", row + 1);
            for value in [127, 127, 0, 0, 127, 0] {
                move_cc(&mut app, &mut router, &id, value, true);
                for other in 0..4 {
                    assert_eq!(
                        app.studio.settings.sends[other],
                        if other == row {
                            f64::from(value) / 127.0
                        } else {
                            0.0
                        }
                    );
                }
            }
            let id = format!("mixer{}.delay", row + 1);
            let config = Config {
                pickup: true,
                bindings: vec![Binding {
                    input: 0,
                    channel: 1,
                    controller: 74,
                    target: id.clone(),
                }],
                ..Config::default()
            };
            for value in [127, 127, 0, 0, 127, 0] {
                router.apply(
                    Cc {
                        channel: 1,
                        controller: 74,
                        value,
                    },
                    &config,
                    &mut app.panel,
                    0.0,
                );
                assert_eq!(app.panel.value(&id), Some(value));
            }
        }
    }

    #[test]
    fn sampler_midi_on_off_retains_the_individual_mix_and_global_level() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        app.studio.settings.sampler_bus.gain = 0.6;
        let lanes = app.studio.settings.lanes.clone();
        move_cc(&mut app, &mut router, "sampler.on", 0, false);
        assert!(!app.studio.settings.sampler_bus.enabled);
        assert_eq!(app.panel.value("sampler.on"), Some(0));
        move_cc(&mut app, &mut router, "sampler.on", 127, false);
        assert!(app.studio.settings.sampler_bus.enabled);
        assert_eq!(app.studio.settings.sampler_bus.gain, 0.6);
        assert_eq!(app.studio.settings.lanes, lanes);
    }

    #[test]
    fn warp_source_tempo_fit_and_midi_pickup_reach_the_sampler_settings() {
        use orb_engine::studio::audio_file::Wave;
        use std::sync::Arc;
        let mut app = App::new(Panel::new(), None);
        app.studio.waves[0] = Some(Arc::new(Wave {
            frames: vec![[0.0; 2]; 88200],
        }));
        let mut router = Router::default();
        move_cc(&mut app, &mut router, "sampler1.warp_bpm", 0, false);
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 20.0);
        move_cc(&mut app, &mut router, "sampler1.warp_bpm", 127, true);
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 400.0);
        move_cc(&mut app, &mut router, "sampler1.warp_beats", 25, false);
        assert_eq!(app.studio.settings.lanes[0].warp_beats, 4);
        move_cc(&mut app, &mut router, "sampler1.warp_fit", 127, false);
        assert!(app.studio.settings.lanes[0].warp);
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 120.0);
        app.studio.settings.lanes[0].warp_bpm = 111.0;
        move_cc(&mut app, &mut router, "sampler1.warp_fit", 127, false);
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 111.0);
        move_cc(&mut app, &mut router, "sampler1.warp_fit", 0, false);
        move_cc(&mut app, &mut router, "sampler1.warp_fit", 127, false);
        assert_eq!(app.studio.settings.lanes[0].warp_bpm, 120.0);
        for value in 0..=127 {
            move_cc(&mut app, &mut router, "sampler1.warp_beats", value, true);
            assert_eq!(app.panel.value("sampler1.warp_beats"), Some(value));
        }
        move_cc(&mut app, &mut router, "sampler1.warp", 0, false);
        assert!(!app.studio.settings.lanes[0].warp);
    }

    #[test]
    fn all_studio_controls_are_registered_with_correct_midi_behavior() {
        assert_eq!(controls().len(), 233);
        for (id, _) in controls() {
            assert!(kind(&id).is_some(), "{id}");
            assert!(Panel::new().value(&id).is_some(), "{id}");
        }
        assert_eq!(kind("sampler8.audition"), Some(Kind::Button));
        assert_eq!(kind("sampler1.mute"), Some(Kind::Switch));
        assert_eq!(kind("studio.enabled"), Some(Kind::Switch));
        assert_eq!(kind("duck1.on"), Some(Kind::Switch));
        assert_eq!(kind("sampler.dist"), Some(Kind::Switch));
        assert_eq!(kind("sampler.on"), Some(Kind::Switch));
        assert_eq!(kind("sampler1.warp"), Some(Kind::Switch));
        assert_eq!(kind("sampler8.warp_fit"), Some(Kind::Button));
    }

    #[test]
    fn selectors_keep_pickup_latched_through_every_quantization_boundary() {
        for id in [
            "sampler1.pitch",
            "sampler1.mode",
            "sampler1.choke",
            "sampler1.gater_source",
            "sampler1.gater_instrument",
        ] {
            let mut app = App::new(Panel::new(), None);
            let mut router = Router::default();
            let current = app.panel.value(id).unwrap();
            for value in (0..=current).rev().chain(1..=127) {
                move_cc(&mut app, &mut router, id, value, true);
                assert_eq!(app.panel.value(id), Some(value), "{id}: CC {value}");
            }
            let lane = &app.studio.settings.lanes[0];
            match id {
                "sampler1.pitch" => assert_eq!(lane.pitch, 24),
                "sampler1.mode" => assert_eq!(lane.mode, Mode::Loop),
                "sampler1.choke" => assert_eq!(lane.choke, 8),
                "sampler1.gater_source" => assert_eq!(lane.gater_source, 4),
                _ => assert_eq!(lane.gater_instrument, 11),
            }
        }
    }

    #[test]
    fn envelope_gater_and_boolean_endpoints_reach_the_settings() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        for name in [
            "attack",
            "release",
            "gate",
            "gater_depth",
            "gater_attack_ms",
            "gater_release_ms",
            "reverse",
            "mute",
            "solo",
            "sidechain",
        ] {
            move_cc(
                &mut app,
                &mut router,
                &format!("sampler8.{name}"),
                127,
                false,
            );
        }
        let lane = &app.studio.settings.lanes[7];
        assert_eq!(
            (lane.attack_ms, lane.release_ms, lane.gate),
            (2000.0, 3000.0, 16.0)
        );
        assert_eq!(
            (
                lane.gater_depth,
                lane.gater_attack_ms,
                lane.gater_release_ms
            ),
            (1.0, 100.0, 500.0)
        );
        assert!(lane.reverse && lane.muted && lane.solo && lane.sidechain);
        for name in [
            "attack",
            "release",
            "gate",
            "gater_depth",
            "gater_attack_ms",
            "gater_release_ms",
            "reverse",
            "mute",
            "solo",
            "sidechain",
        ] {
            move_cc(&mut app, &mut router, &format!("sampler8.{name}"), 0, true);
        }
        let lane = &app.studio.settings.lanes[7];
        assert_eq!(lane.gater_depth, 0.0);
        assert!(!lane.reverse && !lane.muted && !lane.solo && !lane.sidechain);
        move_cc(&mut app, &mut router, "studio.enabled", 0, true);
        assert!(!app.studio.settings.enabled);
    }

    #[test]
    fn moving_trim_end_below_start_keeps_valid_bounds_and_pickup_continues() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        move_cc(&mut app, &mut router, "sampler1.start", 100, false);
        for value in (0..=127).rev() {
            move_cc(&mut app, &mut router, "sampler1.end", value, true);
            assert_eq!(app.panel.value("sampler1.end"), Some(value));
            let lane = &app.studio.settings.lanes[0];
            assert!(lane.end > lane.start && lane.end <= 1.0);
        }
    }

    #[test]
    fn drum_gater_sources_instruments_and_hold_cover_the_full_cc_range() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        for (cc, source) in [(0, 0), (32, 1), (64, 2), (95, 3), (127, 4)] {
            move_cc(&mut app, &mut router, "sampler8.gater_source", cc, false);
            assert_eq!(app.studio.settings.lanes[7].gater_source, source);
        }
        move_cc(
            &mut app,
            &mut router,
            "sampler8.gater_instrument",
            127,
            false,
        );
        assert_eq!(app.studio.settings.lanes[7].gater_instrument, 11);
        for cc in (0..=127).rev() {
            move_cc(&mut app, &mut router, "sampler8.gater_instrument", cc, true);
            assert_eq!(app.panel.value("sampler8.gater_instrument"), Some(cc));
        }
        assert_eq!(app.studio.settings.lanes[7].gater_instrument, 1);
        move_cc(&mut app, &mut router, "sampler8.gater_source", 95, false);
        move_cc(
            &mut app,
            &mut router,
            "sampler8.gater_instrument",
            127,
            false,
        );
        assert_eq!(app.studio.settings.lanes[7].gater_instrument, 11);
        for (cc, hold) in [(0, 0.125), (127, 16.0)] {
            move_cc(&mut app, &mut router, "sampler8.gater_hold", cc, false);
            assert_eq!(app.studio.settings.lanes[7].gater_hold, hold);
        }
    }

    #[test]
    fn tb_duck_switches_are_independent_and_bypass_retains_depth() {
        let mut app = App::new(Panel::new(), None);
        let mut router = Router::default();
        move_cc(&mut app, &mut router, "duck1.depth", 127, false);
        move_cc(&mut app, &mut router, "duck1.on", 127, true);
        assert!(app.settings.sidechain.enabled);
        assert_eq!(app.settings.sidechain.bypass, [false, true]);
        assert_eq!(app.settings.sidechain.engine_config().depth, [100, 0]);
        assert_eq!(app.panel.value("duck2.on"), Some(0));
        move_cc(&mut app, &mut router, "duck1.on", 0, true);
        assert_eq!(app.settings.sidechain.depth, [100, 55]);
        assert_eq!(app.settings.sidechain.engine_config().depth, [0, 0]);
        move_cc(&mut app, &mut router, "duck2.on", 127, true);
        assert_eq!(app.settings.sidechain.bypass, [true, false]);
        assert_eq!(app.settings.sidechain.engine_config().depth, [0, 55]);
        move_cc(&mut app, &mut router, "duck1.on", 127, true);
        assert_eq!(app.settings.sidechain.engine_config().depth, [100, 55]);
        move_cc(&mut app, &mut router, "duck.release", 0, false);
        assert_eq!(app.settings.sidechain.release_ms, 20);
        move_cc(&mut app, &mut router, "duck.release", 127, false);
        assert_eq!(app.settings.sidechain.release_ms, 1000);
        move_cc(&mut app, &mut router, "duck.source", 0, false);
        assert_eq!(app.settings.sidechain.source, 0);
        move_cc(&mut app, &mut router, "duck.source", 127, false);
        assert_eq!(app.settings.sidechain.source, 2);
    }

    #[test]
    fn duck_selectors_and_percentages_do_not_rearm_their_own_pickup() {
        for id in [
            "duck1.depth",
            "duck2.depth",
            "duck.source",
            "duck.release",
            "duck.instrument",
        ] {
            let mut app = App::new(Panel::new(), None);
            let mut router = Router::default();
            let current = app.panel.value(id).unwrap();
            for raw in (0..=current).rev().chain(1..=127) {
                move_cc(&mut app, &mut router, id, raw, true);
                assert_eq!(app.panel.value(id), Some(raw), "{id}: {raw}");
            }
        }
    }
}
