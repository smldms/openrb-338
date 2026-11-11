//! Portable, versioned routing files. Device selection and enable state remain local.

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{Binding, Config, kind};

const FORMAT: &str = "openrb-midi-routing";
const VERSION: u32 = 1;
const MAX_BYTES: usize = 1024 * 1024;
const MAX_BINDINGS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingPreset {
    format: String,
    version: u32,
    pickup: bool,
    bindings: Vec<Binding>,
}

impl RoutingPreset {
    pub fn from_config(config: &Config) -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            pickup: config.pickup,
            bindings: config.bindings.clone(),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.format != FORMAT || self.version != VERSION {
            return Err("Not a supported OpenRB MIDI routing file (expected version 1).".into());
        }
        if self.bindings.len() > MAX_BINDINGS {
            return Err("Too many assignments in the routing file.".into());
        }
        let mut sources = HashSet::new();
        let mut targets = HashSet::new();
        for b in &self.bindings {
            if !(1..=16).contains(&b.channel) || b.controller > 127 {
                return Err("A MIDI channel or CC number in the file is invalid.".into());
            }
            if kind(&b.target).is_none() {
                return Err(format!("Unknown MIDI target: {}", b.target));
            }
            if !sources.insert((b.channel, b.controller)) || !targets.insert(b.target.as_str()) {
                return Err("The file contains conflicting MIDI assignments.".into());
            }
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<String, String> {
        self.validate()?;
        toml::to_string_pretty(self).map_err(|e| format!("Cannot encode routing: {e}"))
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        if text.len() > MAX_BYTES {
            return Err("The routing file exceeds the 1 MB size limit.".into());
        }
        let preset: Self =
            toml::from_str(text).map_err(|e| format!("Invalid routing file: {e}"))?;
        preset.validate()?;
        Ok(preset)
    }

    /// Validation happens before modifying anything; ports and activation are not imported.
    pub fn apply(&self, config: &mut Config) -> Result<usize, String> {
        self.validate()?;
        config.bindings.clone_from(&self.bindings);
        config.pickup = self.pickup;
        Ok(self.bindings.len())
    }

    pub fn read(path: &Path) -> Result<Self, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("Cannot open routing: {e}"))?;
        let mut text = String::new();
        file.take((MAX_BYTES + 1) as u64)
            .read_to_string(&mut text)
            .map_err(|e| format!("Cannot read routing: {e}"))?;
        Self::decode(&text)
    }

    pub fn write(&self, path: &Path) -> Result<(), String> {
        crate::session::write_file(path, self.encode()?.as_bytes())
    }
}
