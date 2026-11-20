use super::*;

#[test]
fn playheads_follow_the_latest_voice_reverse_warp_loop_and_stop() {
    let (mut sampler, mut lanes) = setup(Mode::OneShot);
    assert_eq!(sampler.playheads(&lanes), [None; LANES]);
    let render = |sampler: &mut Sampler, lanes: &[Lane; LANES]| {
        sampler.process(
            lanes,
            0.0,
            120.0 * 192.0 / (60.0 * 44100.0),
            false,
            &mut [[0.0; 64]; 2],
            &mut [[0.0; 64]; 2],
        );
    };
    sampler.audition(0);
    render(&mut sampler, &lanes);
    let first = sampler.playheads(&lanes)[0].unwrap();
    assert!((f64::from(first.source) - 64.0 / 50000.0).abs() < 1e-7);
    render(&mut sampler, &lanes);
    assert!(sampler.playheads(&lanes)[0].unwrap().source > first.source);
    sampler.audition(0);
    render(&mut sampler, &lanes);
    assert_eq!(sampler.playheads(&lanes)[0], Some(first));
    sampler.stop();
    assert_eq!(sampler.playheads(&lanes), [None; LANES]);
    lanes[0].reverse = true;
    lanes[0].warp = true;
    lanes[0].warp_grid = true;
    lanes[0].warp_markers.insert(0.5, 0.75, 0.0, 1.0).unwrap();
    sampler.audition(0);
    render(&mut sampler, &lanes);
    let reverse = sampler.playheads(&lanes)[0].unwrap();
    assert!(reverse.source > 0.99 && reverse.musical > 0.99);
    render(&mut sampler, &lanes);
    assert!(sampler.playheads(&lanes)[0].unwrap().source < reverse.source);
    lanes[0].warp = false;
    render(&mut sampler, &lanes);
    let head = sampler.playheads(&lanes)[0].unwrap();
    let source = lanes[0]
        .warp_markers
        .map(f64::from(head.musical), 0.0, 1.0)
        .0;
    assert!((source - f64::from(head.source)).abs() < 1e-6);
    lanes[0].mode = Mode::Loop;
    sampler.voices[0][0].position = 50001.0;
    let head = sampler.playheads(&lanes)[0].unwrap();
    assert!((0.0..=1.0).contains(&head.source));
    assert!((0.0..=1.0).contains(&head.musical));
    sampler.replace(0, None);
    assert_eq!(sampler.playheads(&lanes), [None; LANES]);
}

#[test]
fn warp_markers_keep_order_endpoints_and_survive_serialization() {
    let mut map = WarpMarkers::default();
    assert_eq!(map.insert(0.4, 0.6, 0.1, 0.9), Some(0));
    assert_eq!(map.insert(0.7, 0.8, 0.1, 0.9), Some(1));
    assert_eq!(map.map(0.0, 0.1, 0.9).0, 0.1);
    assert_eq!(map.map(0.6, 0.1, 0.9).0, 0.4);
    assert!((map.map(1.0, 0.1, 0.9).0 - 0.9).abs() < 1e-12);
    assert_eq!(map.insert(0.8, 0.5, 0.1, 0.9), None);
    map.move_target(0, 1.0);
    assert!(map.points()[0].target < map.points()[1].target);
    map.sanitize(0.1, 0.9);
    assert_eq!(map.points().len(), 2);
    let saved = serde_json::to_string(&map).unwrap();
    assert_eq!(serde_json::from_str::<WarpMarkers>(&saved).unwrap(), map);
    map.markers[0].source = f64::NAN;
    map.count = 255;
    map.sanitize(0.1, 0.9);
    assert_eq!(map.points().len(), 1);
    map.remove(0);
    assert!(map.points().is_empty());
    let old: Lane = serde_json::from_str("{}").unwrap();
    assert!(old.warp_markers.points().is_empty());
}

