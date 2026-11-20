use orb_ui::midi::preset::RoutingPreset;
use orb_ui::midi::{Binding, Config, targets};

fn config() -> Config {
    Config {
        clock: Default::default(),
        enabled: true,
        port: Some("UC-33 on first PC".into()),
        pickup: false,
        minimum_one: false,
        minimum_cc: 0,
        additional_inputs: Vec::new(),
        ranges: Default::default(),
        bindings: targets()
            .into_iter()
            .enumerate()
            .map(|(i, (target, _))| Binding {
                input: 0,
                channel: (i / 128 + 1) as u8,
                controller: (i % 128) as u8,
                target,
            })
            .collect(),
    }
}

#[test]
fn all_assignments_round_trip_and_input_selection_remains_local() {
    let original = config();
    let encoded = RoutingPreset::from_config(&original).encode().unwrap();
    assert!(!encoded.contains("UC-33 on first PC"));
    let metadata: toml::Value = toml::from_str(&encoded).unwrap();
    assert!(metadata.get("enabled").is_none());
    assert!(metadata.get("port").is_none());
    let mut other = Config {
        port: Some("Focusrite USB MIDI".into()),
        minimum_one: true,
        minimum_cc: 4,
        ..Config::default()
    };
    let count = RoutingPreset::decode(&encoded)
        .unwrap()
        .apply(&mut other)
        .unwrap();
    assert_eq!(count, original.bindings.len());
    assert_eq!(other.bindings, original.bindings);
    assert_eq!(other.pickup, original.pickup);
    assert_eq!(other.port.as_deref(), Some("Focusrite USB MIDI"));
    assert!(!other.enabled);
    assert!(
        other.minimum_one,
        "hardware calibration remains local on import"
    );
    assert_eq!(
        other.minimum_cc, 4,
        "numeric calibration remains local on import"
    );
    assert!(!encoded.contains("minimum_one"));
    assert!(!encoded.contains("minimum_cc"));
}

#[test]
fn corrupt_versions_unknown_targets_and_conflicting_assignments_are_rejected() {
    let original = config();
    let good = RoutingPreset::from_config(&original).encode().unwrap();
    for bad in [
        good.replace("version = 1", "version = 99"),
        good.replace("openrb-midi-routing", "other-program"),
        good.replace("channel = 1", "channel = 0"),
        good.replace("controller = 0", "controller = 128"),
        good.replace("transport.shuffle", "unknown.parameter"),
        good.replace("pickup = false", "pickup = 'broken'"),
        "not a config [".into(),
    ] {
        assert!(RoutingPreset::decode(&bad).is_err());
    }
    let mut conflict = original.clone();
    conflict.bindings.push(original.bindings[0].clone());
    assert!(RoutingPreset::from_config(&conflict).encode().is_err());
    let mut conflict = original.clone();
    conflict.bindings[1].target = conflict.bindings[0].target.clone();
    assert!(RoutingPreset::from_config(&conflict).encode().is_err());
}

#[test]
fn files_can_be_backed_up_overwritten_and_read_on_another_machine() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("UC33 routing backup.toml");
    let preset = RoutingPreset::from_config(&config());
    preset.write(&path).unwrap();
    assert_eq!(RoutingPreset::read(&path).unwrap(), preset);
    let empty = RoutingPreset::from_config(&Config::default());
    empty.write(&path).unwrap();
    assert_eq!(RoutingPreset::read(&path).unwrap(), empty);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn invalid_imports_leave_existing_assignments_untouched_and_large_files_are_bounded() {
    let mut existing = config();
    let before = existing.clone();
    let result = RoutingPreset::decode("format = 'bad'").and_then(|p| p.apply(&mut existing));
    assert!(result.is_err());
    assert_eq!(existing, before);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("huge.toml");
    let text = " ".repeat(1024 * 1024 + 10);
    std::fs::write(&path, text).unwrap();
    assert!(
        RoutingPreset::read(&path)
            .unwrap_err()
            .contains("size limit")
    );
}

#[test]
fn multiple_controller_slots_export_without_machine_specific_names() {
    let mut config = Config {
        port: Some("UC33 local device".into()),
        additional_inputs: vec![orb_ui::midi::InputSettings {
            port: Some("Pads local device".into()),
            minimum_one: true,
            minimum_cc: 5,
        }],
        ..Config::default()
    };
    for (input, target) in [(0, "sampler1.level"), (1, "sampler2.level")] {
        config.assign(Binding {
            input,
            channel: 1,
            controller: 7,
            target: target.into(),
        });
    }
    let encoded = RoutingPreset::from_config(&config).encode().unwrap();
    assert!(!encoded.contains("local device"));
    let mut imported = Config::default();
    RoutingPreset::decode(&encoded)
        .unwrap()
        .apply(&mut imported)
        .unwrap();
    assert_eq!(imported.bindings, config.bindings);
    assert!(imported.additional_inputs.is_empty());
}
