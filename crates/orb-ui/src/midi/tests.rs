use std::sync::Arc;

use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};

use super::*;
use crate::link::Link;

#[test]
fn context_learning_maximum_and_removal_use_the_regular_saved_routing() {
    let ctx = egui::Context::default();
    let mut app = App::new(Panel::new(), None);
    app.midi.started = true;
    app.settings.midi.enabled = true;
    app.midi.filter = "tr909.ch_level".into();
    let action = |app: &mut App, action: context::Action| {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            context::queue(ui, action.clone());
            app.midi_tick(ui.ctx());
        });
        output.textures_delta.clear();
    };
    action(&mut app, context::Action::Learn("mixer3.level".into()));
    assert_eq!(app.midi.learning.as_deref(), Some("mixer3.level"));
    assert!(
        app.midi
            .handle_cc(cc(7, 100), &mut app.settings.midi, &mut app.panel, 0.0)
    );
    action(
        &mut app,
        context::Action::Range("mixer3.level".into(), ValueRange { max: 50 }),
    );
    assert_eq!(app.settings.midi.ranges["mixer3.level"].max, 50);
    assert!(app.midi.learning.is_none());
    action(&mut app, context::Action::Remove("mixer3.level".into()));
    assert!(app.settings.midi.bindings.is_empty());
    assert!(app.settings.midi.ranges.is_empty());
}

fn cc(controller: u8, value: u8) -> Cc {
    Cc {
        channel: 1,
        controller,
        value,
    }
}

#[test]
fn a_precise_filter_changes_the_actual_learn_target_and_cancels_stale_learning() {
    let mut midi = Midi {
        filter: "tr909.ch_level".into(),
        ..Midi::default()
    };
    midi.begin_learn();
    assert_eq!(midi.learning.as_deref(), Some("tr909.ch_level"));
    for filter in ["tr909.hh_level", "  HH level  ", "TR-909 / OH level"] {
        midi.filter = filter.into();
        let candidates = midi.target_candidates();
        assert_eq!(candidates, vec![("tr909.hh_level".into(), Kind::Knob)]);
        assert_eq!(midi.selected, "tr909.hh_level");
        midi.begin_learn();
        assert_eq!(midi.learning.as_deref(), Some("tr909.hh_level"));
    }
    midi.filter = "tr909.ch_level".into();
    midi.target_candidates();
    assert!(midi.learning.is_none());
    midi.begin_learn();
    assert_eq!(midi.learning.as_deref(), Some("tr909.ch_level"));
}

#[test]
fn an_empty_or_ambiguous_result_cannot_learn_a_hidden_previous_target() {
    let mut midi = Midi {
        filter: "tr909.ch_level".into(),
        ..Midi::default()
    };
    midi.begin_learn();
    midi.filter = "no matching parameter".into();
    midi.begin_learn();
    assert!(midi.selected.is_empty() && midi.learning.is_none());
    midi.filter = "level".into();
    midi.begin_learn();
    assert!(midi.selected.is_empty() && midi.learning.is_none());
}

#[test]
fn the_rendered_midi_window_reconciles_a_filter_with_the_previous_selection() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    app.midi.open = true;
    app.midi.selected = "tr909.ch_level".into();
    app.midi.filter = "tr909.hh_level".into();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1000.0, 900.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
    assert_eq!(app.midi.selected, "tr909.hh_level");
    app.midi.begin_learn();
    assert_eq!(app.midi.learning.as_deref(), Some("tr909.hh_level"));
}

fn calls(audio: &mut Audio) {
    let mut out = vec![0; 2 * CALL_FRAMES];
    audio.render_i16(&mut out);
    audio.render_i16(&mut out);
}

#[test]
fn learning_ch_then_oh_keeps_two_assignments_and_controls_the_correct_engine_levels() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    app.settings.midi.pickup = false;
    app.settings.midi.minimum_one = true;
    for (target, controller) in [("tr909.ch_level", 74), ("tr909.hh_level", 75)] {
        app.midi.filter = target.into();
        app.midi.begin_learn();
        assert_eq!(app.midi.learning.as_deref(), Some(target));
        assert!(app.midi.handle_cc(
            cc(controller, 50),
            &mut app.settings.midi,
            &mut app.panel,
            0.0
        ));
    }
    assert_eq!(app.settings.midi.bindings.len(), 2);
    assert_eq!(app.settings.midi.bindings[0].target, "tr909.ch_level");
    assert_eq!(app.settings.midi.bindings[1].target, "tr909.hh_level");
    let ctx = egui::Context::default();
    app.midi
        .handle_cc(cc(74, 1), &mut app.settings.midi, &mut app.panel, 0.0);
    app.sync_effects(&ctx);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.midi
        .handle_cc(cc(75, 127), &mut app.settings.midi, &mut app.panel, 0.0);
    app.sync_effects(&ctx);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert_eq!(app.panel.value("tr909.ch_level"), Some(0));
    assert_eq!(app.panel.value("tr909.hh_level"), Some(127));
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        0
    );
}

