//! The sampler's stereo mix bus, before shared send effects and the master fader.

#[cfg(test)]
mod tests;

use crate::dist::Distortion;
use crate::frame::{Block, BlockContext, InsertEffect, SongDevice};

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SamplerBusSettings {
    pub enabled: bool,
    pub gain: f64,
    pub pan: f64,
    pub dist: bool,
    pub delay: f64,
    pub reverb_send: f64,
}

impl Default for SamplerBusSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gain: 1.0,
            pan: 0.0,
            dist: false,
            delay: 0.0,
            reverb_send: 0.0,
        }
    }
}

impl SamplerBusSettings {
    pub fn sanitize(&mut self) {
        fn finite(value: f64, default: f64, lo: f64, hi: f64) -> f64 {
            if value.is_finite() {
                value.clamp(lo, hi)
            } else {
                default
            }
        }
        self.gain = finite(self.gain, 1.0, 0.0, 2.0);
        self.pan = finite(self.pan, 0.0, -1.0, 1.0);
        self.delay = finite(self.delay, 0.0, 0.0, 1.0);
        self.reverb_send = finite(self.reverb_send, 0.0, 0.0, 1.0);
    }
}

pub(super) struct Bus {
    pub dry: [[f64; 64]; 2],
    pub reverb: [[f64; 64]; 2],
    gain: f64,
    pan: f64,
    delay: f64,
    reverb_send: f64,
    peak: [f64; 2],
    // Independent stereo state; only control values are shared with the legacy insert.
    distortion: [Distortion; 2],
}

impl Default for Bus {
    fn default() -> Self {
        Self {
            dry: [[0.0; 64]; 2],
            reverb: [[0.0; 64]; 2],
            gain: 1.0,
            pan: 0.0,
            delay: 0.0,
            reverb_send: 0.0,
            peak: [0.0; 2],
            distortion: std::array::from_fn(|_| Distortion::new()),
        }
    }
}

impl Bus {
    pub fn clear(&mut self) {
        self.dry = [[0.0; 64]; 2];
        self.reverb = [[0.0; 64]; 2];
    }

    pub fn route(
        &mut self,
        settings: SamplerBusSettings,
        effect: Option<(&dyn InsertEffect, &BlockContext<'_>)>,
        legacy_delay: &mut Block,
        space: &mut [[f64; 64]; 2],
    ) {
        if settings.dist
            && let Some((effect, ctx)) = effect
            && effect.live().first().is_some_and(|&on| on != 0)
        {
            for (channel, distortion) in self.dry.iter_mut().zip(&mut self.distortion) {
                for (controller, &value) in effect.live().iter().take(3).enumerate() {
                    distortion.set_param(controller as u8, value);
                }
                let input = channel.iter().any(|&v| v != 0.0);
                let mut mono: Block = std::array::from_fn(|j| channel[j] as f32);
                // Both processors advance even for a silent side, keeping stereo state aligned.
                distortion.process(&mut mono, ctx);
                for (output, sample) in channel.iter_mut().zip(mono) {
                    *output = if input { f64::from(sample) } else { 0.0 };
                }
            }
        }
        // Match the machines: insert first, then fader/pan and post-fader sends.
        for j in 0..64 {
            let target = if settings.enabled { settings.gain } else { 0.0 };
            super::smooth(&mut self.gain, target, 0.004);
            super::smooth(&mut self.pan, settings.pan, 0.004);
            let gains = [
                self.gain * (1.0 - self.pan).min(1.0),
                self.gain * (1.0 + self.pan).min(1.0),
            ];
            for (c, gain) in gains.into_iter().enumerate() {
                if gain != 1.0 {
                    self.dry[c][j] *= gain;
                    self.reverb[c][j] *= gain;
                }
            }
        }
        self.peak = self.peak.map(|v| v * 0.995);
        for (j, delay) in legacy_delay.iter_mut().enumerate() {
            super::smooth(&mut self.delay, settings.delay, 0.004);
            super::smooth(&mut self.reverb_send, settings.reverb_send, 0.004);
            let sent = (self.dry[0][j] + self.dry[1][j]) * 0.5 * self.delay;
            if sent != 0.0 {
                *delay = (f64::from(*delay) + sent) as f32;
            }
            for (c, space) in space.iter_mut().enumerate() {
                let dry = self.dry[c][j];
                self.peak[c] = self.peak[c].max(dry.abs());
                let reverb = self.reverb[c][j] + dry * self.reverb_send;
                if reverb != 0.0 {
                    space[j] += reverb;
                }
            }
        }
    }

    pub fn meter(&self) -> [u8; 2] {
        self.peak.map(|v| (v.clamp(0.0, 1.0) * 127.0).round() as u8)
    }
}
