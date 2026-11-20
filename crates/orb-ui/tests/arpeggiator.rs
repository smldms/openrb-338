use std::sync::Arc;

use orb_engine::tables::Tables;
use orb_player::{Audio, CALL_FRAMES};
use orb_ui::app::App;
use orb_ui::arpeggiator::{Chord, Options, Order, Source, render};
use orb_ui::link::Link;
use orb_ui::model::Panel;

fn empty() -> Vec<u8> {
    let mut bytes = vec![0; 98];
    bytes[1] = 16;
    for b in bytes[2..].as_chunks_mut::<6>().0 {
        b[5] = 1;
    }
    bytes
}

fn pitch(b: &[u8]) -> i32 {
    i32::from(b[0]) + 12 * i32::from(b[3]) - 12 * i32::from(b[4])
}

fn pitches(bytes: &[u8]) -> Vec<i32> {
    bytes[2..]
        .as_chunks::<6>()
        .0
        .iter()
        .take(usize::from(bytes[1]))
        .filter(|b| b[5] == 0)
        .map(|b| pitch(b))
        .collect()
}

#[test]
fn directions_bounce_without_repeating_endpoints() {
    for (order, expected) in [
        (Order::Up, vec![0, 4, 7, 0, 4, 7, 0, 4]),
        (Order::Down, vec![7, 4, 0, 7, 4, 0, 7, 4]),
        (Order::UpDown, vec![0, 4, 7, 4, 0, 4, 7, 4]),
        (Order::DownUp, vec![7, 4, 0, 4, 7, 4, 0, 4]),
    ] {
        let o = Options {
            root: 0,
            chord: Chord::Major,
            order,
            length: 8,
            ..Options::default()
        };
        assert_eq!(pitches(&render(&o, &empty()).unwrap().bytes), expected);
    }
}

#[test]
fn pattern_source_deduplicates_preserves_order_and_ignores_rests_and_hidden_tail() {
    let mut bytes = empty();
    bytes[1] = 5;
    for (step, note, up, down, rest) in [
        (0, 7, 0, 0, 0),
        (1, 0, 0, 1, 0),
        (2, 7, 0, 0, 0),
        (3, 12, 1, 0, 0),
        (4, 3, 0, 0, 1),
        (5, 5, 0, 0, 0),
    ] {
        bytes[2 + step * 6..8 + step * 6].copy_from_slice(&[note, 0, 0, up, down, rest]);
    }
    let o = Options {
        source: Source::Pattern,
        order: Order::AsWritten,
        length: 6,
        ..Options::default()
    };
    let result = render(&o, &bytes).unwrap();
    assert_eq!(result.notes, vec![7, -12, 24]);
    assert_eq!(pitches(&result.bytes), vec![7, -12, 24, 7, -12, 24]);
}

#[test]
fn empty_source_and_invalid_notes_are_reported_without_fabricating_notes() {
    let o = Options {
        source: Source::Pattern,
        ..Options::default()
    };
    assert!(render(&o, &empty()).is_err());
    assert!(render(&o, &[0; 12]).is_err());
    let mut bytes = empty();
    bytes[2] = 13;
    bytes[7] = 0;
    assert!(render(&o, &bytes).is_err());
    assert!(render(&Options::default(), &empty()).is_ok());
}

