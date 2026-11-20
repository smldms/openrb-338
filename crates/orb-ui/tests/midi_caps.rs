use orb_ui::midi::{Binding, Cc, Config, Router, ValueRange, preset::RoutingPreset};
use orb_ui::model::Panel;

fn binding(target: &str, controller: u8) -> Binding {
    Binding {
        input: 0,
        channel: 1,
        controller,
        target: target.into(),
    }
}

#[test]
fn any_chosen_maximum_scales_the_whole_fader_travel_after_device_calibration() {
    for max in [0, 1, 50, 80, 120, 127] {
        let mut config = Config {
            pickup: false,
            minimum_cc: 2,
            ..Config::default()
        };
        config.assign(binding("mixer3.level", 7));
        config
            .ranges
            .insert("mixer3.level".into(), ValueRange { max });
        let mut panel = Panel::new();
        let mut router = Router::default();
        for (raw, expected) in [(127, max), (2, 0), (0, 0), (64, max / 2)] {
            router.apply(
                Cc {
                    channel: 1,
                    controller: 7,
                    value: raw,
                },
                &config,
                &mut panel,
                0.0,
            );
            assert!(
                panel.value("mixer3.level").unwrap().abs_diff(expected) <= 1,
                "MAX {max}, physical CC {raw}"
            );
        }
    }
}

#[test]
fn pickup_can_catch_a_new_maximum_below_the_current_parameter() {
    let mut config = Config::default();
    config.assign(binding("mixer1.level", 7));
    config
        .ranges
        .insert("mixer1.level".into(), ValueRange { max: 50 });
    let mut panel = Panel::new();
    panel.set_fader("mixer1.level", 999);
    let mut router = Router::default();
    for raw in [127, 90, 40, 0] {
        router.apply(
            Cc {
                channel: 1,
                controller: 7,
                value: raw,
            },
            &config,
            &mut panel,
            0.0,
        );
        let expected = (u16::from(raw) * 50 + 63) / 127;
        assert!(u16::from(panel.value("mixer1.level").unwrap()).abs_diff(expected) <= 1);
    }
}

#[test]
fn caps_are_saved_imported_and_preserved_when_relearning_or_sanitizing() {
    let mut config = Config::default();
    config.assign(binding("mixer1.level", 7));
    config.assign(binding("mixer2.level", 8));
    config
        .ranges
        .insert("mixer1.level".into(), ValueRange { max: 50 });
    config
        .ranges
        .insert("mixer2.level".into(), ValueRange { max: 120 });
    config.sanitize();
    assert_eq!(config.ranges.len(), 2);
    let encoded = RoutingPreset::from_config(&config).encode().unwrap();
    let mut restored = Config::default();
    RoutingPreset::decode(&encoded)
        .unwrap()
        .apply(&mut restored)
        .unwrap();
    assert_eq!(restored.ranges, config.ranges);
    let prefs = toml::to_string(&config).unwrap();
    assert_eq!(
        toml::from_str::<Config>(&prefs).unwrap().ranges,
        config.ranges
    );
    config.assign(binding("mixer1.level", 9));
    assert_eq!(config.ranges["mixer1.level"].max, 50);
    config.assign(binding("mixer3.level", 9));
    assert!(!config.ranges.contains_key("mixer1.level"));
    assert_eq!(config.ranges["mixer2.level"].max, 120);
    let old = encoded.split("[ranges").next().unwrap();
    let legacy = RoutingPreset::decode(old).unwrap();
    legacy.apply(&mut restored).unwrap();
    assert!(restored.ranges.is_empty());
}
