//! Musical pattern generation. Only pattern data changes; synthesis stays in orb-engine.
//! Seeded generators are deterministic so musical constraints can be checked without audio I/O.

mod ui;

use crate::model::Step303;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scale {
    #[default]
    Minor,
    Dorian,
    Phrygian,
    Pentatonic,
    Major,
}

impl Scale {
    pub const ALL: [Self; 5] = [
        Self::Minor,
        Self::Dorian,
        Self::Phrygian,
        Self::Pentatonic,
        Self::Major,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Minor => "Natural minor",
            Self::Dorian => "Dorian",
            Self::Phrygian => "Phrygian",
            Self::Pentatonic => "Minor pentatonic",
            Self::Major => "Major",
        }
    }

    pub fn intervals(self) -> &'static [u8] {
        match self {
            Self::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Self::Pentatonic => &[0, 3, 5, 7, 10],
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DrumStyle {
    #[default]
    Techno,
    Broken,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub root: u8,
    pub scale: Scale,
    pub density: u8,
    pub accents: u8,
    pub slides: u8,
    pub variation: u8,
    pub drum_density: u8,
    pub drum_style: DrumStyle,
    pub link_tb: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            root: 2,
            scale: Scale::Minor,
            density: 72,
            accents: 35,
            slides: 28,
            variation: 35,
            drum_density: 48,
            drum_style: DrumStyle::Techno,
            link_tb: false,
        }
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut n = self.0;
        n = (n ^ (n >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        n = (n ^ (n >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        n ^ (n >> 31)
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn chance(&mut self, percent: u8) -> bool {
        self.pick(100) < usize::from(percent.min(100))
    }
}

fn note(pitch: i32, accent: bool) -> Step303 {
    let mut pitch = pitch;
    while pitch > 23 {
        pitch -= 12;
    }
    while pitch < -12 {
        pitch += 12;
    }
    Step303 {
        note: pitch.rem_euclid(12) as u8,
        up: pitch >= 12,
        down: pitch < 0,
        accent,
        slide: false,
        rest: false,
    }
}

pub fn pitch(step: Step303) -> i32 {
    i32::from(step.note) + 12 * i32::from(step.up) - 12 * i32::from(step.down)
}

fn finish_slides(steps: &mut [Step303; 16], length: usize, percent: u8, rng: &mut Rng) {
    for s in 0..length {
        steps[s].slide =
            length > 1 && !steps[s].rest && !steps[(s + 1) % length].rest && rng.chance(percent);
    }
}

fn bytes303(steps: &[Step303; 16], length: usize) -> Vec<u8> {
    let mut bytes = vec![0, length as u8];
    for s in steps {
        bytes.extend_from_slice(&[
            s.note,
            u8::from(s.slide),
            u8::from(s.accent),
            u8::from(s.up),
            u8::from(s.down),
            u8::from(s.rest),
        ]);
    }
    bytes
}

/// One bass line and a complementary answer in the same scale. Simultaneous notes use
/// unisons, thirds, fourths, fifths or sixths, avoiding seconds and tritones.
pub fn tb_pair(options: &Options, length: u8, seed: u64) -> [Vec<u8>; 2] {
    let length = usize::from(length.clamp(1, 16));
    let root = i32::from(options.root % 12);
    let scale = options.scale.intervals();
    let mut rng = Rng(seed);
    let third = scale.iter().position(|&n| n == 3 || n == 4).unwrap_or(1);
    let fifth = scale.iter().position(|&n| n == 7).unwrap_or(2);
    let chord = [0, 0, third, fifth];
    let mut motif = [0; 4];
    motif[1] = 1 + rng.pick(scale.len() - 1);
    motif[2] = chord[rng.pick(chord.len())];
    motif[3] = rng.pick(scale.len());
    let mut bass = [Step303::default(); 16];
    let mut answer = [Step303::default(); 16];
    for s in 0..length {
        let strong = s % 4 == 0;
        let call_half = s < length.div_ceil(2);
        let probability = options.density.saturating_add(if strong { 12 } else { 0 });
        let probability = if call_half || options.density >= 100 {
            probability
        } else {
            probability.saturating_sub(12)
        };
        if options.density == 0 || (s != 0 && !rng.chance(probability)) {
            continue;
        }
        let mut degree = motif[s % 4];
        if s != 0 && rng.chance(options.variation) {
            degree = (degree + scale.len() + rng.pick(3) - 1) % scale.len();
        }
        if strong {
            degree = chord[rng.pick(chord.len())];
        }
        if s == 0 {
            degree = 0;
        }
        let register = if rng.chance(12) { -12 } else { 0 };
        bass[s] = note(
            root + i32::from(scale[degree]) + register,
            options.accents > 0
                && rng.chance(options.accents.saturating_add(if strong { 20 } else { 0 })),
        );
    }
    for s in 0..length {
        let probability = if bass[s].rest {
            options.density.saturating_add(20)
        } else {
            options.density / 4
        };
        let probability = if s >= length / 2 {
            probability
        } else {
            probability / 2
        };
        if options.density == 0 || !rng.chance(probability) {
            continue;
        }
        let candidates: Vec<_> = scale
            .iter()
            .copied()
            .filter(|&interval| {
                bass[s].rest
                    || matches!(
                        (root + i32::from(interval) - pitch(bass[s])).rem_euclid(12),
                        0 | 3 | 4 | 5 | 7 | 8 | 9
                    )
            })
            .collect();
        let interval = candidates[rng.pick(candidates.len())];
        answer[s] = note(root + i32::from(interval) + 12, rng.chance(options.accents));
    }
    if options.density > 0 && answer[..length].iter().all(|s| s.rest) {
        let s = length / 2;
        let interval = if bass[s].rest {
            0
        } else {
            (pitch(bass[s]) - root).rem_euclid(12) as u8
        };
        answer[s] = note(root + i32::from(interval) + 12, rng.chance(options.accents));
    }
    finish_slides(&mut bass, length, options.slides, &mut rng);
    finish_slides(&mut answer, length, options.slides, &mut rng);
    [bytes303(&bass, length), bytes303(&answer, length)]
}

/// A structured techno or broken beat. Columns follow the original machine, including the
/// 909's different hat indices and its 0/1/2/3 velocity/flam values.
pub fn drum_pattern(options: &Options, is808: bool, length: u8, seed: u64) -> Vec<u8> {
    use orb_engine::{tr808, tr909};

    let length = usize::from(length.clamp(1, 16));
    let density = options.drum_density.min(100);
    let mut rng = Rng(seed);
    let mut bytes = vec![0; 194];
    bytes[1] = length as u8;
    let (closed, open) = if is808 {
        (tr808::CH, tr808::OH)
    } else {
        (tr909::CH, tr909::OH)
    };
    for s in 0..length {
        if density == 0 {
            continue;
        }
        let step = &mut bytes[2 + s * 12..14 + s * 12];
        let strong = s % 4 == 0;
        let kick = match options.drum_style {
            DrumStyle::Techno => strong || (s % 4 == 3 && rng.chance(density / 5)),
            DrumStyle::Broken => s == 0 || ((s % 8 == 3 || s % 8 == 6) && rng.chance(75)),
        };
        if kick {
            step[1] = if is808 { 1 } else { 2 };
        }
        if s % 8 == 4 || (s % 8 == 7 && rng.chance(density / 4)) {
            step[2] = if is808 {
                1
            } else if s % 8 == 4 {
                2
            } else {
                1
            };
            if rng.chance(density / 2) {
                step[7] = if is808 { 1 } else { 2 };
            }
            if !is808 && s % 8 == 7 && rng.chance(options.variation / 3) {
                step[2] = 3;
            }
        }
        if s % 4 == 2 && rng.chance(density.saturating_add(15)) {
            step[open] = 1;
        } else if (s % 2 == 0 && rng.chance(90)) || rng.chance(density / 2) {
            step[closed] = if is808 || s % 2 != 0 { 1 } else { 2 };
        }
        if !strong && rng.chance(density / 2) {
            let instruments = if is808 {
                [3, 4, 5, 6, 8]
            } else {
                [3, 4, 5, 6, 11]
            };
            let instrument = instruments[rng.pick(instruments.len())];
            step[instrument] = if is808 || rng.chance(65) { 1 } else { 2 };
        }
        if step[1..].iter().any(|&v| v != 0) && (strong || rng.chance(options.accents)) {
            step[0] = if is808 { 1 } else { 2 };
        }
    }
    bytes
}

#[derive(Clone)]
struct Change {
    row: usize,
    pattern: u8,
    before: Vec<u8>,
    after: Vec<u8>,
}

struct Undo {
    changes: Vec<Change>,
    tuning: Option<([i32; 2], [i32; 2])>,
}

pub struct Generator {
    pub open: bool,
    pub options: Options,
    seed: u64,
    undo: Option<Undo>,
    notice: Option<String>,
}

impl Default for Generator {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_nanos() as u64);
        Self {
            open: false,
            options: Options::default(),
            seed,
            undo: None,
            notice: None,
        }
    }
}

impl Generator {
    pub fn clear_undo(&mut self) {
        self.undo = None;
        self.notice = None;
    }
}
