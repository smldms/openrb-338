use super::*;

#[test]
fn output_clock_is_24_ppqn_without_drift_across_audio_blocks() {
    for tempo in [20_000u32, 97_300, 120_000, 140_000, 400_000] {
        let (mut connection, mut clock) = bridge();
        connection.shared.mode.store(MASTER, Ordering::Release);
        clock.begin_pull(0);
        let base = clock.base_time;
        let mut pulses = Vec::new();
        let total = 44100 * 20;
        let mut frame = 0;
        while frame < total {
            let count = 64.min(total - frame);
            clock.master(frame as u64, count, true, tempo, 0);
            while let Ok(event) = connection.output.pop() {
                if event.word == 0xf8 {
                    pulses.push(event.at);
                }
            }
            frame += count;
        }
        let expected = (f64::from(tempo) / 1000.0 * 24.0 / 60.0 * 20.0).ceil() as usize;
        assert_eq!(pulses.len(), expected, "tempo {tempo}");
        for (index, at) in pulses.iter().enumerate() {
            let expected = index as f64 * 60_000_000_000_000.0 / (f64::from(tempo) * 24.0);
            assert!((*at as f64 - base as f64 - expected).abs() < 23_000.0);
        }
        clock.master(total as u64, 64, false, tempo, 0);
        assert_eq!(connection.output.pop().unwrap().word, 0xfc);
        assert!(connection.output.pop().is_err());
        assert_eq!(connection.shared.overflow.load(Ordering::Relaxed), 0);
    }
}

#[test]
fn output_locate_sends_song_position_and_continue_before_clock() {
    let (mut connection, mut clock) = bridge();
    connection.shared.mode.store(MASTER, Ordering::Release);
    clock.begin_pull(0);
    clock.master(0, 64, true, 120_000, 768);
    assert_eq!(connection.output.pop().unwrap().word, 0xf2 | (16 << 8));
    assert_eq!(connection.output.pop().unwrap().word, 0xfb);
    assert_eq!(connection.output.pop().unwrap().word, 0xf8);
    clock.resync = true;
    clock.master(64, 64, true, 120_000, 0);
    assert_eq!(connection.output.pop().unwrap().word, 0xfc);
    assert_eq!(connection.output.pop().unwrap().word, 0xfa);
    assert_eq!(connection.output.pop().unwrap().word, 0xf8);
}

#[test]
fn input_start_waits_for_clock_stop_holds_and_continue_resumes() {
    let shared = Shared::default();
    let mut follower = Follower::default();
    follower.receive(0xfa, 0, &shared);
    follower.timeout(1_000_000, &shared);
    assert!(!shared.lost.load(Ordering::Relaxed));
    assert!(!shared.read().unwrap().playing);
    for pulse in 0..25u64 {
        follower.receive(0xf8, pulse * 20_833_333 + 2_000_000, &shared);
    }
    let state = shared.read().unwrap();
    assert_eq!(state.tick, 192);
    assert!(state.playing);
    assert!((i64::from(shared.bpm.load(Ordering::Relaxed)) - 120_000).abs() <= 1);
    follower.receive(0xfc, 510_000_000, &shared);
    follower.receive(0xf8, 522_833_325, &shared);
    assert!(!shared.read().unwrap().playing);
    assert_eq!(shared.read().unwrap().tick, 192);
    follower.receive(0xfb, 530_000_000, &shared);
    assert!(!shared.read().unwrap().playing);
    follower.receive(0xf8, 543_666_658, &shared);
    assert_eq!(shared.read().unwrap().tick, 200);
    assert!(shared.read().unwrap().playing);
    follower.timeout(1_100_000_000, &shared);
    assert!(!shared.read().unwrap().playing);
    assert!(shared.lost.load(Ordering::Relaxed));
    follower.receive(0xf8, 1_200_000_000, &shared);
    assert!(
        !shared.read().unwrap().playing,
        "clock alone cannot defeat the safety stop"
    );
}

#[test]
fn input_spp_uses_sixteenth_notes_and_ignores_cc_active_sensing_and_bad_data() {
    let shared = Shared::default();
    let mut follower = Follower::default();
    let spp = 0xf2 | (45 << 8) | (1 << 16);
    follower.receive(spp, 10, &shared);
    follower.receive(0xfb, 20, &shared);
    follower.receive(0xf8, 30, &shared);
    assert_eq!(shared.read().unwrap().tick, 173 * 48);
    for word in [0xfe, 0xff, 0x7f07b0, 0x0080f2] {
        follower.receive(word, 40, &shared);
    }
    assert_eq!(shared.read().unwrap().tick, 173 * 48);
    follower.receive(0xfa, 50, &shared);
    follower.receive(0xf8, 60, &shared);
    assert_eq!(shared.read().unwrap().tick, 0);
}

#[test]
fn input_tempo_averages_timestamp_jitter_without_losing_pulses() {
    let shared = Shared::default();
    let mut follower = Follower::default();
    follower.receive(0xfa, 0, &shared);
    for pulse in 0..240u64 {
        follower.receive(
            0xf8,
            1_000_000 + pulse * 20_833_333 + (pulse % 2) * 500_000,
            &shared,
        );
    }
    assert_eq!(shared.read().unwrap().tick, 239 * 8);
    assert!((i64::from(shared.bpm.load(Ordering::Relaxed)) - 120_000).abs() <= 1);
}
