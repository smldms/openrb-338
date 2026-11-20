mod common;

use orb_player::clock::{InputFrame, MASTER, OFF, SLAVE};
use orb_player::{Command, Player};
use std::sync::atomic::Ordering;

#[test]
fn input_offset_adjusts_phase_without_seeking_past_the_first_downbeat() {
    for (offset, faster) in [(-30, true), (30, false)] {
        let (mut player, mut audio) = Player::new(common::tables(), None);
        let connection = player.take_midi_clock().unwrap();
        let shared = &connection.shared;
        shared.mode.store(SLAVE, Ordering::Release);
        shared.input_offset_ms.store(offset, Ordering::Release);
        shared.write(InputFrame {
            at: shared.now(),
            tick: 0,
            period: 20_833_333,
            playing: true,
            generation: 1,
        });
        audio.render_i16(&mut [0; 128]);
        let snapshot = player.snapshot();
        assert!(snapshot.playing);
        assert!(snapshot.tick <= 8, "offset must not skip the downbeat");
        let tempo = audio.current_mut().frame.seq.globals.tempo;
        if faster {
            assert!(tempo > 120_000);
        } else {
            assert!(tempo < 120_000);
        }
    }
}

#[test]
fn external_transport_reaches_engine_and_local_stop_requires_a_new_external_start() {
    let (mut player, mut audio) = Player::new(common::tables(), None);
    let connection = player.take_midi_clock().unwrap();
    let shared = &connection.shared;
    shared.mode.store(SLAVE, Ordering::Release);
    let mut buffer = [0; 128];
    assert!(player.send(Command::Play).is_ok());
    audio.render_i16(&mut buffer);
    assert!(!player.snapshot().playing);
    let mut input = InputFrame {
        at: shared.now(),
        tick: 96,
        period: 20_833_333,
        playing: true,
        generation: 1,
    };
    shared.write(input);
    audio.render_i16(&mut buffer);
    assert!(player.snapshot().playing);
    assert!((96..=104).contains(&player.snapshot().tick));
    input.playing = false;
    shared.write(input);
    audio.render_i16(&mut buffer);
    let paused = player.snapshot().tick;
    assert!(!player.snapshot().playing);
    audio.render_i16(&mut buffer);
    assert_eq!(player.snapshot().tick, paused);
    input.playing = true;
    input.tick = 144;
    input.at = shared.now();
    input.generation += 1;
    shared.write(input);
    audio.render_i16(&mut buffer);
    assert!(player.snapshot().playing);
    assert!(player.send(Command::Stop).is_ok());
    audio.render_i16(&mut buffer);
    assert!(!player.snapshot().playing);
    audio.render_i16(&mut buffer);
    assert!(!player.snapshot().playing);
    shared.inhibit.store(false, Ordering::Release);
    input.generation += 1;
    input.at = shared.now();
    shared.write(input);
    audio.render_i16(&mut buffer);
    assert!(player.snapshot().playing);
    shared.mode.store(OFF, Ordering::Release);
    audio.render_i16(&mut buffer);
    assert!(
        !player.snapshot().playing,
        "disconnect must not start free-running"
    );
    shared.mode.store(MASTER, Ordering::Release);
    assert!(player.send(Command::Locate(0)).is_ok());
    assert!(player.send(Command::Play).is_ok());
    audio.render_i16(&mut [0; 2048]);
    let mut connection = connection;
    let mut messages = Vec::new();
    while let Ok(event) = connection.output.pop() {
        messages.push(event.word);
    }
    assert!(messages.starts_with(&[0xfa, 0xf8]));
}
