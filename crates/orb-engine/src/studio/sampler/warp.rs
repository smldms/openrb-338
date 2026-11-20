//! Bounded stereo waveform-similarity overlap-add, reading the existing PCM directly.
//! Grain timing follows the transport; grain read speed determines pitch independently.

#[derive(Clone, Copy, Default)]
pub(super) struct Warp {
    positions: [f64; 2],
    phase: usize,
    hop: usize,
    region: (usize, usize, bool, bool, usize),
    ready: bool,
    active: bool,
    fade: usize,
    from: [f64; 2],
    last: [f64; 2],
}

pub(super) struct Region<'a> {
    pub frames: &'a [[f32; 2]],
    pub table: &'a [[f64; 16]],
    pub start: usize,
    pub end: usize,
    pub reverse: bool,
    pub looping: bool,
    pub pitch: usize,
    pub rate: f64,
    pub advance: f64,
}

impl Region<'_> {
    fn length(&self) -> usize {
        self.end - self.start
    }

    fn position(&self, relative: f64) -> f64 {
        let length = self.length() as f64;
        let relative = if self.looping {
            relative.rem_euclid(length)
        } else {
            relative.clamp(0.0, length - 1.0)
        };
        if self.reverse {
            self.end as f64 - 1.0 - relative
        } else {
            self.start as f64 + relative
        }
    }

    fn index(&self, index: isize) -> usize {
        if self.looping {
            self.start + (index - self.start as isize).rem_euclid(self.length() as isize) as usize
        } else {
            index.clamp(self.start as isize, self.end as isize - 1) as usize
        }
    }

    fn linear(&self, relative: f64) -> [f64; 2] {
        let position = self.position(relative);
        let center = position.floor() as isize;
        let fraction = position - center as f64;
        let a = self.frames[self.index(center)];
        let b = self.frames[self.index(center + 1)];
        std::array::from_fn(|c| f64::from(a[c]) + f64::from(b[c] - a[c]) * fraction)
    }

    fn sample(&self, relative: f64) -> [f64; 2] {
        let position = self.position(relative);
        let center = position.floor() as isize;
        if self.pitch == 24 && position == center as f64 {
            return self.frames[self.index(center)].map(f64::from);
        }
        let phase = ((position - center as f64) * 256.0) as usize;
        let weights = &self.table[self.pitch * 256 + phase.min(255)];
        let mut value = [0.0; 2];
        for (tap, &weight) in weights.iter().enumerate() {
            let frame = self.frames[self.index(center + tap as isize - 7)];
            for (c, sample) in value.iter_mut().enumerate() {
                *sample += f64::from(frame[c]) * weight;
            }
        }
        value
    }

    fn align(&self, ideal: f64, previous: f64, hop: usize) -> f64 {
        const POINTS: usize = 32;
        let stride = (hop / POINTS).max(1) as f64 * self.rate;
        let reference: [[f64; 2]; POINTS] =
            std::array::from_fn(|n| self.linear(previous + n as f64 * stride));
        let sums: [f64; 2] = std::array::from_fn(|c| reference.iter().map(|v| v[c]).sum());
        let energy: f64 = (0..2)
            .map(|c| {
                reference.iter().map(|v| v[c] * v[c]).sum::<f64>()
                    - sums[c] * sums[c] / POINTS as f64
            })
            .sum();
        if energy < 1e-12 {
            return ideal;
        }
        let search = (self.length() / 8).clamp(1, 256) as i32;
        let score = |offset: i32| {
            let candidate = ideal + f64::from(offset);
            if !self.looping && !(0.0..self.length() as f64).contains(&candidate) {
                return f64::NEG_INFINITY;
            }
            let mut sum = [0.0; 2];
            let mut square = 0.0;
            let mut dot = 0.0;
            for (n, reference) in reference.iter().enumerate() {
                let sample = self.linear(candidate + n as f64 * stride);
                for c in 0..2 {
                    sum[c] += sample[c];
                    square += sample[c] * sample[c];
                    dot += sample[c] * reference[c];
                }
            }
            for c in 0..2 {
                square -= sum[c] * sum[c] / POINTS as f64;
                dot -= sum[c] * sums[c] / POINTS as f64;
            }
            if square < 1e-12 {
                return -1.0;
            }
            dot / (square * energy).sqrt() - f64::from(offset.abs()) / f64::from(search) * 0.0001
        };
        let mut best = 0;
        let mut best_score = score(0);
        // Fixed upper bound: 17 coarse + 17 medium + 9 fine candidates per grain.
        for step in [32, 4, 1] {
            let radius = match step {
                32 => search,
                4 => 32,
                _ => 4,
            };
            let center = best;
            for delta in (-radius..=radius).step_by(step as usize) {
                let candidate = (center + delta).clamp(-search, search);
                let value = score(candidate);
                if value > best_score {
                    best = candidate;
                    best_score = value;
                }
            }
        }
        ideal + f64::from(best)
    }
}

impl Warp {
    pub fn reset(&mut self) {
        self.ready = false;
    }

    pub fn read(&mut self, region: &Region<'_>, cursor: f64) -> [f64; 2] {
        let tag = (
            region.start,
            region.end,
            region.reverse,
            region.looping,
            region.pitch,
        );
        if !self.ready || self.region != tag {
            if self.ready {
                self.from = self.last;
                self.fade = 64;
            }
            self.ready = true;
            self.region = tag;
            self.hop = (region.length() / 8).clamp(16, 1024);
            self.phase = 0;
            self.positions = [cursor; 2];
        } else if self.phase >= self.hop {
            self.positions[0] = self.positions[1];
            // Compensate the overlap's analysis lag so transients follow the source clock.
            let ideal = cursor + (region.advance - region.rate) * self.hop as f64;
            let ideal = if region.looping {
                ideal
            } else {
                ideal.clamp(0.0, region.length() as f64 - 1.0)
            };
            self.positions[1] = region.align(ideal, self.positions[0], self.hop);
            self.phase = 0;
        }
        let phase = self.phase as f64 / self.hop as f64;
        let mix = phase * phase * (3.0 - 2.0 * phase);
        let a = region.sample(self.positions[0]);
        let b = region.sample(self.positions[1]);
        let value = std::array::from_fn(|c| a[c] * (1.0 - mix) + b[c] * mix);
        self.positions = self.positions.map(|p| p + region.rate);
        self.phase += 1;
        value
    }

    pub fn transition(&mut self, value: [f64; 2], active: bool) -> [f64; 2] {
        if active != self.active {
            self.active = active;
            self.from = self.last;
            self.fade = 64;
        }
        let value = if self.fade > 0 {
            let mix = (65 - self.fade) as f64 / 64.0;
            self.fade -= 1;
            std::array::from_fn(|c| self.from[c] * (1.0 - mix) + value[c] * mix)
        } else {
            value
        };
        self.last = value;
        value
    }
}
