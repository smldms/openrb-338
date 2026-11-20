use super::*;

#[test]
fn default_bus_is_exact_unity_and_preserves_lane_sends() {
    let mut bus = Bus {
        dry: [[0.25; 64], [-0.125; 64]],
        reverb: [[0.05; 64], [-0.03; 64]],
        ..Bus::default()
    };
    let dry = bus.dry;
    let mut delay = [0.0; 64];
    let mut space = [[0.0; 64]; 2];
    bus.route(SamplerBusSettings::default(), None, &mut delay, &mut space);
    assert_eq!(bus.dry, dry);
    assert_eq!(space, bus.reverb);
    assert_eq!(delay, [0.0; 64]);
    assert_eq!(bus.meter(), [32, 16]);
}

#[test]
fn global_zero_cuts_dry_delay_and_both_kinds_of_space_send() {
    let tables = crate::tables::Tables::new();
    let ctx = BlockContext {
        tables: &tables,
        meter_index: 0,
        tempo: 140_000,
        tick: 0,
        stack_word: None,
    };
    let global = Distortion::new();
    let mut bus = Bus::default();
    let settings = SamplerBusSettings {
        gain: 0.0,
        dist: true,
        delay: 1.0,
        reverb_send: 1.0,
        ..SamplerBusSettings::default()
    };
    for _ in 0..160 {
        bus.dry = [[0.25; 64]; 2];
        bus.reverb = [[0.125; 64]; 2];
        let mut delay = [0.0; 64];
        let mut space = [[0.0; 64]; 2];
        bus.route(settings, Some((&global, &ctx)), &mut delay, &mut space);
    }
    bus.dry = [[0.25; 64]; 2];
    bus.reverb = [[0.125; 64]; 2];
    let mut delay = [0.0; 64];
    let mut space = [[0.0; 64]; 2];
    bus.route(settings, Some((&global, &ctx)), &mut delay, &mut space);
    assert_eq!(bus.dry, [[0.0; 64]; 2]);
    assert_eq!(delay, [0.0; 64]);
    assert_eq!(space, [[0.0; 64]; 2]);
}

#[test]
fn switching_off_preserves_level_and_can_resume_the_sampler_bus() {
    let mut bus = Bus::default();
    let mut settings = SamplerBusSettings {
        enabled: false,
        gain: 0.6,
        delay: 0.5,
        reverb_send: 0.5,
        ..SamplerBusSettings::default()
    };
    for _ in 0..160 {
        bus.dry = [[0.25; 64]; 2];
        bus.reverb = [[0.125; 64]; 2];
        bus.route(settings, None, &mut [0.0; 64], &mut [[0.0; 64]; 2]);
    }
    let mut delay = [0.0; 64];
    let mut space = [[0.0; 64]; 2];
    bus.dry = [[0.25; 64]; 2];
    bus.reverb = [[0.125; 64]; 2];
    bus.route(settings, None, &mut delay, &mut space);
    assert_eq!(bus.dry, [[0.0; 64]; 2]);
    assert_eq!(delay, [0.0; 64]);
    assert_eq!(space, [[0.0; 64]; 2]);
    assert_eq!(settings.gain, 0.6);
    settings.enabled = true;
    bus.dry = [[0.25; 64]; 2];
    bus.route(settings, None, &mut delay, &mut space);
    assert!(bus.dry.iter().flatten().any(|&v| v > 0.0));
}

#[test]
fn sampler_pan_also_pans_its_per_lane_sends() {
    let mut bus = Bus::default();
    let settings = SamplerBusSettings {
        pan: 1.0,
        ..SamplerBusSettings::default()
    };
    let mut last = [[0.0; 64]; 2];
    for _ in 0..160 {
        bus.dry = [[0.25; 64]; 2];
        bus.reverb = [[0.125; 64]; 2];
        last = [[0.0; 64]; 2];
        bus.route(settings, None, &mut [0.0; 64], &mut last);
    }
    assert_eq!(bus.dry[0], [0.0; 64]);
    assert_eq!(last[0], [0.0; 64]);
    assert_eq!(bus.dry[1], [0.25; 64]);
    assert_eq!(last[1], [0.125; 64]);
}

#[test]
fn old_studio_projects_default_the_new_bus_to_exact_bypass() {
    let mut old = serde_json::to_value(crate::studio::Settings::default()).unwrap();
    old.as_object_mut().unwrap().remove("sampler_bus");
    let settings: crate::studio::Settings = serde_json::from_value(old).unwrap();
    assert_eq!(settings.sampler_bus, SamplerBusSettings::default());
    let mut invalid = SamplerBusSettings {
        gain: f64::NAN,
        pan: 9.0,
        delay: -3.0,
        reverb_send: f64::INFINITY,
        ..SamplerBusSettings::default()
    };
    invalid.sanitize();
    assert_eq!(
        invalid,
        SamplerBusSettings {
            pan: 1.0,
            ..SamplerBusSettings::default()
        }
    );
}

#[test]
fn distortion_is_stereo_symmetric_and_never_advances_the_legacy_processor() {
    let tables = crate::tables::Tables::new();
    let ctx = BlockContext {
        tables: &tables,
        meter_index: 0,
        tempo: 140_000,
        tick: 0,
        stack_word: None,
    };
    let mut global = Distortion::new();
    global.set_param(2, 64);
    let mut bus = Bus::default();
    for block in 0..48 {
        if block == 24 {
            global.set_param(1, 100);
            global.set_param(2, 96);
        }
        let input = std::array::from_fn(|j| ((block * 64 + j) as f64 * 0.13).sin() * 0.2);
        bus.dry = if block < 12 {
            [[0.0; 64], input]
        } else {
            [input; 2]
        };
        bus.route(
            SamplerBusSettings {
                dist: true,
                ..SamplerBusSettings::default()
            },
            Some((&global, &ctx)),
            &mut [0.0; 64],
            &mut [[0.0; 64]; 2],
        );
        if block < 12 {
            assert_eq!(bus.dry[0], [0.0; 64]);
        } else {
            assert_eq!(bus.dry[0], bus.dry[1]);
        }
        assert_eq!(
            global.counter, 0,
            "sampler processing must not retime the legacy insert"
        );
        assert_eq!(global.gain_in, 0.0);
    }
    global.set_param(0, 0);
    let input = [0.123456789; 64];
    bus.dry = [input; 2];
    bus.route(
        SamplerBusSettings {
            dist: true,
            ..SamplerBusSettings::default()
        },
        Some((&global, &ctx)),
        &mut [0.0; 64],
        &mut [[0.0; 64]; 2],
    );
    assert_eq!(
        bus.dry, [input; 2],
        "the shared on/off switch bypasses without rounding"
    );
}
