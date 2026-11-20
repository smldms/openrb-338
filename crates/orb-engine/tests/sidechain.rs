use std::sync::Arc;

use orb_engine::frame::{Devices, Engine};
use orb_engine::sidechain::{Config, Ducker, Source};
use orb_engine::song::Device;
use orb_engine::tables::Tables;

#[test]
fn envelope_has_independent_depths_and_returns_to_unity() {
    let mut d = Ducker::default();
    d.set_config(Config {
        enabled: true,
        depth: [100, 0],
        release_ms: 20,
        ..Config::default()
    });
    let mut minimum = 1.0f32;
    for _ in 0..30 {
        let gains = d.gains([1, 0], [true; 2]);
        assert!(gains[1].iter().all(|&g| g == 1.0));
        for &g in &gains[0] {
            minimum = minimum.min(g);
        }
    }
    assert_eq!(minimum, 0.0);
    assert_eq!(d.gains([1, 0], [true; 2]), [[1.0; 64]; 2]);
}

#[test]
fn sources_muting_and_bypass_do_not_replay_stale_kicks() {
    let mut d = Ducker::default();
    assert_eq!(d.gains([1, 1], [true; 2]), [[1.0; 64]; 2]);
    d.set_config(Config {
        enabled: true,
        source: Source::Tr909,
        ..Config::default()
    });
    assert_eq!(d.gains([1, 1], [true; 2]), [[1.0; 64]; 2]);
    assert_eq!(d.gains([2, 1], [true; 2]), [[1.0; 64]; 2]);
    assert_eq!(d.gains([2, 2], [true, false]), [[1.0; 64]; 2]);
    assert_eq!(d.gains([2, 2], [true; 2]), [[1.0; 64]; 2]);
    assert!(d.gains([2, 3], [true; 2])[0].iter().any(|&g| g < 1.0));
}

#[test]
fn repeated_triggers_and_live_bypass_have_bounded_click_free_gain_steps() {
    let mut d = Ducker::default();
    d.set_config(Config {
        enabled: true,
        depth: [255; 2],
        release_ms: 0,
        ..Config::default()
    });
    assert_eq!(d.config().release_ms, 20);
    let mut previous = [1.0f32; 2];
    for block in 0..80 {
        if block == 40 {
            d.set_config(Config::default());
        }
        let gains = d.gains([block / 4, block / 5], [true; 2]);
        for (row, gain_block) in gains.iter().enumerate() {
            for &g in gain_block {
                assert!(g.is_finite() && (0.0..=1.0).contains(&g));
                assert!((g - previous[row]).abs() < 0.01137);
                previous[row] = g;
            }
        }
    }
    assert_eq!(previous, [1.0; 2]);
}

fn engine(instrument: usize, tr909: bool, tb: bool) -> Engine {
    let mut e = Engine::new(
        Arc::new(Tables::new()),
        Devices::implemented(Some(rbm::standard::builtin_samples())),
    );
    let row = if tr909 { 3 } else { 2 };
    let p = e.frame.devices.channels[row].patterns_mut().unwrap();
    p[2 + instrument] = if tr909 { 2 } else { 1 };
    if tb {
        let p = e.frame.devices.channels[0].patterns_mut().unwrap();
        for b in p[2..98].as_chunks_mut::<6>().0 {
            b.copy_from_slice(&[3, 0, 0, 0, 0, 0]);
        }
    }
    e.play();
    e
}

fn audio(e: &mut Engine, calls: usize) -> Vec<i16> {
    let mut out = vec![0; calls * 2048];
    for block in out.chunks_mut(2048) {
        e.render(block, 1024);
    }
    out
}

#[test]
fn disabled_and_zero_depth_are_sample_exact_with_the_legacy_mix() {
    let mut base = engine(1, false, true);
    let expected = audio(&mut base, 25);
    let mut zero = engine(1, false, true);
    zero.frame.sidechain.set_config(Config {
        enabled: true,
        depth: [0; 2],
        ..Config::default()
    });
    assert_eq!(audio(&mut zero, 25), expected);
    let mut duck = engine(1, false, true);
    duck.frame.sidechain.set_config(Config {
        enabled: true,
        ..Config::default()
    });
    assert_ne!(audio(&mut duck, 25), expected);
}