#[test]
fn beat_fit_duration_follows_beats_even_with_no_anchors_or_extremely_short_audio() {
    for (frames, beats) in [(50000, 4), (50000, 8), (2, 1), (2, 16)] {
        let (mut sampler, mut lanes) = setup(Mode::OneShot);
        sampler.replace(
            0,
            Some(Box::new(Wave {
                frames: vec![[0.2; 2]; frames],
            })),
        );
        lanes[0].warp_beats = beats;
        lanes[0].fit_warp(Some(frames));
        sampler.audition(0);
        let expected = 22050 * usize::from(beats);
        for _ in 0..expected.div_ceil(64) + 1 {
            sampler.process(
                &lanes,
                0.0,
                120.0 * 192.0 / (60.0 * 44100.0),
                false,
                &mut [[0.0; 64]; 2],
                &mut [[0.0; 64]; 2],
            );
        }
        assert!((sampler.voices[0][0].age as f64 - expected as f64).abs() <= 2.0);
    }
}

#[test]
fn warp_markers_move_audio_regions_preserve_pitch_and_follow_tempo_and_reverse() {
    for reverse in [false, true] {
        for bpm in [90.0, 120.0, 180.0] {
            let mut sampler = Sampler::default();
            sampler.replace(
                0,
                Some(Box::new(Wave {
                    frames: (0..88200)
                        .map(|n| {
                            let hz = if n < 44100 { 440.0 } else { 880.0 };
                            let v = (n as f64 * std::f64::consts::TAU * hz / 44100.0).sin() as f32;
                            [v, -v]
                        })
                        .collect(),
                })),
            );
            let mut lanes = std::array::from_fn(|_| Lane::default());
            lanes[0].warp = true;
            lanes[0].reverse = reverse;
            lanes[0].warp_markers.insert(0.5, 0.75, 0.0, 1.0).unwrap();
            sampler.audition(0);
            let length = (44100.0 * 240.0 / bpm) as usize;
            let mut audio = Vec::new();
            while audio.len() < length + 64 {
                let mut out = [[0.0; 64]; 2];
                sampler.process(
                    &lanes,
                    0.0,
                    bpm * 192.0 / (60.0 * 44100.0),
                    false,
                    &mut out,
                    &mut [[0.0; 64]; 2],
                );
                for (&l, &r) in out[0].iter().zip(&out[1]) {
                    assert!(l.is_finite() && (l + r).abs() < 1e-10);
                }
                audio.extend(out[0]);
            }
            assert!((sampler.voices[0][0].age as f64 - length as f64).abs() <= 2.0);
            let frequency = |a: f64, b: f64| {
                let slice = &audio[(a * length as f64) as usize..(b * length as f64) as usize];
                let crossings = slice
                    .windows(2)
                    .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
                    .count();
                crossings as f64 * 44100.0 / slice.len() as f64
            };
            let (early, late) = if reverse {
                (880.0, 440.0)
            } else {
                (440.0, 880.0)
            };
            assert!((frequency(0.08, 0.2) - early).abs() < 15.0);
            assert!((frequency(0.8, 0.95) - late).abs() < 15.0);
            // The first region now occupies 75% rather than 50% of the musical duration.
            assert!((frequency(0.55, 0.7) - 440.0).abs() < 15.0);
        }
    }
}

fn setup(mode: Mode) -> (Sampler, [Lane; LANES]) {
    let mut sampler = Sampler::default();
    sampler.replace(
        0,
        Some(Box::new(Wave {
            frames: vec![[0.2, -0.1]; 50000],
        })),
    );
    let mut lanes = std::array::from_fn(|_| Lane::default());
    lanes[0].mode = mode;
    lanes[0].attack_ms = 0.1;
    lanes[0].release_ms = 1.0;
    (sampler, lanes)
}

