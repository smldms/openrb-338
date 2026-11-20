use super::*;
use crate::{link::Link, model::Panel};
use eframe::egui;
use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};

#[test]
fn graphite_navigation_has_no_mods_menu() {
    fn has_mods(shape: &egui::Shape) -> bool {
        match shape {
            egui::Shape::Text(t) => t.galley.text() == "Mods",
            egui::Shape::Vec(items) => items.iter().any(has_mods),
            _ => false,
        }
    }
    let ctx = Context::default();
    let mut app = App::new(Panel::new(), None);
    app.menu = true;
    app.studio.modern = true;
    app.studio.ready = true;
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 1000.0),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    out.textures_delta.clear();
    assert!(!out.shapes.iter().any(|s| has_mods(&s.shape)));
}

#[test]
fn waveform_cache_remains_bounded_during_repeated_updates() {
    let frames = vec![[0.1, -0.2]; 44100 * 10];
    let mut cache = WaveformCache::default();
    for epoch in 0..64 {
        assert!(cache.get(epoch, &frames).len() <= 2048);
        let pointer = cache.peaks.as_ptr();
        assert_eq!(cache.get(epoch, &frames).as_ptr(), pointer);
    }
    assert!(cache.peaks.capacity() <= 4096);
}

fn setup() -> (App, Audio, Context) {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let ir = Arc::new(Wave {
        frames: vec![[0.1; 2]; 64],
    });
    app.studio_command(Command::Studio(Box::new(Studio::new(Box::new(
        Convolver::new(&ir.frames).unwrap(),
    )))));
    app.studio.ir = Some(ir);
    app.studio.ready = true;
    app.studio.content = true;
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let ctx = Context::default();
    (app, audio, ctx)
}

#[test]
fn silent_sampler_audition_publishes_positions_and_keeps_ui_repainting() {
    let (mut app, mut audio, _) = setup();
    app.studio_command(Command::StudioWave {
        lane: 0,
        wave: Some(Box::new(Wave {
            frames: vec![[0.0; 2]; 100000],
        })),
    });
    app.studio_command(Command::StudioAudition(0));
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let first = app.panel.sampler_playheads[0].unwrap();
    assert!(!app.panel.transport.playing);
    assert!(app.link.as_mut().unwrap().moving(&app.panel));
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert!(app.panel.sampler_playheads[0].unwrap().source > first.source);
    app.studio_command(Command::StudioWave {
        lane: 0,
        wave: None,
    });
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    assert!(app.panel.sampler_playheads[0].is_none());
    assert!(
        app.link.as_mut().unwrap().moving(&app.panel),
        "clear the final drawn head"
    );
}

