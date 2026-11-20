use orb_ui::midi::{Config, preset::RoutingPreset, sync::Mode};

#[test]
fn sync_defaults_and_explicit_saved_timings_are_distinct() {
    let default = Config::default();
    assert_eq!(default.clock.output_delay_ms, 30);
    assert_eq!(default.clock.input_offset_ms, 0);
    let saved: Config =
        toml::from_str("[clock]\noutput_delay_ms = 0\ninput_offset_ms = -30\n").unwrap();
    assert_eq!(saved.clock.output_delay_ms, 0);
    assert_eq!(saved.clock.input_offset_ms, -30);
    let old: Config = toml::from_str("enabled = false\n").unwrap();
    assert_eq!(old.clock.output_delay_ms, 30);
    let roundtrip: Config = toml::from_str(&toml::to_string(&saved).unwrap()).unwrap();
    assert_eq!(roundtrip.clock, saved.clock);
}

#[test]
fn old_preferences_disable_sync_and_routing_import_never_enables_a_master() {
    let mut config: Config = toml::from_str("enabled = true\npickup = true\n").unwrap();
    assert_eq!(config.clock.mode, Mode::Off);
    config.clock.mode = Mode::Slave;
    config.clock.input = Some("Drum machine IN".into());
    config.clock.output_delay_ms = 999;
    config.sanitize();
    assert_eq!(config.clock.output_delay_ms, 500);
    let preset = RoutingPreset::from_config(&config);
    let mut other = Config::default();
    preset.apply(&mut other).unwrap();
    assert_eq!(other.clock.mode, Mode::Off);
    assert!(other.clock.input.is_none());
    let encoded = toml::to_string(&config).unwrap();
    let restored: Config = toml::from_str(&encoded).unwrap();
    assert_eq!(restored.clock, config.clock);
}