fn warped_tone(warp: bool, target_bpm: f64, pitch: i8) -> (Vec<f64>, usize) {
    let mut sampler = Sampler::default();
    sampler.replace(
        0,
        Some(Box::new(Wave {
            frames: (0..44100)
                .map(|n| {
                    let value = (n as f64 * std::f64::consts::TAU * 440.0 / 44100.0).sin() as f32;
                    [value, -value]
                })
                .collect(),
        })),
    );
    let mut lanes = std::array::from_fn(|_| Lane::default());
    lanes[0].warp = warp;
    lanes[0].warp_bpm = 120.0;
    lanes[0].pitch = pitch;
    sampler.audition(0);
    let mut audio = Vec::new();
    for _ in 0..(4 * 44100 / 64 + 1) {
        let mut out = [[0.0; 64]; 2];
        sampler.process(
            &lanes,
            0.0,
            target_bpm * 192.0 / (60.0 * 44100.0),
            false,
            &mut out,
            &mut [[0.0; 64]; 2],
        );
        assert!(out.iter().flatten().all(|v| v.is_finite()));
        for (left, right) in out[0].iter().zip(&out[1]) {
            assert!((left + right).abs() < 1e-10);
        }
        audio.extend(out[0]);
        if !sampler.voices[0][0].active {
            break;
        }
    }
    (audio, sampler.voices[0][0].age)
}

fn crossing_frequency(audio: &[f64]) -> f64 {
    let middle = &audio[4410..audio.len() - 2205];
    let crossings = middle
        .windows(2)
        .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
        .count();
    crossings as f64 * 44100.0 / middle.len() as f64
}

#[test]
fn time_warp_changes_duration_preserves_pitch_and_stereo() {
    for bpm in [60.0, 90.0, 160.0, 240.0] {
        let (audio, age) = warped_tone(true, bpm, 0);
        let expected = 44100.0 * 120.0 / bpm;
        assert!((age as f64 - expected).abs() <= 1.0, "{bpm}: age={age}");
        let frequency = crossing_frequency(&audio);
        assert!((frequency - 440.0).abs() < 6.0, "{bpm} BPM: {frequency} Hz");
    }
}

#[test]
fn warp_transpose_is_independent_of_duration_and_unity_is_exact() {
    let (audio, age) = warped_tone(true, 90.0, 12);
    assert!((age as f64 - 58800.0).abs() <= 1.0);
    let frequency = crossing_frequency(&audio);
    assert!((frequency - 880.0).abs() < 8.0, "{frequency} Hz");
    assert_eq!(warped_tone(false, 120.0, 0), warped_tone(true, 120.0, 0));
    assert_eq!(warped_tone(false, 90.0, 12).1, 22050);
}

#[test]
fn warped_reverse_loops_honor_trim_and_follow_tempo_changes() {
    let mut sampler = Sampler::default();
    sampler.replace(
        0,
        Some(Box::new(Wave {
            frames: (0..2048)
                .map(|n| {
                    let value = if (512..1536).contains(&n) {
                        (n as f64 * 0.1).sin() as f32 * 0.2
                    } else {
                        42.0
                    };
                    [value, -value]
                })
                .collect(),
        })),
    );
    let mut lanes = std::array::from_fn(|_| Lane::default());
    lanes[0].warp = true;
    lanes[0].warp_bpm = 100.0;
    lanes[0].mode = Mode::Loop;
    lanes[0].reverse = true;
    lanes[0].start = 0.25;
    lanes[0].end = 0.75;
    sampler.audition(0);
    for _ in 0..32 {
        let mut out = [[0.0; 64]; 2];
        sampler.process(
            &lanes,
            0.0,
            150.0 * 192.0 / (60.0 * 44100.0),
            false,
            &mut out,
            &mut [[0.0; 64]; 2],
        );
        assert!(
            out.iter()
                .flatten()
                .all(|v| v.is_finite() && v.abs() < 0.25)
        );
    }
    let phase = sampler.voices[0][0].position.rem_euclid(1024.0);
    assert!(phase.min(1024.0 - phase) < 1e-8, "loop phase: {phase}");
    sampler.process(
        &lanes,
        0.0,
        90.0 * 192.0 / (60.0 * 44100.0),
        false,
        &mut [[0.0; 64]; 2],
        &mut [[0.0; 64]; 2],
    );
    assert!((sampler.voices[0][0].position - 57.6).abs() < 1e-8);
    assert!(sampler.voices[0][0].active);
    assert_eq!(lanes[0].fitted_warp_bpm(88200), Some(240.0));
}

