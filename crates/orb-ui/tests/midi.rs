//! Controller mapping is tested without a MIDI device; the player test also renders audio.

use std::sync::Arc;

use orb_engine::tables::Tables;
use orb_player::CALL_FRAMES;
use orb_ui::link::Link;
use orb_ui::midi::{Binding, Cc, Config, Kind, Router, kind, targets};
use orb_ui::model::{Event, Panel};
use orb_ui::settings::Settings;

fn config(target: &str, pickup: bool) -> Config {
    Config {
        enabled: true,
        port: Some("UC-33 USB MIDI".into()),
        pickup,
        bindings: vec![Binding {
            channel: 1,
            controller: 74,
            target: target.into(),
        }],
    }
}

fn cc(channel: u8, value: u8) -> Cc {
    Cc {
        channel,
        controller: 74,
        value,
    }
}

#[test]
fn only_complete_control_changes_are_accepted() {
    assert_eq!(Cc::decode(0x007f_4abf), Some(cc(16, 127)));
    assert_eq!(Cc::decode(0x0000_4ab0), Some(cc(1, 0)));
    for invalid in [0xf8, 0xfe, 0x0064_3c90, 0x0080_4ab0, 0x007f_80b0] {
        assert_eq!(Cc::decode(invalid), None);
    }
}

#[test]
fn all_four_machines_mixer_and_effects_are_mappable() {
    for section in [
        "tb303_1.", "tb303_2.", "tr808.", "tr909.", "mixer1.", "mixer2.", "mixer3.", "mixer4.",
        "master.", "delay.", "dist.", "pcf.", "comp.",
    ] {
        assert!(
            targets().iter().any(|(id, _)| id.starts_with(section)),
            "{section}"
        );
    }
    assert_eq!(kind("tb303_1.accent"), Some(Kind::Knob));
    assert_eq!(kind("tb303_1.key1"), None);
    assert_eq!(kind("does.not.exist"), None);
}

#[test]
fn continuous_controls_keep_the_full_range_and_ignore_other_channels() {
    let mut p = Panel::new();
    let mut r = Router::default();
    for id in [
        "tb303_1.cutoff",
        "tr808.bd_decay",
        "tr909.sd_tone",
        "mixer3.level",
        "master.fader",
        "mixer4.delay",
        "pcf.freq",
        "dist.amount",
        "comp.threshold",
    ] {
        let c = config(id, false);
        for value in 0..=127 {
            r.apply(cc(1, value), &c, &mut p, 0.0);
            assert_eq!(p.value(id), Some(value), "{id}: {value}");
        }
        r.apply(cc(2, 0), &c, &mut p, 0.0);
        assert_eq!(p.value(id), Some(127));
    }
}

#[test]
fn pickup_waits_for_crossing_and_rearms_after_an_external_change() {
    let mut p = Panel::new();
    let c = config("tb303_1.cutoff", true);
    let mut r = Router::default();
    p.adopt("tb303_1.cutoff", 64);
    p.events.clear();
    r.apply(cc(1, 10), &c, &mut p, 0.0);
    r.apply(cc(1, 30), &c, &mut p, 0.0);
    assert_eq!(p.value("tb303_1.cutoff"), Some(64));
    assert!(p.events.is_empty());
    r.apply(cc(1, 70), &c, &mut p, 0.0);
    r.apply(cc(1, 80), &c, &mut p, 0.0);
    assert_eq!(p.value("tb303_1.cutoff"), Some(80));
    // A new song or mouse edit moves the engine parameter far below the physical knob.
    p.adopt("tb303_1.cutoff", 20);
    r.apply(cc(1, 75), &c, &mut p, 0.0);
    assert_eq!(p.value("tb303_1.cutoff"), Some(20));
    r.apply(cc(1, 15), &c, &mut p, 0.0);
    assert_eq!(p.value("tb303_1.cutoff"), Some(15));
}

