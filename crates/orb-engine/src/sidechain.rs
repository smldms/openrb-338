//! Optional TR-triggered TB ducking. Disabled by default; not part of the RBS format.
//! Trigger bookkeeping never changes the original instruments' DSP or sequencing.

use crate::frame::BLOCK;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    Tr808,
    Tr909,
    #[default]
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub enabled: bool,
    pub source: Source,
    /// Native TR pattern column, 1 (BD) through 11.
    pub instrument: u8,
    /// Gain reduction at the deepest point, in percent, independently for each TB.
    pub depth: [u8; 2],
    pub release_ms: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            source: Source::Both,
            instrument: 1,
            depth: [55, 55],
            release_ms: 180,
        }
    }
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        self.instrument = self.instrument.clamp(1, 11);
        self.depth = self.depth.map(|d| d.min(100));
        self.release_ms = self.release_ms.clamp(20, 1000);
        self
    }

    /// Select cumulative voice counters; source filtering remains in `Ducker::gains`.
    pub fn trigger_counts(&self, counts: [[u64; 12]; 2]) -> [u64; 2] {
        let instrument = usize::from(self.instrument.clamp(1, 11));
        counts.map(|machine| machine[instrument])
    }
}

const RATE: u32 = 44_100;
const ATTACK: u32 = 44; // 1 ms, a short ramp rather than a discontinuous gain change.
const HOLD: u32 = 441; // Leave a short space for the kick transient.
const SLEW: f64 = 1.0 / 88.0; // 2 ms maximum transition for bypass and depth changes.

#[derive(Debug)]
pub struct Ducker {
    config: Config,
    seen: [u64; 2],
    seen_voices: [[u64; 12]; 2],
    phase: u32,
    start: f64,
    envelope: f64,
    gain: [f64; 2],
}

impl Default for Ducker {
    fn default() -> Self {
        Self {
            config: Config::default(),
            seen: [0; 2],
            seen_voices: [[0; 12]; 2],
            phase: u32::MAX,
            start: 0.0,
            envelope: 0.0,
            gain: [1.0; 2],
        }
    }
}

impl Ducker {
    pub fn set_config(&mut self, config: Config) {
        self.config = config.sanitized();
    }

    pub fn config(&self) -> Config {
        self.config
    }

    pub fn reduction(&self) -> [u8; 2] {
        self.gain
            .map(|g| ((1.0 - g) * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    /// Track every voice so changing the selected instrument never replays an older hit.
    pub fn gains_for_voices(
        &mut self,
        counts: [[u64; 12]; 2],
        audible: [bool; 2],
    ) -> [[f32; BLOCK]; 2] {
        self.seen = self.config.trigger_counts(self.seen_voices);
        self.seen_voices = counts;
        self.gains(self.config.trigger_counts(counts), audible)
    }

    /// One block, with selected trigger counts sampled after the sequencer ran.
    /// No allocation or lock. `gains_for_voices` additionally handles live voice changes.
    pub fn gains(&mut self, kicks: [u64; 2], audible: [bool; 2]) -> [[f32; BLOCK]; 2] {
        let changed = [
            kicks[0] != self.seen[0] && audible[0],
            kicks[1] != self.seen[1] && audible[1],
        ];
        self.seen = kicks;
        if (!self.config.enabled || self.config.depth == [0, 0]) && self.gain == [1.0; 2] {
            self.phase = u32::MAX;
            self.envelope = 0.0;
            return [[1.0; BLOCK]; 2];
        }
        let trigger = match self.config.source {
            Source::Tr808 => changed[0],
            Source::Tr909 => changed[1],
            Source::Both => changed[0] || changed[1],
        };
        if self.config.enabled && trigger {
            self.start = self.envelope;
            self.phase = 0;
        }
        let release = RATE * u32::from(self.config.release_ms) / 1000;
        let end = ATTACK + HOLD + release;
        let mut out = [[1.0; BLOCK]; 2];
        for i in 0..BLOCK {
            self.envelope = if self.phase < ATTACK {
                self.start + (1.0 - self.start) * f64::from(self.phase + 1) / f64::from(ATTACK)
            } else if self.phase < ATTACK + HOLD {
                1.0
            } else if self.phase < end {
                f64::from(end - self.phase) / f64::from(release)
            } else {
                0.0
            };
            self.phase = self.phase.saturating_add(1);
            for (row, block) in out.iter_mut().enumerate() {
                let target = if self.config.enabled {
                    1.0 - self.envelope * f64::from(self.config.depth[row]) / 100.0
                } else {
                    1.0
                };
                self.gain[row] += (target - self.gain[row]).clamp(-SLEW, SLEW);
                block[i] = self.gain[row] as f32;
            }
        }
        out
    }
}