#[test]
fn release_time_warp_32_voice_performance() {
    if cfg!(debug_assertions) {
        return;
    }
    let mut sampler = Sampler::default();
    let mut lanes = std::array::from_fn(|_| Lane::default());
    for (i, lane) in lanes.iter_mut().enumerate() {
        lane.warp = true;
        lane.warp_bpm = 120.0;
        lane.pitch = [-12, -5, 0, 7, 12, 0, 3, -7][i];
        for marker in 1..=MAX_WARP_MARKERS {
            let source = marker as f64 / (MAX_WARP_MARKERS + 1) as f64;
            let target = source + if marker % 2 == 0 { 0.005 } else { -0.005 };
            lane.warp_markers.insert(source, target, 0.0, 1.0).unwrap();
        }
        sampler.replace(
            i,
            Some(Box::new(Wave {
                frames: (0..88200)
                    .map(|n| {
                        let t = n as f64 / 44100.0;
                        let v = ((t * std::f64::consts::TAU * 330.0).sin() * 0.05
                            + (t * std::f64::consts::TAU * 731.0).sin() * 0.025)
                            as f32;
                        [v, -v]
                    })
                    .collect(),
            })),
        );
    }
    let ticks = 160.0 * 192.0 / (60.0 * 44100.0);
    for _ in 0..4 {
        for i in 0..8 {
            sampler.audition(i);
        }
        sampler.process(
            &lanes,
            0.0,
            ticks,
            false,
            &mut [[0.0; 64]; 2],
            &mut [[0.0; 64]; 2],
        );
    }
    let start = std::time::Instant::now();
    for _ in 0..690 {
        let mut out = [[0.0; 64]; 2];
        sampler.process(&lanes, 0.0, ticks, false, &mut out, &mut [[0.0; 64]; 2]);
        assert!(out.iter().flatten().all(|v| v.is_finite()));
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!("WARP_PERFORMANCE: 1.001 seconds audio / {elapsed:.3} seconds CPU / 32 voices");
    assert!(
        elapsed < 4.0,
        "Warp render unexpectedly slow: {elapsed:.3} seconds"
    );
}

#[test]
fn old_projects_default_warp_to_off_and_controls_are_sanitized() {
    let mut old = serde_json::to_value(Lane::default()).unwrap();
    for field in [
        "warp",
        "warp_bpm",
        "warp_beats",
        "warp_grid",
        "warp_markers",
    ] {
        old.as_object_mut().unwrap().remove(field);
    }
    let mut lane: Lane = serde_json::from_value(old).unwrap();
    assert!(!lane.warp);
    assert_eq!((lane.warp_bpm, lane.warp_beats), (120.0, 4));
    lane.warp_bpm = f64::NAN;
    lane.warp_beats = 0;
    lane.sanitize();
    assert_eq!((lane.warp_bpm, lane.warp_beats), (120.0, 1));
}

fn block(sampler: &mut Sampler, lanes: &[Lane; LANES], tick: f64, playing: bool) -> [[f64; 64]; 2] {
    let mut out = [[0.0; 64]; 2];
    sampler.process(lanes, tick, 0.01, playing, &mut out, &mut [[0.0; 64]; 2]);
    out
}

#[test]
fn older_lane_settings_default_new_modulation_to_bypass() {
    let mut old = serde_json::to_value(Lane::default()).unwrap();
    let map = old.as_object_mut().unwrap();
    for key in [
        "gater_source",
        "gater_depth",
        "gater_attack_ms",
        "gater_release_ms",
        "sidechain",
    ] {
        map.remove(key);
    }
    let lane: Lane = serde_json::from_value(old).unwrap();
    assert_eq!(lane.gater_source, 0);
    assert!(!lane.sidechain);
    assert_eq!(lane.steps.len(), STEPS);
}

#[test]
fn auto_cut_is_monophonic_one_shot_overlaps_and_stop_releases_every_mode() {
    for mode in [Mode::OneShot, Mode::AutoCut, Mode::Gate, Mode::Loop] {
        let (mut sampler, lanes) = setup(mode);
        for _ in 0..3 {
            sampler.audition(0);
            block(&mut sampler, &lanes, 0.0, false);
        }
        let voices = sampler.voices[0].iter().filter(|v| v.active).count();
        assert_eq!(voices, if mode == Mode::OneShot { 3 } else { 1 });
        sampler.stop();
        for _ in 0..12 {
            block(&mut sampler, &lanes, 0.0, false);
        }
        let out = block(&mut sampler, &lanes, 0.0, false);
        assert!(out.iter().flatten().all(|&x| x == 0.0));
    }
}

#[test]
fn bar_64_last_step_wraps_without_retriggering_idle_blocks() {
    let (mut sampler, mut lanes) = setup(Mode::OneShot);
    lanes[0].bars = 64;
    lanes[0].steps[1023] = 127;
    block(&mut sampler, &lanes, 1023.0 * 48.0, true);
    assert_eq!(sampler.voices[0].iter().filter(|v| v.active).count(), 1);
    block(&mut sampler, &lanes, 1023.0 * 48.0 + 0.64, true);
    assert_eq!(sampler.voices[0].iter().filter(|v| v.active).count(), 1);
    block(&mut sampler, &lanes, 0.0, true);
    assert_eq!(sampler.voices[0].iter().filter(|v| v.active).count(), 1);
}

#[test]
fn one_shots_survive_wraps_and_excess_retriggers_without_stealing() {
    let (mut sampler, lanes) = setup(Mode::OneShot);
    for _ in 0..4 {
        sampler.audition(0);
        block(&mut sampler, &lanes, 0.0, false);
    }
    let ages = sampler.voices[0].map(|v| v.age);
    sampler.audition(0);
    block(&mut sampler, &lanes, 0.0, false);
    assert_eq!(sampler.voices[0].map(|v| v.age), ages.map(|age| age + 64));
    block(&mut sampler, &lanes, 500.0, true);
    block(&mut sampler, &lanes, 0.0, true);
    assert_eq!(sampler.voices[0].iter().filter(|v| v.active).count(), 4);
    for n in 0..800 {
        block(&mut sampler, &lanes, n as f64 * 0.64, true);
    }
    assert!(sampler.voices[0].iter().all(|v| !v.active));
}

#[test]
fn gate_duration_counts_played_ticks_across_a_transport_wrap() {
    let (mut sampler, mut lanes) = setup(Mode::Gate);
    lanes[0].gate = 0.1;
    sampler.audition(0);
    block(&mut sampler, &lanes, 767.0, true);
    for step in 0..20 {
        block(&mut sampler, &lanes, step as f64 * 0.64, true);
    }
    assert!(sampler.voices[0].iter().all(|v| !v.active));
}

#[test]
fn gater_follows_selected_tb_and_sidechain_is_opt_in() {
    let (mut sampler, mut lanes) = setup(Mode::Loop);
    lanes[0].gater_source = 2;
    lanes[0].gater_release_ms = 1.0;
    sampler.audition(0);
    sampler.set_modulation([true, false], [1.0; 64]);
    for _ in 0..80 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert!(sampler.modulation[0] < 1e-10);
    sampler.set_modulation([false, true], [0.0; 64]);
    for _ in 0..100 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert!(
        sampler.modulation[0] > 0.99,
        "unchecked sidechain must bypass"
    );
    lanes[0].sidechain = true;
    let mut out = [[0.0; 64]; 2];
    for _ in 0..80 {
        out = block(&mut sampler, &lanes, 0.0, false);
    }
    assert!(out.iter().flatten().all(|v| v.abs() < 1e-10));
    assert!(
        sampler.modulation[0] > 0.99,
        "sidechain must not alter the gater envelope"
    );
    lanes[0].gater_source = 0;
    lanes[0].sidechain = false;
    for _ in 0..100 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert_eq!(sampler.modulation[0], 1.0);
}

#[test]
fn drum_gater_uses_only_the_selected_machine_and_instrument() {
    let (mut sampler, mut lanes) = setup(Mode::Loop);
    lanes[0].gater_source = 3;
    lanes[0].gater_instrument = 2;
    lanes[0].gater_hold = 0.125;
    lanes[0].gater_attack_ms = 0.1;
    lanes[0].gater_release_ms = 1.0;
    sampler.audition(0);
    for _ in 0..40 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert_eq!(sampler.modulation[0], 0.0);

    let mut hits = [[0; 12]; 2];
    hits[0][1] = 1;
    hits[1][2] = 1;
    sampler.set_drum_modulation(hits, 0.01);
    block(&mut sampler, &lanes, 0.0, false);
    assert_eq!(
        sampler.modulation[0], 0.0,
        "808 kick and 909 snare are not 808 snare"
    );

    hits = [[0; 12]; 2];
    hits[0][2] = 1;
    sampler.set_drum_modulation(hits, 0.01);
    let output = block(&mut sampler, &lanes, 0.0, false);
    assert!(sampler.modulation[0] > 0.999);
    assert!(output[0].iter().any(|&sample| sample > 0.0));

    for _ in 0..40 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert_eq!(sampler.gater_holds[0], 0.0);
    assert_eq!(
        sampler.modulation[0], 0.0,
        "a consumed hit must not retrigger every block"
    );
}

#[test]
fn drum_gater_hold_counts_tempo_ticks_and_retriggers_without_envelope_reset() {
    for ticks_per_frame in [0.01, 0.02] {
        let (mut sampler, mut lanes) = setup(Mode::Loop);
        lanes[0].gater_source = 4;
        lanes[0].gater_instrument = 8;
        lanes[0].gater_hold = 0.5;
        lanes[0].gater_attack_ms = 0.1;
        let mut hits = [[0; 12]; 2];
        hits[1][8] = 1;
        sampler.set_drum_modulation(hits, ticks_per_frame);
        block(&mut sampler, &lanes, 0.0, false);
        let remaining = 24.0 - 64.0 * ticks_per_frame;
        assert!((sampler.gater_holds[0] - remaining).abs() < 1e-9);
        for _ in 0..10 {
            block(&mut sampler, &lanes, 0.0, false);
        }
        let before = sampler.modulation[0];
        sampler.set_drum_modulation(hits, ticks_per_frame);
        block(&mut sampler, &lanes, 0.0, false);
        assert!((sampler.gater_holds[0] - remaining).abs() < 1e-9);
        assert!(
            sampler.modulation[0] >= before,
            "retrigger extends Hold without closing the gate"
        );
        for _ in 0..60 {
            block(&mut sampler, &lanes, 0.0, false);
        }
        assert_eq!(sampler.gater_holds[0], 0.0);
    }
}

#[test]
fn drum_gater_depth_and_smoothing_remain_independent_of_sidechain() {
    let (mut sampler, mut lanes) = setup(Mode::Loop);
    lanes[0].gater_source = 4;
    lanes[0].gater_instrument = 8;
    lanes[0].gater_hold = 16.0;
    lanes[0].gater_depth = 0.65;
    lanes[0].gater_release_ms = 1.0;
    sampler.audition(0);
    for _ in 0..80 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert!((sampler.modulation[0] - 0.35).abs() < 1e-10);
    let mut hits = [[0; 12]; 2];
    hits[1][8] = 1;
    sampler.set_drum_modulation(hits, 0.01);
    sampler.set_modulation([false; 2], [0.0; 64]);
    let before = sampler.modulation[0];
    let output = block(&mut sampler, &lanes, 0.0, false);
    assert!(sampler.modulation[0] > before && sampler.modulation[0] < 1.0);
    assert!(output[0].iter().all(|&x| x.is_finite() && x > 0.0));
    for _ in 0..100 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert_eq!(sampler.modulation[0], 1.0);
    lanes[0].sidechain = true;
    let mut output = [[0.0; 64]; 2];
    for _ in 0..80 {
        output = block(&mut sampler, &lanes, 0.0, false);
    }
    assert!(output.iter().flatten().all(|x| x.abs() < 1e-10));
    assert_eq!(
        sampler.modulation[0], 1.0,
        "sidechain does not change the gater envelope"
    );
}

#[test]
fn old_gater_projects_keep_tb_routes_and_new_fields_default_safely() {
    for source in 0..=4 {
        let mut value = serde_json::to_value(Lane::default()).unwrap();
        value.as_object_mut().unwrap().remove("gater_instrument");
        value.as_object_mut().unwrap().remove("gater_hold");
        value["gater_source"] = serde_json::json!(source);
        let mut lane: Lane = serde_json::from_value(value).unwrap();
        lane.sanitize();
        assert_eq!(lane.gater_source, source);
        assert_eq!(lane.gater_instrument, 1);
        assert_eq!(lane.gater_hold, 0.5);
    }
}

#[test]
fn choke_groups_cut_other_lanes_and_unload_tail_cannot_return_on_reload() {
    let (mut sampler, mut lanes) = setup(Mode::OneShot);
    sampler.replace(
        1,
        Some(Box::new(Wave {
            frames: vec![[0.1; 2]; 10000],
        })),
    );
    lanes[0].choke = 3;
    lanes[1].choke = 3;
    sampler.audition(0);
    block(&mut sampler, &lanes, 0.0, false);
    sampler.audition(1);
    block(&mut sampler, &lanes, 0.0, false);
    assert!(sampler.voices[0].iter().all(|v| !v.active));
    assert!(sampler.voices[1].iter().any(|v| v.active));
    sampler.replace(1, None);
    for _ in 0..12 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert_eq!(sampler.tails[1], [0.0; 2]);
    sampler.replace(
        1,
        Some(Box::new(Wave {
            frames: vec![[0.1; 2]; 10000],
        })),
    );
    let out = block(&mut sampler, &lanes, 0.0, false);
    assert!(out.iter().flatten().all(|&x| x == 0.0));
}

#[test]
fn voice_trigger_occurs_at_the_sixteenth_crossing_inside_a_block() {
    let (mut sampler, mut lanes) = setup(Mode::OneShot);
    lanes[0].steps[1] = 127;
    let out = block(&mut sampler, &lanes, 47.7, true);
    assert!(out.iter().all(|c| c[..30].iter().all(|&x| x == 0.0)));
    assert!(out[0][32..].iter().any(|&x| x > 0.0));
    assert!(out[1][32..].iter().any(|&x| x < 0.0));
}

#[test]
fn loop_seam_preserves_duration_and_mute_reaches_exact_silence() {
    let (mut sampler, mut lanes) = setup(Mode::Loop);
    sampler.replace(
        0,
        Some(Box::new(Wave {
            frames: vec![[0.2; 2]; 1000],
        })),
    );
    sampler.audition(0);
    for _ in 0..30 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    let voice = sampler.voices[0].iter().find(|v| v.active).unwrap();
    assert_eq!(voice.position, 920.0);
    lanes[0].muted = true;
    for _ in 0..100 {
        block(&mut sampler, &lanes, 0.0, false);
    }
    assert!(
        block(&mut sampler, &lanes, 0.0, false)
            .iter()
            .flatten()
            .all(|&x| x == 0.0)
    );
}

#[test]
fn native_pitch_reads_pcm_without_resampling() {
    let (mut sampler, mut lanes) = setup(Mode::OneShot);
    lanes[0].gain = 1.0;
    sampler.gains[0] = [1.0; 2];
    sampler.audition(0);
    block(&mut sampler, &lanes, 0.0, false);
    let out = block(&mut sampler, &lanes, 0.0, false);
    assert!(out[0].iter().all(|&x| x == f64::from(0.2f32)));
    assert!(out[1].iter().all(|&x| x == f64::from(-0.1f32)));
}