fn finish(app: &mut App, audio: &mut Audio, ctx: &Context) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.studio.busy() {
        audio.render_i16(&mut vec![0; CALL_FRAMES * 2]);
        app.studio_poll(ctx);
        assert!(
            std::time::Instant::now() < deadline,
            "Studio operation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn unloading_can_be_undone_and_redone_with_the_original_pcm_and_warp_settings() {
    let (mut app, mut audio, ctx) = setup();
    let wave = Arc::new(Wave {
        frames: vec![[0.2, -0.1]; 10000],
    });
    app.studio.waves[0] = Some(wave.clone());
    app.studio.names[0] = "Original.wav".into();
    app.studio.settings.lanes[0].pitch = 7;
    app.studio.settings.lanes[0]
        .warp_markers
        .insert(0.5, 0.6, 0.0, 1.0)
        .unwrap();
    app.sampler_history_reset();
    app.sampler_unload(0);
    app.studio_poll(&ctx);
    assert!(app.studio.waves[0].is_none());
    app.sampler_history_step(&ctx, false);
    assert!(app.studio.history_restoring);
    finish(&mut app, &mut audio, &ctx);
    assert!(Arc::ptr_eq(app.studio.waves[0].as_ref().unwrap(), &wave));
    assert_eq!(app.studio.names[0], "Original.wav");
    assert_eq!(app.studio.settings.lanes[0].pitch, 7);
    assert_eq!(app.studio.settings.lanes[0].warp_markers.points().len(), 1);
    app.studio_command(Command::StudioAudition(0));
    let mut out = vec![0; CALL_FRAMES * 4];
    audio.render_i16(&mut out);
    assert!(
        out.iter().any(|&v| v != 0),
        "undo must restore the actual player sample"
    );
    app.sampler_history_step(&ctx, true);
    finish(&mut app, &mut audio, &ctx);
    assert!(app.studio.waves[0].is_none());
}

#[test]
fn save_and_open_restore_embedded_sample_last_step_and_mix_settings() {
    let (mut app, mut audio, ctx) = setup();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("session.orb338");
    app.studio.settings.lanes[7].bars = 64;
    app.studio.settings.lanes[7].steps[1023] = 117;
    app.studio.settings.return_gain = 0.73;
    app.studio.settings.sampler_bus.gain = 0.42;
    app.studio.settings.sampler_bus.dist = true;
    app.studio.settings.sampler_bus.delay = 0.31;
    app.studio.settings.sampler_bus.reverb_send = 0.24;
    app.studio.settings.lanes[7].reverse = true;
    app.studio.settings.lanes[7].warp = true;
    app.studio.settings.lanes[7].warp_bpm = 137.5;
    app.studio.settings.lanes[7].warp_beats = 8;
    app.studio.settings.lanes[7].warp_grid = true;
    app.studio.settings.lanes[7]
        .warp_markers
        .insert(0.4, 0.6, 0.0, 1.0)
        .unwrap();
    app.studio.waves[7] = Some(Arc::new(Wave {
        frames: vec![[0.2, -0.1]; 1000],
    }));
    app.settings.sidechain.depth = [21, 48];
    app.settings.closed_hat_level = Some(42);
    app.doc.modified = true;
    app.studio_save(&path, &ctx);
    finish(&mut app, &mut audio, &ctx);
    assert!(app.studio.notice.is_none(), "{:?}", app.studio.notice);
    assert!(path.is_file());
    assert!(!app.doc.modified);
    app.studio.settings.lanes[7].steps[1023] = 0;
    app.studio_open(&path, &ctx);
    assert!(app.studio.opening);
    finish(&mut app, &mut audio, &ctx);
    assert_eq!(app.studio.settings.lanes[7].steps[1023], 117);
    assert!(app.studio.settings.lanes[7].reverse);
    assert!(app.studio.settings.lanes[7].warp);
    assert_eq!(app.studio.settings.lanes[7].warp_bpm, 137.5);
    assert_eq!(app.studio.settings.lanes[7].warp_beats, 8);
    assert!(app.studio.settings.lanes[7].warp_grid);
    let marker = app.studio.settings.lanes[7].warp_markers.points()[0];
    assert_eq!((marker.source, marker.target), (0.4, 0.6));
    assert_eq!(app.studio.settings.sampler_bus.gain, 0.42);
    assert!(app.studio.settings.sampler_bus.dist);
    assert_eq!(app.studio.settings.sampler_bus.delay, 0.31);
    assert_eq!(app.studio.settings.sampler_bus.reverb_send, 0.24);
    assert_eq!(app.panel.value("studio.return"), Some(93));
    assert_eq!(app.studio.waves[7].as_ref().unwrap().frames.len(), 1000);
    assert_eq!(app.settings.sidechain.depth, [21, 48]);
    assert_eq!(app.settings.closed_hat_level, Some(42));
    assert_eq!(app.doc.path.as_ref(), Some(&path));
    assert!(!app.doc.modified);
}

#[test]
fn async_save_does_not_clear_edits_made_after_its_snapshot() {
    let (mut app, mut audio, ctx) = setup();
    let folder = tempfile::tempdir().unwrap();
    app.doc.modified = true;
    app.studio_save(&folder.path().join("session.orb338"), &ctx);
    app.settings.sidechain.depth[0] = 13;
    finish(&mut app, &mut audio, &ctx);
    assert!(app.doc.modified);
    assert!(app.after_save.is_none());
}

#[test]
fn successful_save_runs_deferred_new_and_rejected_save_clears_the_deferred_action() {
    let (mut app, mut audio, ctx) = setup();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("session.orb338");
    app.doc.modified = true;
    app.after_save = Some(crate::menu::Then::New);
    app.studio_save(&path, &ctx);
    finish(&mut app, &mut audio, &ctx);
    assert!(path.is_file());
    assert!(!app.studio.content && !app.doc.modified);
    let (_tx, rx) = mpsc::sync_channel(1);
    app.studio.worker = Some(rx);
    app.after_save = Some(crate::menu::Then::Quit);
    app.studio_save(&path, &ctx);
    assert!(app.after_save.is_none());
}

#[test]
fn invalid_open_preserves_dirty_document_and_existing_studio() {
    let (mut app, mut audio, ctx) = setup();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("broken.orb338");
    std::fs::write(&path, b"not a project").unwrap();
    app.studio.settings.return_gain = 0.73;
    app.doc.modified = true;
    let settlement = app.settlement;
    app.studio_open(&path, &ctx);
    finish(&mut app, &mut audio, &ctx);
    assert!(app.studio.notice.is_some());
    assert_eq!(app.studio.settings.return_gain, 0.73);
    assert!(app.doc.modified);
    assert_eq!(app.settlement, settlement);
}

#[test]
fn midi_routes_independently_to_machine_sends_and_sampler_zero() {
    let (mut app, mut audio, ctx) = setup();
    app.panel.set_knob("studio.send4", 1000);
    app.panel.set_knob("sampler8.level", 0);
    app.studio_midi();
    app.studio_poll(&ctx);
    audio.render_i16(&mut vec![0; CALL_FRAMES * 4]);
    let studio = audio.current_mut().frame.studio.as_ref().unwrap();
    assert_eq!(studio.settings.sends, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(studio.settings.lanes[7].gain, 0.0);
    assert_eq!(studio.settings.lanes[6].gain, 0.7);
}

#[test]
fn new_editors_render_in_modern_rack_without_an_audio_device() {
    let ctx = Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    app.studio.modern = true;
    app.studio.ready = true;
    app.studio.sampler_open = true;
    app.studio.reverb_open = true;
    let wave = Arc::new(Wave {
        frames: vec![[0.2, -0.1]; 128],
    });
    app.studio.waves[0] = Some(wave.clone());
    app.studio.ir = Some(wave);
    let mut output = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| app.show(ui),
    );
    output.textures_delta.clear();
    assert!(app.studio.sampler_open && app.studio.reverb_open);
}

#[test]
fn pitch_pickup_crosses_every_semitone_and_rearms_for_external_edits() {
    use crate::midi::{Binding, Cc, Config, Router};
    let mut app = App::new(Panel::new(), None);
    let config = Config {
        enabled: true,
        pickup: true,
        bindings: vec![Binding {
            input: 0,
            channel: 1,
            controller: 74,
            target: "sampler1.pitch".into(),
        }],
        ..Config::default()
    };
    let mut router = Router::default();
    let cc = |value| Cc {
        channel: 1,
        controller: 74,
        value,
    };
    for value in (0..=64).rev().chain(1..=127) {
        router.apply(cc(value), &config, &mut app.panel, 0.0);
        app.studio_midi();
        app.panel.events.clear();
        assert_eq!(
            app.studio.settings.lanes[0].pitch,
            (f64::from(value) / 127.0 * 48.0 - 24.0).round() as i8,
            "CC {value}"
        );
    }
    assert_eq!(app.studio.settings.lanes[0].pitch, 24);
    app.studio.settings.lanes[0].pitch = 12;
    app.studio_mirror();
    router.apply(cc(126), &config, &mut app.panel, 0.0);
    app.studio_midi();
    app.panel.events.clear();
    assert_eq!(app.studio.settings.lanes[0].pitch, 12);
    for value in (0..=95).rev() {
        router.apply(cc(value), &config, &mut app.panel, 0.0);
        app.studio_midi();
        app.panel.events.clear();
    }
    assert_eq!(app.studio.settings.lanes[0].pitch, -24);
}

#[test]
fn compact_compressor_and_reverb_buttons_have_independent_click_regions() {
    let ctx = Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    app.studio.modern = true;
    app.studio.ready = true;
    let mut draw = |events| {
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(crate::graphite_rack::WIDTH, crate::graphite_rack::HEIGHT),
                )),
                events,
                ..Default::default()
            },
            |ui| app.show(ui),
        );
        output.textures_delta.clear();
    };
    draw(Vec::new());
    for x in [966.5, 1010.5] {
        let pos = egui::pos2(x * crate::graphite_rack::WIDTH / 1280.0, 183.5);
        for pressed in [true, false] {
            draw(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ]);
        }
    }
    assert_eq!(app.panel.comp_id, 2);
    assert_eq!(app.studio.settings.sends, [0.3, 0.0, 0.0, 0.0]);
}
