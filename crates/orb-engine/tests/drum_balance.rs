use std::sync::Arc;

use orb_engine::frame::{Devices, Engine};
use orb_engine::song::Device;
use orb_engine::studio::{Settings, Studio, convolution::Convolver};
use orb_engine::tables::Tables;

fn render(row: usize, voice: usize) -> Vec<i16> {
    render_mix(row, voice, None)
}

fn render_mix(row: usize, voice: usize, settings: Option<Settings>) -> Vec<i16> {
    render_with_inserts(row, voice, settings, false)
}

fn render_with_inserts(
    row: usize,
    voice: usize,
    settings: Option<Settings>,
    inserts: bool,
) -> Vec<i16> {
    let mut engine = Engine::new(
        Arc::new(Tables::new()),
        Devices::implemented(Some(rbm::standard::builtin_samples())),
    );
    engine.mixer_channel(row as u8, 1, 100);
    if inserts {
        engine.mixer_channel(row as u8, 4, 1);
        engine.device_param(Device::Distortion, 0, 1);
        engine.device_param(Device::Distortion, 1, 100);
        engine.device_param(Device::Distortion, 2, 64);
        engine.mixer_global(1, row as u8 + 1);
        engine.mixer_global(2, row as u8 + 1);
    }
    if let Some(settings) = settings {
        let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]]).unwrap()));
        studio.settings = Box::new(settings);
        engine.frame.studio = Some(Box::new(studio));
    }
    let pattern = engine.frame.devices.channels[row].patterns_mut().unwrap();
    for step in [0, 4, 8, 12] {
        pattern[2 + step * 12 + voice] = if row == 3 { 2 } else { 1 };
    }
    engine.play();
    let mut audio = vec![0; 2 * 44100 * 4];
    for block in audio.chunks_mut(2048) {
        engine.render(block, block.len() / 2);
    }
    audio
}

#[test]
fn default_trims_preserve_original_drum_audio_with_and_without_inserts() {
    assert_eq!(Settings::default().drum_gain_db, [0.0; 2]);
    for row in [2, 3] {
        for voice in [1, 2, 8] {
            for inserts in [false, true] {
                let legacy = render_with_inserts(row, voice, None, inserts);
                let studio = render_with_inserts(row, voice, Some(Settings::default()), inserts);
                assert!(legacy.iter().any(|&v| v != 0));
                assert_eq!(
                    studio, legacy,
                    "row {row}, voice {voice}, inserts {inserts}"
                );
            }
        }
    }
}

#[test]
fn output_trim_scales_the_processed_signal_without_changing_insert_drive() {
    for row in [2, 3] {
        let base = render_with_inserts(row, 1, Some(Settings::default()), true);
        let mut settings = Settings::default();
        settings.drum_gain_db[row - 2] = -6.0;
        let trimmed = render_with_inserts(row, 1, Some(settings), true);
        // Allow smoothing to settle; i16 conversion introduces at most one count of error.
        let gain = 10.0_f64.powf(-6.0 / 20.0);
        let mut peak = 0;
        for (&a, &b) in base[44100..].iter().zip(&trimmed[44100..]) {
            peak = peak.max(a.unsigned_abs());
            assert!(
                (f64::from(b) - f64::from(a) * gain).abs() < 2.0,
                "row {row}: {a} -> {b}, expected output gain {gain}"
            );
        }
        assert!(peak > 100, "must exercise an audible processed signal");
        assert!(peak < 32767, "comparison must not use clipped output");
    }
}

#[test]
fn old_projects_keep_unity_and_trim_is_bounded_smoothed_and_finite() {
    let mut json = serde_json::to_value(Settings::default()).unwrap();
    json.as_object_mut().unwrap().remove("drum_gain_db");
    let old: Settings = serde_json::from_value(json).unwrap();
    assert_eq!(old.drum_gain_db, [0.0; 2]);
    let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]]).unwrap()));
    studio.settings.drum_gain_db[0] = 6.0;
    let mut previous = 1.0_f32;
    for _ in 0..200 {
        let mut block = [1.0_f32; 64];
        studio.trim_drum(2, &mut block);
        for sample in block {
            assert!((sample - previous).abs() < 0.005);
            previous = sample;
        }
    }
    assert!((previous - 10.0_f32.powf(6.0 / 20.0)).abs() < 0.001);
    studio.settings.drum_gain_db = [f64::NAN, 1000.0];
    studio.settings.sanitize();
    assert_eq!(studio.settings.drum_gain_db, [0.0, 12.0]);
}

#[test]
fn standard_kit_drum_levels() {
    for (a, b, name) in [(1, 1, "kick"), (2, 2, "snare"), (11, 8, "closed hat")] {
        let stats = |row, voice| {
            let audio = render(row, voice);
            let power =
                audio.iter().map(|&v| f64::from(v).powi(2)).sum::<f64>() / audio.len() as f64;
            let peak = audio.iter().map(|v| v.unsigned_abs()).max().unwrap();
            assert!(power > 0.0);
            (power.sqrt(), peak)
        };
        let (rms808, peak808) = stats(2, a);
        let (rms909, peak909) = stats(3, b);
        println!(
            "{name}: 808 RMS {rms808:.2}, peak {peak808}; \
            909 RMS {rms909:.2}, peak {peak909}; difference {:.2} dB",
            20.0 * (rms909 / rms808).log10()
        );
    }
}