#[test]
fn buttons_fire_once_per_press_and_selectors_use_their_real_range() {
    let mut p = Panel::new();
    let mut r = Router::default();
    let c = config("mixer3.dist", false);
    let initial = p.toggle_on("mixer3.dist");
    for value in [127, 127, 0] {
        r.apply(cc(1, value), &c, &mut p, 0.0);
    }
    assert_eq!(p.toggle_on("mixer3.dist"), !initial);
    r.apply(cc(1, 127), &c, &mut p, 0.0);
    assert_eq!(p.toggle_on("mixer3.dist"), initial);
    for (id, lo, hi) in [("delay.steps", 1, 32), ("pcf.pattern", 0, 54)] {
        let c = config(id, false);
        r.apply(cc(1, 0), &c, &mut p, 0.0);
        assert_eq!(p.display(id), Some(lo));
        r.apply(cc(1, 127), &c, &mut p, 0.0);
        assert_eq!(p.display(id), Some(hi));
    }
    let c = config("tb303_1.wave", false);
    r.apply(cc(1, 127), &c, &mut p, 0.0);
    assert_eq!(p.switch_position("tb303_1.wave"), 1);
    r.apply(cc(1, 0), &c, &mut p, 0.0);
    assert_eq!(p.switch_position("tb303_1.wave"), 0);
}

#[test]
fn learn_replaces_conflicts_and_settings_survive_restart() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("ui.toml");
    let mut s = Settings::load_from(&file);
    s.midi = config("mixer3.level", true);
    s.midi.assign(Binding {
        channel: 1,
        controller: 74,
        target: "dist.amount".into(),
    });
    assert_eq!(s.midi.bindings.len(), 1);
    s.midi.assign(Binding {
        channel: 2,
        controller: 10,
        target: "dist.amount".into(),
    });
    assert_eq!(s.midi.bindings.len(), 1);
    s.midi.bindings.push(Binding {
        channel: 0,
        controller: 255,
        target: "bad".into(),
    });
    s.midi.sanitize();
    assert_eq!(s.midi.bindings.len(), 1);
    s.save().unwrap();
    assert_eq!(Settings::load_from(&file), s);
    // Old settings remain valid and corrupted MIDI preferences cannot create unsafe bindings.
    std::fs::write(&file, "startup_sound = false\n[midi]\npickup = 'bad'\n").unwrap();
    let s = Settings::load_from(&file);
    assert!(!s.startup_sound);
    assert_eq!(s.midi, Config::default());
}

#[test]
fn midi_edits_reach_the_audio_player_and_follow_its_snapshot() {
    let samples = Some(rbm::standard::builtin_samples());
    let (mut link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut p = Panel::new();
    link.load_new(samples, &mut p);
    let mut out = vec![0i16; 2 * CALL_FRAMES];
    audio.render_i16(&mut out);
    link.sync(&mut p);
    let mut router = Router::default();
    for id in [
        "tb303_1.cutoff",
        "tb303_2.reso",
        "tr808.bd_decay",
        "tr909.sd_tone",
        "mixer3.level",
        "mixer4.delay",
        "master.fader",
        "dist.amount",
        "delay.feedback",
        "pcf.freq",
        "comp.threshold",
    ] {
        router.apply(cc(1, 37), &config(id, false), &mut p, 0.0);
        assert!(
            p.events
                .iter()
                .any(|e| matches!(e, Event::Set { id: changed, .. } if changed == id))
        );
        link.sync(&mut p);
        audio.render_i16(&mut out);
        audio.render_i16(&mut out);
        // Overwrite the local display to prove the following value comes from the engine.
        p.adopt(id, 111);
        link.sync(&mut p);
        assert_eq!(p.value(id), Some(37), "{id}");
    }
}

#[test]
fn the_midi_window_renders_without_a_connected_device() {
    let ctx = eframe::egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = orb_ui::app::App::new(Panel::new(), None);
    app.midi.open = true;
    let input = eframe::egui::RawInput {
        screen_rect: Some(eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::Vec2::new(1000.0, 800.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    assert!(app.midi.open);
    assert!(!app.settings.midi.enabled);
}
