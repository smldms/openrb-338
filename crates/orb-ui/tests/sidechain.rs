use std::sync::Arc;

use eframe::egui;
use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};
use orb_ui::app::App;
use orb_ui::link::Link;
use orb_ui::midi::{Binding, Cc, Config, Kind, Router, kind};
use orb_ui::model::Panel;
use orb_ui::settings::Settings;

fn calls(audio: &mut Audio) {
    let mut out = vec![0; 2 * CALL_FRAMES];
    audio.render_i16(&mut out);
    audio.render_i16(&mut out);
}

#[test]
fn midi_learn_ch_level_reaches_zero_and_preserves_open_hat_level() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert_eq!(kind("tr909.ch_level"), Some(Kind::Knob));
    let mut config = Config {
        minimum_one: true,
        pickup: false,
        ..Config::default()
    };
    config.bindings.push(Binding {
        input: 0,
        target: "tr909.ch_level".into(),
        channel: 1,
        controller: 74,
    });
    let before = app.panel.value("tr909.hh_level");
    Router::default().apply(
        Cc {
            channel: 1,
            controller: 74,
            value: 1,
        },
        &config,
        &mut app.panel,
        0.0,
    );
    app.sync_effects(&egui::Context::default());
    app.note_edits();
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert_eq!(app.panel.value("tr909.ch_level"), Some(0));
    assert_eq!(app.panel.value("tr909.hh_level"), before);
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        0
    );
    assert_eq!(app.settings.closed_hat_level, Some(0));
    assert!(
        !app.doc.modified,
        "the global mix extension is not stored in RBS"
    );
}

#[test]
fn additional_mix_settings_round_trip_and_old_settings_default_to_bypass() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ui.toml");
    let mut s = Settings::load_from(&file);
    assert!(!s.sidechain.enabled);
    assert_eq!(s.closed_hat_level, None);
    s.sidechain.enabled = true;
    s.sidechain.depth = [0, 80];
    s.sidechain.source = 1;
    s.sidechain.instrument = 8;
    s.sidechain.release_ms = 270;
    s.closed_hat_level = Some(0);
    s.save().unwrap();
    assert_eq!(Settings::load_from(&file), s);
}

#[test]
fn old_sidechain_settings_keep_the_kick_and_existing_depths() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("ui.toml");
    std::fs::write(
        &file,
        concat!(
            "[sidechain]\nenabled=true\nsource=1\ndepth=[22,78]\n",
            "release_ms=240\nbypass=[true,false]\n",
        ),
    )
    .unwrap();
    let settings = Settings::load_from(&file);
    assert_eq!(settings.sidechain.instrument, 1);
    assert_eq!(settings.sidechain.source, 1);
    assert_eq!(settings.sidechain.depth, [22, 78]);
    assert_eq!(settings.sidechain.bypass, [true, false]);
    assert_eq!(settings.sidechain.engine_config().depth, [0, 78]);
    assert_eq!(settings.sidechain.engine_config().instrument, 1);
}

#[test]
fn editing_open_hat_first_freezes_the_previous_closed_hat_level() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let old_ch = app.panel.value("tr909.ch_level").unwrap();
    app.panel.set_knob("tr909.hh_level", 0);
    app.sync_effects(&egui::Context::default());
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert_eq!(app.panel.value("tr909.hh_level"), Some(0));
    assert_eq!(app.panel.value("tr909.ch_level"), Some(old_ch));
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        old_ch
    );
}

#[test]
fn ch_pickup_does_not_rearm_on_a_stale_snapshot_between_audio_callbacks() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let mut config = Config {
        pickup: true,
        ..Config::default()
    };
    config.bindings.push(Binding {
        input: 0,
        target: "tr909.ch_level".into(),
        channel: 1,
        controller: 74,
    });
    let mut router = Router::default();
    for value in [100, 80, 20, 0] {
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
        app.sync_effects(&egui::Context::default());
        app.link.as_mut().unwrap().sync(&mut app.panel);
        assert_eq!(app.panel.value("tr909.ch_level"), Some(value));
    }
    calls(&mut audio);
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        0
    );
}

#[test]
fn commands_survive_a_new_song_and_the_sidechain_window_renders() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    app.settings.sidechain.enabled = true;
    app.settings.sidechain.depth = [75, 0];
    app.settings.closed_hat_level = Some(12);
    app.sync_effects(&ctx);
    calls(&mut audio);
    app.new_now();
    calls(&mut audio);
    assert_eq!(audio.current_mut().frame.sidechain.config().depth, [75, 0]);
    assert!(audio.current_mut().frame.sidechain.config().enabled);
    assert_eq!(
        audio.current_mut().frame.devices.channels[3].closed_hat_level(),
        12
    );
    app.sidechain.open = true;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(936.0, 787.0),
        )),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

#[test]
fn the_apps_audio_export_includes_sidechain_processing() {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let mut tb = vec![0; 98];
    tb[1] = 16;
    let mut drums = vec![0; 194];
    drums[1] = 16;
    drums[3] = 1;
    app.panel.replace_pattern(0, 0, tb);
    app.panel.replace_pattern(2, 0, drums);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    app.panel.transport.loop_start = 0;
    app.panel.transport.loop_end = 768;
    let ctx = egui::Context::default();
    let dir = tempfile::tempdir().unwrap();
    let mut files = Vec::new();
    for enabled in [false, true] {
        app.settings.sidechain.enabled = enabled;
        app.settings.sidechain.depth = [100, 0];
        let path = dir.path().join(format!("duck-{enabled}.wav"));
        app.export_to(&path);
        for _ in 0..1200 {
            app.poll_export(&ctx);
            if !app.exporting() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!app.exporting());
        files.push(std::fs::read(path).expect("the export must succeed"));
    }
    assert_eq!(files[0].len(), files[1].len());
    assert_ne!(
        files[0], files[1],
        "the exported audio must include the ducking"
    );
}
