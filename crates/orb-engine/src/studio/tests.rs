use super::*;
use crate::{
    frame::{Devices, Engine},
    tables::Tables,
};
use std::sync::Arc;

fn sampler_engine(bus: SamplerBusSettings, lane_send: f64) -> Engine {
    let mut engine = Engine::new(Arc::new(Tables::new()), Devices::implemented(None));
    for channel in &mut engine.frame.devices.mixer.live.channels {
        channel.enabled = 0;
        channel.level = 0;
    }
    // Shared native effect settings are still authoritative for the sampler routes.
    engine.frame.devices.dist.set_param(0, 1);
    engine.frame.devices.dist.set_param(1, 110);
    engine.frame.devices.delay.set_param(0, 1);
    engine.frame.devices.delay.set_param(1, 1);
    engine.frame.devices.delay.set_param(3, 90);
    engine.frame.mixer_update();
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap()));
    studio.settings.sampler_bus = bus;
    studio.settings.predelay_ms = 0.0;
    studio.settings.lanes[0].send = lane_send;
    studio.settings.lanes[0].mode = sampler::Mode::Loop;
    studio.sampler.replace(
        0,
        Some(Box::new(audio_file::Wave {
            frames: (0..44100)
                .map(|j| {
                    let value =
                        (j as f64 * std::f64::consts::TAU * 220.0 / 44100.0).sin() as f32 * 0.2;
                    [value; 2]
                })
                .collect(),
        })),
    );
    studio.sampler.audition(0);
    engine.frame.studio = Some(Box::new(studio));
    engine
}

fn render_sampler(engine: &mut Engine, calls: usize) -> Vec<i16> {
    let mut result = Vec::new();
    let mut block = vec![0; 2048];
    for _ in 0..calls {
        engine.render(&mut block, 1024);
        result.extend_from_slice(&block);
    }
    result
}

#[test]
fn sampler_bus_native_dist_delay_and_space_routes_are_audible() {
    let baseline = render_sampler(&mut sampler_engine(SamplerBusSettings::default(), 0.0), 24);
    assert!(baseline.iter().any(|v| v.unsigned_abs() > 100));
    for bus in [
        SamplerBusSettings {
            dist: true,
            ..SamplerBusSettings::default()
        },
        SamplerBusSettings {
            delay: 1.0,
            ..SamplerBusSettings::default()
        },
        SamplerBusSettings {
            reverb_send: 1.0,
            ..SamplerBusSettings::default()
        },
    ] {
        let output = render_sampler(&mut sampler_engine(bus, 0.0), 24);
        assert!(
            output
                .iter()
                .zip(&baseline)
                .any(|(a, b)| a.abs_diff(*b) > 10),
            "{bus:?}"
        );
    }
    let lane_only = render_sampler(&mut sampler_engine(SamplerBusSettings::default(), 1.0), 24);
    assert!(
        lane_only
            .iter()
            .zip(&baseline)
            .any(|(a, b)| a.abs_diff(*b) > 10)
    );
}

#[test]
fn sampler_bus_gain_zero_silences_the_sampler_after_its_meter_detected_audio() {
    let mut engine = sampler_engine(SamplerBusSettings::default(), 0.0);
    assert!(
        render_sampler(&mut engine, 4)
            .iter()
            .any(|v| v.unsigned_abs() > 100)
    );
    assert!(
        engine
            .frame
            .studio
            .as_ref()
            .unwrap()
            .sampler_meter()
            .iter()
            .any(|&v| v > 0)
    );
    engine
        .frame
        .studio
        .as_mut()
        .unwrap()
        .settings
        .sampler_bus
        .gain = 0.0;
    render_sampler(&mut engine, 12);
    assert!(
        render_sampler(&mut engine, 1)
            .iter()
            .all(|v| v.unsigned_abs() <= 1)
    );
}

#[test]
fn an_empty_studio_is_sample_exact_with_the_legacy_engine() {
    let tables = Arc::new(Tables::new());
    let mut legacy = Engine::new(tables.clone(), Devices::implemented(None));
    let mut extended = Engine::new(tables, Devices::implemented(None));
    let mut song = rbs::Song::empty();
    song.glob.song_mode = 0;
    song.tb303[0].params[0] = 1;
    for value in &mut song.tb303[0].params[3..] {
        *value = 64;
    }
    song.tb303[0].patterns[0].length = 16;
    for (i, step) in song.tb303[0].patterns[0].steps.iter_mut().enumerate() {
        step.note = [0, 3, 7, 10][i % 4];
        step.rest = 0;
    }
    song.mixer.master = 80;
    song.mixer.channels[0].enabled = 1;
    song.mixer.channels[0].level = 80;
    let song = crate::load::convert(&song).unwrap();
    legacy.load(&song).unwrap();
    extended.load(&song).unwrap();
    extended.frame.studio = Some(Box::new(Studio::new(Box::new(
        Convolver::new(&[[0.1; 2]; 64]).unwrap(),
    ))));
    legacy.play();
    extended.play();
    let mut a = vec![0i16; 2048];
    let mut b = a.clone();
    let mut audible = false;
    for _ in 0..20 {
        legacy.render(&mut a, 1024);
        extended.render(&mut b, 1024);
        assert_eq!(a, b);
        audible |= a.iter().any(|x| x.unsigned_abs() > 10);
    }
    assert!(
        audible,
        "the bypass comparison must include an audible TB line"
    );
}

