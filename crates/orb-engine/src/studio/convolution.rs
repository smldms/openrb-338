//! Zero-latency, nonuniform partitioned stereo FIR convolution at 44.1 kHz.
//! The first 2048 taps use 64-frame partitions; the tail uses 2048-frame partitions.
//! All plans, spectra and delay lines are allocated by `new`, never by `process`.

use super::fft::{Complex, Fft};

struct Partitions {
    hop: usize,
    fft: Fft,
    kernel: Vec<Vec<Complex>>,
    history: Vec<Vec<Complex>>,
    input: Vec<Complex>,
    sum: Vec<Complex>,
    overlap: Vec<f64>,
    cursor: usize,
}

impl Partitions {
    fn new(ir: &[f64], hop: usize) -> Self {
        let fft = Fft::new(hop * 2);
        let count = ir.len().div_ceil(hop).max(1);
        let mut kernel = Vec::with_capacity(count);
        for p in 0..count {
            let mut spectrum = vec![Complex::default(); 2 * hop];
            for (j, v) in spectrum.iter_mut().take(hop).enumerate() {
                v.re = ir.get(p * hop + j).copied().unwrap_or(0.0);
            }
            fft.run(&mut spectrum, false);
            kernel.push(spectrum);
        }
        Self {
            hop,
            fft,
            kernel,
            history: vec![vec![Complex::default(); 2 * hop]; count],
            input: vec![Complex::default(); 2 * hop],
            sum: vec![Complex::default(); 2 * hop],
            overlap: vec![0.0; hop],
            cursor: 0,
        }
    }

    fn process(&mut self, input: &[f64], output: &mut [f64]) {
        self.input.fill(Complex::default());
        for (v, &x) in self.input.iter_mut().zip(input) {
            v.re = x;
        }
        self.fft.run(&mut self.input, false);
        self.history[self.cursor].copy_from_slice(&self.input);
        self.sum.fill(Complex::default());
        let count = self.kernel.len();
        for (p, kernel) in self.kernel.iter().enumerate() {
            let history = &self.history[(self.cursor + count - p) % count];
            for ((sum, &x), &h) in self.sum.iter_mut().zip(history).zip(kernel) {
                let product = x.mul(h);
                sum.re += product.re;
                sum.im += product.im;
            }
        }
        self.fft.run(&mut self.sum, true);
        for (i, out) in output.iter_mut().enumerate().take(self.hop) {
            *out = self.sum[i].re + self.overlap[i];
            self.overlap[i] = self.sum[i + self.hop].re;
        }
        self.cursor = (self.cursor + 1) % count;
    }
}

struct Channel {
    head: Partitions,
    tail: Option<Partitions>,
    input: [f64; 2048],
    delayed: [f64; 2048],
    position: usize,
}

impl Channel {
    fn new(ir: &[f64]) -> Self {
        Self {
            head: Partitions::new(&ir[..ir.len().min(2048)], 64),
            tail: (ir.len() > 2048).then(|| Partitions::new(&ir[2048..], 2048)),
            input: [0.0; 2048],
            delayed: [0.0; 2048],
            position: 0,
        }
    }

    fn process(&mut self, input: &[f64; 64], output: &mut [f64; 64]) {
        self.head.process(input, output);
        for (i, out) in output.iter_mut().enumerate() {
            *out += self.delayed[self.position + i];
            self.input[self.position + i] = input[i];
        }
        self.position += 64;
        if self.position == 2048 {
            self.position = 0;
            if let Some(tail) = &mut self.tail {
                tail.process(&self.input, &mut self.delayed);
            }
        }
    }
}

pub struct Convolver {
    channels: [Channel; 2],
}

