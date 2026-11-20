use std::sync::Arc;

use orb_engine::frame::{Devices, Engine};
use orb_engine::song::Device;
use orb_engine::studio::{Studio, audio_file::Wave, convolution::Convolver, sampler::Mode};
use orb_engine::tables::Tables;

#[test]
fn native_counters_report_each_instrument_and_include_909_flams() {
    let tables = Arc::new(Tables::new());
    let samples = rbm::standard::builtin_samples();
    for row in 2..=3 {
        for instrument in 1..=11 {
            let mut engine =
                Engine::new(tables.clone(), Devices::implemented(Some(samples.clone())));
            engine.frame.devices.channels[row].patterns_mut().unwrap()[2 + instrument] =
                if row == 3 { 2 } else { 1 };
            engine.play();
            let mut audio = [0; 2048];
            engine.render(&mut audio, 1024);
            for column in 0..=11 {
                assert_eq!(
                    engine.frame.devices.channels[row].drum_trigger_count(column),
                    u64::from(column == instrument),
                    "row {row} / triggered {instrument} / observed {column}"
                );
            }
            assert_eq!(engine.frame.devices.channels[row].drum_trigger_count(99), 0);
        }
    }
    let mut engine = Engine::new(tables, Devices::implemented(Some(samples)));
    engine.frame.devices.channels[3].patterns_mut().unwrap()[3] = 3;
    engine.play();
    let mut audio = [0; 8192];
    engine.render(&mut audio, 4096);
    assert_eq!(engine.frame.devices.channels[3].drum_trigger_count(1), 2);
}

#[test]
fn logical_drum_triggers_ignore_level_and_mixer_but_respect_machine_enable() {
    let tables = Arc::new(Tables::new());
    let samples = rbm::standard::builtin_samples();
    for (row, device) in [(2, Device::Tr808), (3, Device::Tr909)] {
        for disabled in [false, true] {
            let mut engine =
                Engine::new(tables.clone(), Devices::implemented(Some(samples.clone())));
            engine.frame.devices.channels[row].patterns_mut().unwrap()[3] = 1;
            engine.device_param(device, 3, 0);
            engine.mixer_channel(row as u8, 0, 0);
            if disabled {
                engine.device_param(device, 0, 0);
            }
            engine.play();
            engine.render(&mut [0; 2048], 1024);
            assert_eq!(
                engine.frame.devices.channels[row].drum_trigger_count(1),
                u64::from(!disabled)
            );
            assert_eq!(engine.frame.devices.channels[row].kick_count(), 0);
        }
    }
}

#[test]
fn a_real_selected_drum_opens_the_sampler_even_with_the_drum_level_at_zero() {
    let tables = Arc::new(Tables::new());
    let samples = rbm::standard::builtin_samples();
    for (source, row, played, selected, opens) in [
        (3, 2, 2, 2, true),
        (3, 2, 1, 2, false),
        (3, 3, 2, 2, false),
        (4, 3, 8, 8, true),
    ] {
        let mut engine = Engine::new(tables.clone(), Devices::implemented(Some(samples.clone())));
        let device = if row == 2 {
            Device::Tr808
        } else {
            Device::Tr909
        };
        for controller in 3..=29 {
            engine.device_param(device, controller, 0);
        }
        engine.frame.devices.channels[row].patterns_mut().unwrap()[2 + played] =
            if row == 3 { 2 } else { 1 };
        let mut studio = Studio::new(Box::new(Convolver::new(&[[0.0; 2]]).unwrap()));
        let lane = &mut studio.settings.lanes[0];
        lane.mode = Mode::Loop;
        lane.steps[0] = 127;
        lane.gater_source = source;
        lane.gater_instrument = selected;
        lane.gater_depth = 1.0;
        lane.gater_attack_ms = 0.1;
        lane.gater_release_ms = 1.0;
        studio.sampler.replace(
            0,
            Some(Box::new(Wave {
                frames: vec![[0.2; 2]; 20000],
            })),
        );
        engine.frame.studio = Some(Box::new(studio));
        let mut audio = [0i16; 2048];
        for _ in 0..4 {
            engine.render(&mut audio, 1024);
        }
        engine.play();
        let mut audible = false;
        for _ in 0..8 {
            engine.render(&mut audio, 1024);
            audible |= audio.iter().any(|&sample| sample.unsigned_abs() > 2);
        }
        assert_eq!(
            audible, opens,
            "source {source}, row {row}, played {played}, selected {selected}"
        );
    }
}
