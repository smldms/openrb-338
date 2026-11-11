//! Musical constraints, exact pattern destinations, undo and saving through the real player.

use std::sync::Arc;

use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};
use orb_ui::app::App;
use orb_ui::generator::{DrumStyle, Options, Scale, drum_pattern, tb_pair};
use orb_ui::link::Link;
use orb_ui::model::Panel;

fn semitone(b: &[u8]) -> i32 {
    i32::from(b[0]) + 12 * i32::from(b[3]) - 12 * i32::from(b[4])
}

#[test]
fn every_key_scale_and_length_stays_in_key_with_valid_gates_and_consonant_overlap() {
    for root in 0..12 {
        for scale in Scale::ALL {
            for length in 1..=16 {
                for seed in 0..8 {
                    let o = Options {
                        root,
                        scale,
                        ..Options::default()
                    };
                    let pair = tb_pair(&o, length, seed);
                    for bytes in &pair {
                        assert_eq!(bytes.len(), 98);
                        assert_eq!(bytes[1], length);
                        for s in 0..16 {
                            let b = &bytes[2 + 6 * s..8 + 6 * s];
                            assert!(b[0] < 12 && b[1..].iter().all(|&v| v < 2));
                            assert!(!(b[3] != 0 && b[4] != 0));
                            if s >= usize::from(length) {
                                assert_eq!(b, &[0, 0, 0, 0, 0, 1]);
                            }
                            if b[5] == 0 {
                                let relative = (semitone(b) - i32::from(root)).rem_euclid(12) as u8;
                                assert!(scale.intervals().contains(&relative));
                            } else {
                                assert_eq!((b[1], b[2]), (0, 0));
                            }
                            if b[1] != 0 {
                                let next = (s + 1) % usize::from(length);
                                assert_eq!(bytes[2 + next * 6 + 5], 0);
                            }
                        }
                    }
                    for s in 0..usize::from(length) {
                        let a = &pair[0][2 + 6 * s..8 + 6 * s];
                        let b = &pair[1][2 + 6 * s..8 + 6 * s];
                        if a[5] == 0 && b[5] == 0 {
                            assert!(matches!(
                                (semitone(b) - semitone(a)).rem_euclid(12),
                                0 | 3 | 4 | 5 | 7 | 8 | 9
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn controls_have_real_extremes_and_seeds_produce_repeatable_diverse_patterns() {
    let empty = Options {
        density: 0,
        ..Options::default()
    };
    for bytes in tb_pair(&empty, 16, 99) {
        assert!(bytes[2..].as_chunks::<6>().0.iter().all(|b| b[5] == 1));
    }
    let plain = Options {
        density: 100,
        accents: 0,
        slides: 0,
        ..Options::default()
    };
    let pair = tb_pair(&plain, 16, 99);
    assert!(pair[0][2..].as_chunks::<6>().0.iter().all(|b| b[5] == 0));
    assert!(pair.iter().all(|bytes| {
        bytes[2..]
            .as_chunks::<6>()
            .0
            .iter()
            .all(|b| b[1] == 0 && b[2] == 0)
    }));
    assert_eq!(tb_pair(&plain, 16, 99), pair);
    let all = Options {
        density: 100,
        accents: 100,
        slides: 100,
        ..Options::default()
    };
    assert!(
        tb_pair(&all, 16, 99)[0][2..]
            .as_chunks::<6>()
            .0
            .iter()
            .all(|b| b[1] == 1 && b[2] == 1)
    );
    assert_eq!(
        tb_pair(&all, 1, 99)[0][3],
        0,
        "a one-step pattern must not slide forever"
    );
    let diverse: std::collections::HashSet<_> = (0..64).map(|s| tb_pair(&plain, 16, s)).collect();
    assert!(diverse.len() > 60);
}

#[test]
fn linked_lines_have_answers_in_the_gaps_rather_than_duplicate_rhythms() {
    let o = Options::default();
    let mut responses = 0;
    let mut different = 0;
    for seed in 0..64 {
        let pair = tb_pair(&o, 16, seed);
        different += usize::from(pair[0] != pair[1]);
        for s in 0..16 {
            if pair[0][2 + 6 * s + 5] == 1 && pair[1][2 + 6 * s + 5] == 0 {
                responses += 1;
            }
        }
    }
    assert_eq!(different, 64);
    assert!(responses > 100);
}

#[test]
fn both_drum_machines_respect_lengths_velocities_and_hat_choking() {
    for is808 in [true, false] {
        for style in [DrumStyle::Techno, DrumStyle::Broken] {
            for length in 1..=16 {
                for seed in 0..32 {
                    let o = Options {
                        drum_style: style,
                        ..Options::default()
                    };
                    let bytes = drum_pattern(&o, is808, length, seed);
                    assert_eq!((bytes.len(), bytes[1]), (194, length));
                    assert_ne!(bytes[3], 0, "a groove anchors its first kick");
                    for s in 0..16 {
                        let b = &bytes[2 + 12 * s..14 + 12 * s];
                        assert!(b.iter().all(|&v| v <= if is808 { 1 } else { 3 }));
                        if !is808 {
                            assert!(b[0] == 0 || b[0] == 2);
                        }
                        let (closed, open) = if is808 { (11, 10) } else { (8, 9) };
                        assert!(b[closed] == 0 || b[open] == 0);
                        if s >= usize::from(length) {
                            assert!(b.iter().all(|&v| v == 0));
                        }
                    }
                    assert_eq!(bytes, drum_pattern(&o, is808, length, seed));
                }
            }
        }
        let o = Options {
            drum_density: 0,
            ..Options::default()
        };
        assert!(drum_pattern(&o, is808, 16, 1)[2..].iter().all(|&v| v == 0));
    }
}

fn calls(audio: &mut Audio) {
    let mut out = vec![0i16; 2 * CALL_FRAMES];
    audio.render_i16(&mut out);
    audio.render_i16(&mut out);
}

fn app() -> (App, Audio) {
    let samples = Some(rbm::standard::builtin_samples());
    let (link, mut audio) = Link::new(Arc::new(Tables::new()), samples.clone());
    let mut app = App::with_link(Panel::new(), link, None);
    app.samples = samples;
    app.new_now();
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    (app, audio)
}

#[test]
fn a_last_slot_generation_reaches_the_engine_saves_and_undo_restores_the_exact_pattern() {
    let (mut app, mut audio) = app();
    app.panel.press("pattern1.bankD", 0.0);
    app.panel.press("pattern1.pattern8", 0.0);
    app.panel.set_pattern_length(0, 13);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let before = app.link.as_ref().unwrap().patterns().clone();
    app.random_pattern_with_seed(0, 17);
    let expected = app.link.as_ref().unwrap().patterns()[0][31 * 98..32 * 98].to_vec();
    assert_ne!(expected, before[0][31 * 98..32 * 98]);
    assert_eq!(
        &app.link.as_ref().unwrap().patterns()[0][..31 * 98],
        &before[0][..31 * 98]
    );
    assert_eq!(&app.link.as_ref().unwrap().patterns()[1..], &before[1..]);
    calls(&mut audio);
    let bytes = audio.current_mut().frame.devices.channels[0]
        .patterns_mut()
        .unwrap();
    assert_eq!(&bytes[31 * 98..32 * 98], expected);
    let saved = rbs::Song::parse(&app.song_bytes().unwrap()).unwrap();
    assert_eq!(saved.tb303[0].patterns[31].length, 13);
    assert_eq!(saved.tb303[0].patterns[31].steps[0].note, expected[2]);
    assert!(app.doc.modified);
    // Undo still addresses the generated slot after selecting a different pattern.
    app.panel.press("pattern1.pattern2", 0.0);
    assert!(app.undo_random());
    assert_eq!(app.link.as_ref().unwrap().patterns(), &before);
    assert_eq!(app.panel.rows[0].pattern, 25);
    assert!(!app.undo_random());
}

#[test]
fn linked_generation_uses_the_clicked_length_and_tuning_and_undo_restores_both() {
    let (mut app, mut audio) = app();
    app.panel.set_pattern_length(0, 11);
    app.panel.set_pattern_length(1, 7);
    app.panel.press("pattern2.shuffle", 0.0);
    app.panel.set_knob("tb303_1.tune", 350);
    app.panel.set_knob("tb303_2.tune", 650);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let before = app.link.as_ref().unwrap().patterns().clone();
    app.generator.options.link_tb = true;
    app.random_pattern_with_seed(1, 999);
    assert_eq!(app.panel.rows[0].len[0], 7);
    assert_eq!(app.panel.rows[1].len[0], 7);
    assert!(app.panel.rows[0].shuffle[0] && app.panel.rows[1].shuffle[0]);
    assert_eq!(app.panel.knob("tb303_1.tune"), 650);
    assert_eq!(app.panel.knob("tb303_2.tune"), 650);
    calls(&mut audio);
    for row in 0..2 {
        let actual = audio.current_mut().frame.devices.channels[row]
            .patterns_mut()
            .unwrap();
        assert_eq!(
            &actual[..98],
            &app.link.as_ref().unwrap().patterns()[row][..98]
        );
    }
    assert!(app.undo_random());
    assert_eq!(app.link.as_ref().unwrap().patterns(), &before);
    assert_eq!(app.panel.knob("tb303_1.tune"), 350);
    assert_eq!(app.panel.knob("tb303_2.tune"), 650);
}

#[test]
fn drum_generation_is_saved_and_never_changes_instrument_sound_or_transport() {
    let (mut app, mut audio) = app();
    app.panel.press("transport.play", 0.0);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let level = app.panel.value("tr808.bd_level");
    let tune = app.panel.value("tr909.bd_tune");
    for row in 2..4 {
        app.panel.set_pattern_length(row, 9);
        app.random_pattern_with_seed(row, 11);
        calls(&mut audio);
        let actual = audio.current_mut().frame.devices.channels[row]
            .patterns_mut()
            .unwrap();
        assert_eq!(
            &actual[..194],
            &app.link.as_ref().unwrap().patterns()[row][..194]
        );
    }
    let saved = rbs::Song::parse(&app.song_bytes().unwrap()).unwrap();
    assert_eq!(saved.tr808.patterns[0].length, 9);
    assert_eq!(saved.tr909.patterns[0].length, 9);
    assert!(app.panel.transport.playing);
    assert_eq!(app.panel.value("tr808.bd_level"), level);
    assert_eq!(app.panel.value("tr909.bd_tune"), tune);
}

#[test]
fn undo_does_not_overwrite_a_later_edit_or_a_new_song() {
    let (mut app, _audio) = app();
    app.random_pattern_with_seed(0, 1);
    app.panel.set_pattern_length(0, 3);
    assert!(!app.can_undo_random());
    assert!(!app.undo_random());
    app.random_pattern_with_seed(0, 2);
    app.new_now();
    assert!(!app.can_undo_random());
}

#[test]
fn noncanonical_shuffle_can_be_generated_and_undone_without_losing_the_original() {
    let (mut app, _audio) = app();
    let mut original = app.panel.pattern_bytes(0);
    original[0] = 255;
    app.panel.replace_pattern(0, 0, original.clone());
    app.link.as_mut().unwrap().sync(&mut app.panel);
    app.random_pattern_with_seed(0, 77);
    assert!(app.can_undo_random());
    assert!(app.undo_random());
    assert_eq!(&app.link.as_ref().unwrap().patterns()[0][..98], original);
}

#[test]
fn a_missing_drum_device_does_not_panic_or_modify_the_document() {
    let (link, _audio) = Link::new(Arc::new(Tables::new()), None);
    let mut app = App::with_link(Panel::new(), link, None);
    app.random_pattern_with_seed(2, 7);
    assert!(!app.doc.modified);
    assert!(!app.can_undo_random());
}

fn frame(ctx: &egui::Context, app: &mut App, events: Vec<Event>, time: f64) {
    let input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(624.0, 510.0))),
        events,
        time: Some(time),
        ..Default::default()
    };
    let mut output = ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

fn click(ctx: &egui::Context, app: &mut App, x: f32, y: f32, time: f64) {
    let pos = Pos2::new(x, y);
    frame(
        ctx,
        app,
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            },
        ],
        time,
    );
    frame(
        ctx,
        app,
        vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        }],
        time + 0.1,
    );
}

#[test]
fn rack_buttons_are_clickable_and_generator_window_renders() {
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    frame(&ctx, &mut app, vec![], 0.0);
    click(&ctx, &mut app, 68.0, 134.0, 1.0);
    assert!(app.generator.options.link_tb);
    click(&ctx, &mut app, 35.0, 137.0, 2.0);
    assert!(app.can_undo_random());
    assert!(app.panel.synth[0].patterns[0].iter().any(|s| !s.rest));
    assert!(app.panel.synth[1].patterns[0].iter().any(|s| !s.rest));
    app.generator.open = true;
    frame(&ctx, &mut app, vec![], 3.0);
}