#[test]
fn queued_ir_requests_keep_latest_and_retire_every_root() {
    let make = || Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap());
    let mut studio = Studio::new(make());
    assert!(studio.replace_reverb(make()).is_none());
    assert!(studio.replace_reverb(make()).is_none());
    assert!(studio.replace_reverb(make()).is_some());
    studio.fade = 0;
    // Retirement can be delayed by a full garbage queue; a new request supersedes pending.
    assert!(studio.replace_reverb(make()).is_some());
    assert!(studio.retired().is_some());
    assert!(studio.pending.is_none());
    assert_eq!(studio.fade, 4410);
    studio.fade = 0;
    assert!(studio.retired().is_some());
    assert!(studio.previous.is_none());
}

#[test]
fn fading_states_reach_zero_instead_of_entering_subnormal_arithmetic() {
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.5; 2]; 64]).unwrap()));
    studio.smooth_sends = [1e-200; 4];
    studio.smooth_return = 1e-200;
    studio.smooth_delay = 1e-200;
    studio.high_state = [1e-200; 2];
    studio.low_state = [1e-200; 2];
    studio.settings.enabled = false;
    studio.settings.predelay_ms = 0.0;
    studio.begin();
    studio.channel(0, 0, 0.0, 0.0);
    studio.process(0.0, 0.01, false, &mut [0.0; 64], &mut [0.0; 64]);
    assert_eq!(studio.smooth_sends[0], 0.0);
    assert_eq!(studio.smooth_return, 0.0);
    assert_eq!(studio.smooth_delay, 0.0);
    assert_eq!(studio.high_state, [0.0; 2]);
    assert_eq!(studio.low_state, [0.0; 2]);
}

#[test]
fn release_long_running_sampler_and_silence_soak() {
    if cfg!(debug_assertions) {
        return;
    }
    let mut studio = Studio::new(Box::new(
        Convolver::new(&convolution::synthetic(1, 1.8)).unwrap(),
    ));
    for i in 0..8 {
        studio.sampler.waves[i] = Some(Box::new(audio_file::Wave {
            frames: vec![[0.1, -0.1]; 44100],
        }));
        studio.settings.lanes[i].steps.fill(100);
        studio.settings.lanes[i].send = 0.3;
    }
    let start = std::time::Instant::now();
    for block in 0..(120 * 44100 / 64) {
        studio.begin();
        let playing = block < 60 * 44100 / 64;
        let mut left = [0.0; 64];
        let mut right = [0.0; 64];
        studio.process(block as f64 * 0.64, 0.01, playing, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
    }
    eprintln!(
        "STUDIO_SOAK: 120 seconds audio / {:.3} seconds CPU",
        start.elapsed().as_secs_f64()
    );
}

#[test]
fn release_performance_with_six_second_ir_and_32_sampler_voices() {
    if cfg!(debug_assertions) {
        return;
    }
    let ir = convolution::synthetic(2, 6.0);
    let mut studio = Studio::new(Box::new(Convolver::new(&ir).unwrap()));
    studio.settings.sends = [0.3; 4];
    for (i, lane) in studio.settings.lanes.iter_mut().enumerate() {
        lane.gain = 0.1;
        lane.send = 0.2;
        studio.sampler.waves[i] = Some(Box::new(audio_file::Wave {
            frames: vec![[0.1; 2]; 100000],
        }));
    }
    for _ in 0..4 {
        for i in 0..8 {
            studio.sampler.audition(i);
        }
        studio.begin();
        studio.process(0.0, 0.01, false, &mut [0.0; 64], &mut [0.0; 64]);
    }
    let mut left = [0.0; 64];
    let mut right = [0.0; 64];
    let start = std::time::Instant::now();
    for _ in 0..690 {
        studio.begin();
        for i in 0..64 {
            studio.channel(0, i, 0.02, -0.01);
        }
        studio.process(0.0, 0.01, false, &mut left, &mut right);
        assert!(left.iter().chain(&right).all(|x| x.is_finite()));
        left.fill(0.0);
        right.fill(0.0);
    }
    let elapsed = start.elapsed().as_secs_f64();
    eprintln!("STUDIO_PERFORMANCE: 1.001 s audio, {elapsed:.3} s processing / 6 s IR / 32 voices");
    assert!(
        elapsed < 4.0,
        "Studio render is unexpectedly slow: {elapsed:.3} s"
    );
}
