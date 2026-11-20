//! In-place radix-2 FFT. Plans and work buffers are prepared outside the audio callback.

#[derive(Clone, Copy, Default, Debug)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}

pub struct Fft {
    reverse: Vec<usize>,
    roots: Vec<Complex>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        Self {
            reverse: (0..n)
                .map(|i| i.reverse_bits() >> (usize::BITS - n.ilog2()))
                .collect(),
            roots: (0..n / 2)
                .map(|i| {
                    let a = -std::f64::consts::TAU * i as f64 / n as f64;
                    Complex {
                        re: a.cos(),
                        im: a.sin(),
                    }
                })
                .collect(),
        }
    }

    pub fn run(&self, x: &mut [Complex], inverse: bool) {
        let n = self.reverse.len();
        assert_eq!(x.len(), n);
        for (i, &j) in self.reverse.iter().enumerate() {
            if i < j {
                x.swap(i, j);
            }
        }
        let mut width = 2;
        while width <= n {
            for start in (0..n).step_by(width) {
                for j in 0..width / 2 {
                    let mut w = self.roots[j * n / width];
                    if inverse {
                        w.im = -w.im;
                    }
                    let b = x[start + j + width / 2].mul(w);
                    let a = x[start + j];
                    x[start + j] = Complex {
                        re: a.re + b.re,
                        im: a.im + b.im,
                    };
                    x[start + j + width / 2] = Complex {
                        re: a.re - b.re,
                        im: a.im - b.im,
                    };
                }
            }
            width *= 2;
        }
        if inverse {
            for a in x {
                a.re /= n as f64;
                a.im /= n as f64;
            }
        }
    }
}