#[test]
fn keys_chords_orders_octaves_and_lengths_keep_harmony_and_valid_tb_encoding() {
    for root in 0..12 {
        for chord in Chord::ALL {
            for order in Order::ALL {
                for octaves in 1..=3 {
                    for length in [1, 7, 16] {
                        let o = Options {
                            root,
                            chord,
                            order,
                            octaves,
                            length,
                            slides: true,
                            ..Options::default()
                        };
                        let p = render(&o, &empty()).unwrap();
                        assert_eq!(p.bytes.len(), 98);
                        for (s, b) in p.bytes[2..].as_chunks::<6>().0.iter().enumerate() {
                            assert!(b[0] <= 12 && b[1..].iter().all(|&v| v <= 1));
                            assert!(!(b[3] == 1 && b[4] == 1));
                            if s >= usize::from(length) {
                                assert_eq!(b, &[0, 0, 0, 0, 0, 1]);
                            } else {
                                assert!((-12..=24).contains(&pitch(b)));
                                assert!(
                                    chord
                                        .intervals()
                                        .contains(&(pitch(b) - i32::from(root)).rem_euclid(12))
                                );
                                assert_eq!(b[1], u8::from(length > 1));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rhythm_accents_offsets_and_random_are_repeatable() {
    for every in [1, 2, 4, 8] {
        let o = Options {
            every,
            slides: true,
            ..Options::default()
        };
        let p = render(&o, &empty()).unwrap();
        for (s, b) in p.bytes[2..].as_chunks::<6>().0.iter().enumerate() {
            assert_eq!(b[5], u8::from(s % usize::from(every) != 0));
            assert_eq!(
                b[2],
                u8::from(s % usize::from(every) == 0 && (s / usize::from(every)) % 4 == 0)
            );
            if every > 1 {
                assert_eq!(b[1], 0);
            }
        }
    }
    let o = Options {
        order: Order::Random,
        accent_every: 0,
        ..Options::default()
    };
    let p = render(&o, &empty()).unwrap();
    assert_eq!(p, render(&o, &empty()).unwrap());
    assert!(pitches(&p.bytes).windows(2).all(|w| w[0] != w[1]));
    assert!(p.bytes[2..].as_chunks::<6>().0.iter().all(|b| b[2] == 0));
    let offset = Options {
        root: 0,
        chord: Chord::Major,
        offset: 2,
        ..Options::default()
    };
    assert_eq!(pitches(&render(&offset, &empty()).unwrap().bytes)[0], 7);
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
fn preview_apply_engine_save_and_undo_use_the_exact_slot_and_preserve_other_machines() {
    let (mut app, mut audio) = app();
    app.panel.press("pattern1.bankD", 0.0);
    app.panel.press("pattern1.pattern8", 0.0);
    app.panel.set_knob("tb303_1.tune", 350);
    app.panel.set_knob("tb303_2.tune", 650);
    app.panel.press("transport.play", 0.0);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    calls(&mut audio);
    app.link.as_mut().unwrap().sync(&mut app.panel);
    let before = app.link.as_ref().unwrap().patterns().clone();
    app.generator.options.link_tb = true;
    app.arpeggiator.options[0].length = 11;
    let preview = app.arpeggiator_preview(0).unwrap();
    assert_eq!(app.link.as_ref().unwrap().patterns(), &before);
    app.apply_arpeggiator(0).unwrap();
    let after = app.link.as_ref().unwrap().patterns();
    assert_eq!(&after[0][31 * 98..32 * 98], preview.bytes);
    assert_eq!(&after[0][..31 * 98], &before[0][..31 * 98]);
    assert_eq!(&after[1..], &before[1..]);
    assert_eq!(app.panel.knob("tb303_1.tune"), 350);
    assert_eq!(app.panel.knob("tb303_2.tune"), 650);
    assert!(app.panel.transport.playing);
    calls(&mut audio);
    let actual = audio.current_mut().frame.devices.channels[0]
        .patterns_mut()
        .unwrap();
    assert_eq!(&actual[31 * 98..32 * 98], preview.bytes);
    let saved = rbs::Song::parse(&app.song_bytes().unwrap()).unwrap();
    assert_eq!(saved.tb303[0].patterns[31].length, 11);
    assert_eq!(saved.tb303[0].patterns[31].steps[0].note, preview.bytes[2]);
    assert!(app.doc.modified);
    app.panel.press("pattern1.pattern2", 0.0);
    assert!(app.undo_arpeggiator(0));
    assert_eq!(app.panel.rows[0].pattern, 25);
    assert_eq!(app.link.as_ref().unwrap().patterns(), &before);
}

#[test]
fn independent_undo_refuses_later_edits_and_is_cleared_on_new_song() {
    let (mut app, _audio) = app();
    app.arpeggiator.options[1].root = 9;
    app.apply_arpeggiator(0).unwrap();
    app.apply_arpeggiator(1).unwrap();
    assert!(app.can_undo_arpeggiator(0) && app.can_undo_arpeggiator(1));
    assert!(app.undo_arpeggiator(1));
    assert!(app.can_undo_arpeggiator(0));
    app.panel.set_pattern_length(0, 3);
    assert!(!app.undo_arpeggiator(0));
    app.apply_arpeggiator(0).unwrap();
    app.new_now();
    assert!(!app.can_undo_arpeggiator(0));
}

#[test]
fn undo_does_not_overwrite_a_pending_edit_after_selecting_another_slot() {
    let (mut app, _audio) = app();
    app.apply_arpeggiator(0).unwrap();
    app.panel.set_pattern_length(0, 3);
    app.panel.press("pattern1.pattern2", 0.0);
    assert!(!app.undo_arpeggiator(0));
    assert_eq!(app.link.as_ref().unwrap().patterns()[0][1], 3);
}

#[test]
fn a_single_source_note_can_bounce_or_randomize_without_division_by_zero() {
    let mut bytes = empty();
    bytes[2..8].copy_from_slice(&[9, 0, 0, 0, 1, 0]);
    for order in Order::ALL {
        let o = Options {
            source: Source::Pattern,
            order,
            ..Options::default()
        };
        assert_eq!(pitches(&render(&o, &bytes).unwrap().bytes), vec![-3; 16]);
    }
}

#[test]
fn empty_pattern_errors_leave_document_and_patterns_untouched() {
    let mut app = App::new(Panel::new(), None);
    let before = app.panel.pattern_bytes(0);
    app.arpeggiator.options[0].source = Source::Pattern;
    assert!(app.apply_arpeggiator(0).is_err());
    assert_eq!(app.panel.pattern_bytes(0), before);
    assert!(!app.doc.modified);
    assert!(!app.can_undo_arpeggiator(0));
    assert!(app.apply_arpeggiator(2).is_err());
}

#[test]
fn undo_retains_noncanonical_original_shuffle_bytes() {
    let (mut app, _audio) = app();
    let mut original = app.panel.pattern_bytes(0);
    original[0] = 255;
    app.panel.replace_pattern(0, 0, original.clone());
    app.link.as_mut().unwrap().sync(&mut app.panel);
    app.apply_arpeggiator(0).unwrap();
    assert!(app.undo_arpeggiator(0));
    assert_eq!(&app.link.as_ref().unwrap().patterns()[0][..98], original);
}

#[test]
fn rack_arp_button_applies_to_the_matching_tb_without_opening_an_editor() {
    use eframe::egui::{self, Event, Modifiers, PointerButton, Pos2, RawInput, Rect, Vec2};
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    let before = [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)];
    let mut time = 0.0;
    let mut frame = |events| {
        time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(624.0, 510.0))),
            events,
            time: Some(time),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
    };
    frame(vec![]);
    let pos = Pos2::new(124.0, 205.5);
    frame(vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        },
    ]);
    frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::default(),
    }]);
    assert_eq!(app.arpeggiator.open, [false, false]);
    assert_eq!(app.panel.pattern_bytes(0), before[0]);
    assert_ne!(app.panel.pattern_bytes(1), before[1]);
    assert!(app.doc.modified);
    assert!(app.undo_arpeggiator(1));
    assert_eq!(app.panel.pattern_bytes(1), before[1]);
}

#[test]
fn both_windows_render_with_unique_controls_and_without_modifying_patterns() {
    use eframe::egui::{self, Pos2, RawInput, Rect, Vec2};
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    let mut app = App::new(Panel::new(), None);
    let before = [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)];
    app.open_arpeggiator(0);
    app.open_arpeggiator(1);
    for source in [Source::Chord, Source::Pattern] {
        app.arpeggiator.options[1].source = source;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 900.0))),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
    }
    assert_eq!(
        [app.panel.pattern_bytes(0), app.panel.pattern_bytes(1)],
        before
    );
    assert!(!app.doc.modified);
}