#[test]
fn real_kicks_trigger_both_sources_but_snares_and_muted_kicks_do_not() {
    for tr909 in [false, true] {
        for instrument in [1, 2] {
            let mut e = engine(instrument, tr909, true);
            e.frame.sidechain.set_config(Config {
                enabled: true,
                ..Config::default()
            });
            let mut seen = false;
            for _ in 0..12 {
                audio(&mut e, 1);
                seen |= e.frame.sidechain.reduction()[0] > 0;
            }
            assert_eq!(seen, instrument == 1);
        }
        let mut e = engine(1, tr909, true);
        e.mixer_channel(if tr909 { 3 } else { 2 }, 0, 0);
        e.frame.sidechain.set_config(Config {
            enabled: true,
            ..Config::default()
        });
        audio(&mut e, 8);
        assert_eq!(e.frame.sidechain.reduction(), [0; 2]);
    }
}

#[test]
fn independent_closed_hat_mutes_ch_without_touching_oh_or_other_909_voices() {
    for instrument in [1, 8, 9] {
        let mut base = engine(instrument, true, false);
        let expected = audio(&mut base, 20);
        assert!(expected.iter().any(|&v| v.unsigned_abs() > 1));
        let mut muted = engine(instrument, true, false);
        muted.frame.devices.channels[3].set_closed_hat_level(Some(0));
        let got = audio(&mut muted, 20);
        if instrument == 8 {
            assert!(got.iter().all(|&v| v.unsigned_abs() <= 1));
        } else {
            assert_eq!(got, expected);
        }
    }
    let mut e = engine(8, true, false);
    e.device_param(Device::Tr909, 22, 0);
    e.frame.devices.channels[3].set_closed_hat_level(Some(100));
    assert!(audio(&mut e, 20).iter().any(|&v| v.unsigned_abs() > 1));
}

#[test]
fn selected_closed_hat_triggers_ducking_while_the_kick_does_not() {
    for tr909 in [false, true] {
        let closed_hat = if tr909 { 8 } else { 11 };
        for played in [1, closed_hat] {
            let mut e = engine(played, tr909, true);
            e.frame.sidechain.set_config(Config {
                enabled: true,
                source: if tr909 { Source::Tr909 } else { Source::Tr808 },
                instrument: closed_hat as u8,
                ..Config::default()
            });
            let mut seen = false;
            for _ in 0..12 {
                audio(&mut e, 1);
                seen |= e.frame.sidechain.reduction()[0] > 0;
            }
            assert_eq!(seen, played == closed_hat);
        }
    }
}

#[test]
fn voice_selection_tracks_history_without_missing_the_first_selected_hit() {
    let mut counts = [[0; 12]; 2];
    counts[1][8] = 12;
    let mut d = Ducker::default();
    assert_eq!(d.gains_for_voices(counts, [true; 2]), [[1.0; 64]; 2]);
    d.set_config(Config {
        enabled: true,
        source: Source::Tr909,
        instrument: 8,
        ..Config::default()
    });
    assert_eq!(d.gains_for_voices(counts, [true; 2]), [[1.0; 64]; 2]);
    counts[0][8] += 1;
    assert_eq!(d.gains_for_voices(counts, [true; 2]), [[1.0; 64]; 2]);
    counts[1][8] += 1;
    assert!(
        d.gains_for_voices(counts, [true; 2])[0]
            .iter()
            .any(|&g| g < 1.0)
    );

    let mut fresh = Ducker::default();
    fresh.set_config(d.config());
    let mut first = [[0; 12]; 2];
    first[1][8] = 1;
    assert!(
        fresh.gains_for_voices(first, [true; 2])[0]
            .iter()
            .any(|&g| g < 1.0)
    );
    assert_eq!(
        Config {
            instrument: 0,
            ..Config::default()
        }
        .sanitized()
        .instrument,
        1
    );
    assert_eq!(
        Config {
            instrument: 255,
            ..Config::default()
        }
        .sanitized()
        .instrument,
        11
    );
}