impl Convolver {
    /// IRs must already be resampled to 44.1 kHz. Mono IRs may be duplicated by the caller.
    pub fn new(ir: &[[f32; 2]]) -> Result<Self, String> {
        if ir.is_empty() || ir.len() > 6 * 44100 || ir.iter().flatten().any(|x| !x.is_finite()) {
            return Err(
                "Impulse response must contain 1 frame to 6 seconds of finite audio".into(),
            );
        }
        Ok(Self {
            channels: std::array::from_fn(|c| {
                Channel::new(&ir.iter().map(|x| f64::from(x[c])).collect::<Vec<_>>())
            }),
        })
    }

    pub fn process(&mut self, input: &[[f64; 64]; 2], output: &mut [[f64; 64]; 2]) {
        for c in 0..2 {
            self.channels[c].process(&input[c], &mut output[c]);
        }
    }
}

/// Original synthetic IRs, with deterministic early reflections and a diffuse decaying tail.
/// These are generated responses, not recordings of real rooms or third-party presets.
pub fn synthetic(preset: usize, seconds: f64) -> Vec<[f32; 2]> {
    let frames = (seconds.clamp(0.1, 6.0) * 44100.0) as usize;
    let mut ir = vec![[0.0f32; 2]; frames];
    let mut seed = 0x338_cafe_u64 + preset as u64;
    let mut smooth = [0.0; 2];
    for (i, frame) in ir.iter_mut().enumerate() {
        for c in 0..2 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let noise = (seed >> 11) as f64 / ((1u64 << 53) as f64) * 2.0 - 1.0;
            smooth[c] = smooth[c] * (0.2 + preset.min(3) as f64 * 0.14) + noise * 0.3;
            let envelope = (-6.91 * i as f64 / frames as f64).exp();
            let onset = (i as f64 / (300.0 + preset as f64 * 300.0)).min(1.0);
            frame[c] = (smooth[c] * envelope * onset) as f32;
        }
    }
    for (index, gain) in [
        (0, 0.35),
        (149, 0.65),
        (379, -0.45),
        (827, 0.31),
        (1421, -0.2),
    ] {
        for c in [0, 1] {
            let i = (index + c * (index / 9)) * (preset.min(3) + 1);
            if i < frames {
                ir[i][c] += gain;
            }
        }
    }
    let energy = ir
        .iter()
        .flatten()
        .map(|&x| f64::from(x).powi(2))
        .sum::<f64>();
    let norm = (1.0 / (energy / 2.0).sqrt().max(1.0)) as f32;
    for x in ir.iter_mut().flatten() {
        *x *= norm;
    }
    ir
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_boundaries_match_direct_stereo_fir_without_latency() {
        let mut ir = vec![[0.0; 2]; 5101];
        for (i, x) in ir.iter_mut().enumerate() {
            *x = [((i * 7 % 19) as f32 - 9.0) * 0.001, (i % 7) as f32 * -0.002];
        }
        let mut convolver = Convolver::new(&ir).unwrap();
        let signal: Vec<_> = (0..15040)
            .map(|i| {
                if i < 160 {
                    ((i * 13 % 23) as f64 - 11.0) * 0.03
                } else if [2047, 2048, 4095, 4096, 8193].contains(&i) {
                    0.2
                } else {
                    0.0
                }
            })
            .collect();
        let events: Vec<_> = signal
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, x)| *x != 0.0)
            .collect();
        for block in 0..signal.len() / 64 {
            let input = std::array::from_fn(|c| {
                std::array::from_fn(|j| signal[block * 64 + j] * if c == 0 { 1.0 } else { -0.4 })
            });
            let mut output = [[0.0; 64]; 2];
            convolver.process(&input, &mut output);
            for c in 0..2 {
                for (j, &sample) in output[c].iter().enumerate() {
                    let n = block * 64 + j;
                    let direct: f64 = events
                        .iter()
                        .filter(|(k, _)| *k <= n)
                        .filter_map(|&(k, x)| ir.get(n - k).map(|h| x * f64::from(h[c])))
                        .sum::<f64>()
                        * if c == 0 { 1.0 } else { -0.4 };
                    assert!((sample - direct).abs() < 1e-10, "{n} / {c}");
                }
            }
        }
    }
}