#[test]
fn reusing_one_cc_reports_which_assignment_was_replaced() {
    let mut midi = Midi::default();
    let mut config = Config::default();
    let mut panel = Panel::new();
    for target in ["tr909.ch_level", "tr909.hh_level"] {
        midi.filter = target.into();
        midi.begin_learn();
        assert!(midi.handle_cc(cc(74, 64), &mut config, &mut panel, 0.0));
    }
    assert_eq!(config.bindings.len(), 1);
    assert_eq!(config.bindings[0].target, "tr909.hh_level");
    let notice = midi.notice.unwrap();
    assert!(notice.contains("OH level (Open Hat)"));
    assert!(notice.contains("Replaced TR-909 / CH level (Closed Hat)"));
}

#[test]
fn two_controllers_can_learn_identical_ccs_without_conflict() {
    let mut midi = Midi::default();
    let mut config = Config {
        pickup: false,
        ..Config::default()
    };
    let mut panel = Panel::new();
    for (input, target) in [(0, "tr909.ch_level"), (1, "tr909.hh_level")] {
        midi.filter = target.into();
        midi.begin_learn();
        assert!(midi.handle_cc_from(input, cc(74, 64), &mut config, &mut panel, 0.0));
    }
    assert_eq!(config.bindings.len(), 2);
    midi.handle_cc_from(0, cc(74, 0), &mut config, &mut panel, 0.0);
    midi.handle_cc_from(1, cc(74, 127), &mut config, &mut panel, 0.0);
    assert_eq!(panel.value("tr909.ch_level"), Some(0));
    assert_eq!(panel.value("tr909.hh_level"), Some(127));
}

#[test]
fn calibration_is_per_device_and_legacy_bindings_stay_on_input_one() {
    let mut config = Config {
        pickup: false,
        minimum_one: true,
        additional_inputs: vec![InputSettings::default()],
        ..Config::default()
    };
    for (input, target) in [(0, "mixer1.level"), (1, "mixer2.level")] {
        config.assign(Binding {
            input,
            channel: 1,
            controller: 74,
            target: target.into(),
        });
    }
    let mut router = Router::default();
    let mut panel = Panel::new();
    router.apply_from(0, cc(74, 2), &config, &mut panel, 0.0);
    router.apply_from(1, cc(74, 2), &config, &mut panel, 0.0);
    assert_eq!(panel.value("mixer1.level"), Some(0));
    assert_eq!(panel.value("mixer2.level"), Some(2));
    let old: Config = toml::from_str(
        "port='UC33'\n[[bindings]]\nchannel=1\ncontroller=74\ntarget='mixer1.level'",
    )
    .unwrap();
    assert_eq!(old.bindings[0].input, 0);
    assert!(old.additional_inputs.is_empty());
}

#[test]
fn controller_slots_are_stable_when_an_input_is_disconnected() {
    let config = Config {
        port: Some("UC33".into()),
        additional_inputs: vec![
            InputSettings::default(),
            InputSettings {
                port: Some("Pads".into()),
                minimum_one: false,
                minimum_cc: 0,
            },
        ],
        ..Config::default()
    };
    assert_eq!(config.inputs(), vec![(0, "UC33"), (2, "Pads")]);
    let encoded = toml::to_string(&config).unwrap();
    assert_eq!(toml::from_str::<Config>(&encoded).unwrap(), config);
}

#[test]
fn button_edges_are_isolated_between_controller_inputs() {
    let mut config = Config::default();
    for (input, target) in [(0, "sampler1.audition"), (1, "sampler2.audition")] {
        config.assign(Binding {
            input,
            channel: 1,
            controller: 74,
            target: target.into(),
        });
    }
    let mut router = Router::default();
    let mut panel = Panel::new();
    for input in [0, 1, 0, 1] {
        router.apply_from(input, cc(74, 127), &config, &mut panel, 0.0);
    }
    assert_eq!(panel.events.len(), 2);
    router.apply_from(0, cc(74, 0), &config, &mut panel, 0.0);
    router.apply_from(0, cc(74, 127), &config, &mut panel, 0.0);
    assert_eq!(panel.events.len(), 3);
}

#[test]
fn every_numeric_floor_is_monotone_and_standard_midi_is_an_exact_identity() {
    for floor in 0..=16 {
        let mut previous = 0;
        let mut values = std::collections::BTreeSet::new();
        for raw in 0..=127 {
            let actual = continuous_value(raw, floor);
            if raw <= floor {
                assert_eq!(actual, 0);
            }
            if floor == 0 {
                assert_eq!(actual, raw);
            }
            assert!(actual >= previous, "floor={floor}, raw={raw}");
            previous = actual;
            values.insert(actual);
        }
        assert_eq!(previous, 127);
        assert_eq!(values.len(), 128 - usize::from(floor));
    }
    assert_eq!(continuous_value(1, 1), 0);
    assert_eq!(continuous_value(2, 1), 1);
    assert_eq!(continuous_value(2, 2), 0);
    assert_eq!(continuous_value(3, 2), 1);
}

#[test]
fn old_uc33_settings_migrate_each_input_and_numeric_floors_survive_restart() {
    let old =
        "[midi]\nminimum_one=true\n[[midi.additional_inputs]]\nport='Pads'\nminimum_one=true\n";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ui.toml");
    std::fs::write(&path, old).unwrap();
    let mut settings = crate::settings::Settings::load_from(&path);
    assert_eq!(settings.midi.minimum_cc, 2);
    assert_eq!(settings.midi.additional_inputs[0].minimum_cc, 2);
    assert!(!settings.midi.minimum_one);
    assert!(!settings.midi.additional_inputs[0].minimum_one);
    settings.midi.minimum_cc = 0;
    settings.midi.additional_inputs[0].minimum_cc = 5;
    settings.save().unwrap();
    assert_eq!(crate::settings::Settings::load_from(&path), settings);
    settings.midi.minimum_cc = 200;
    settings.midi.additional_inputs[0].minimum_cc = 200;
    settings.midi.sanitize();
    assert_eq!(settings.midi.minimum_cc, 16);
    assert_eq!(settings.midi.additional_inputs[0].minimum_cc, 16);
}

#[test]
fn sampler_audio_can_be_silenced_by_a_controller_whose_minimum_is_two() {
    for target in ["sampler1.level", "sampler.level", "sampler.on"] {
        assert_sampler_midi_mute(target);
    }
}

fn assert_sampler_midi_mute(target: &str) {
    use orb_engine::studio::{Studio, audio_file::Wave};
    use orb_player::Command;
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    // The kit startup preview bypasses the sampler and the four legacy mixer faders.
    app.settings.startup_sound = false;
    app.new_now();
    app.studio.modern = true;
    let ctx = egui::Context::default();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        app.studio_poll(&ctx);
        calls(&mut audio);
        if audio.current_mut().frame.studio.is_some() {
            break;
        }
        assert!(
            std::time::Instant::now() < until,
            "Studio preparation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let wave = Box::new(Wave {
        frames: vec![[0.25; 2]; 44100],
    });
    app.link
        .as_mut()
        .unwrap()
        .effect_command(Command::StudioWave {
            lane: 0,
            wave: Some(wave),
        });
    app.studio.settings.lanes[0].steps[0] = 127;
    let mut config = Config {
        minimum_cc: 2,
        pickup: false,
        ..Config::default()
    };
    config.assign(Binding {
        input: 0,
        channel: 1,
        controller: 74,
        target: target.into(),
    });
    let mut router = Router::default();
    router.apply(cc(74, 127), &config, &mut app.panel, 0.0);
    app.studio_midi();
    app.studio_poll(&ctx);
    // Isolate the sampler so legacy channels cannot satisfy either audio assertion.
    for channel in 1..=4 {
        app.panel.set_fader(&format!("mixer{channel}.level"), 0);
    }
    app.panel.press("transport.play", 0.0);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let mut out = vec![0i16; 2 * CALL_FRAMES];
    let mut audible = false;
    for _ in 0..12 {
        audio.render_i16(&mut out);
        audible |= out.iter().any(|v| v.unsigned_abs() > 100);
    }
    assert!(
        audible,
        "the sampler must actually sound before testing zero"
    );
    let mixer = &app.link.as_mut().unwrap().player().snapshot().mixer;
    assert!(mixer.channels.iter().all(|channel| channel.level == 0));
    router.apply(cc(74, 2), &config, &mut app.panel, 0.0);
    app.studio_midi();
    app.studio_poll(&ctx);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    for _ in 0..12 {
        audio.render_i16(&mut out);
    }
    let studio: &Studio = audio.current_mut().frame.studio.as_ref().unwrap();
    if target == "sampler.on" {
        assert!(!studio.settings.sampler_bus.enabled);
    } else if target == "sampler.level" {
        assert_eq!(studio.settings.sampler_bus.gain, 0.0);
    } else {
        assert_eq!(studio.settings.lanes[0].gain, 0.0);
    }
    assert_eq!(app.panel.value(target), Some(0));
    assert!(out.iter().all(|v| v.unsigned_abs() <= 1));
}
