//! Musical pattern generation. Only pattern data changes; synthesis stays in orb-engine.
//! Seeded generators are deterministic so musical constraints can be checked without audio I/O.

mod rhythms;
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
    Idm,
    Break,
}

impl DrumStyle {
    pub const ALL: [Self; 3] = [Self::Techno, Self::Idm, Self::Break];

    pub fn name(self) -> &'static str {
        match self {
            Self::Techno => "Techno",
            Self::Idm => "IDM",
            Self::Break => "Break",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Techno => "Hypnotic four-on-the-floor, offbeat hats and interlocking percussion.",
            Self::Idm => "Fractured pulses, deliberate gaps, ghost notes and metallic detail.",
            Self::Break => "Syncopated kicks, snare backbeats and broken / drum-and-bass motion.",
        }
    }
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
        let strong = match options.drum_style {
            DrumStyle::Techno => s % 4 == 0,
            DrumStyle::Idm => (s * 5) % length < 3,
            DrumStyle::Break => s % 8 == 2 || s % 8 == 6,
        };
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
        if options.drum_style == DrumStyle::Idm && rng.chance(options.variation) {
            degree = rng.pick(scale.len());
        }
        if s != 0 && rng.chance(options.variation) {
            degree = (degree + scale.len() + rng.pick(3) - 1) % scale.len();
        }
        if strong {
            degree = chord[rng.pick(chord.len())];
        }
        if s == 0 {
            degree = 0;
        }
        let register = match options.drum_style {
            DrumStyle::Idm if rng.chance(35) => 12,
            _ if rng.chance(12) => -12,
            _ => 0,
        };
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

fn steps303(bytes: &[u8]) -> [Step303; 16] {
    std::array::from_fn(|s| {
        let b = &bytes[2 + s * 6..8 + s * 6];
        Step303 {
            note: b[0],
            slide: b[1] != 0,
            accent: b[2] != 0,
            up: b[3] != 0,
            down: b[4] != 0,
            rest: b[5] != 0,
        }
    })
}

fn phase_candidates(
    scale: &[u8],
    root: i32,
    bass: &[Step303; 16],
    length: usize,
    phase: usize,
    stride: usize,
) -> Vec<u8> {
    scale
        .iter()
        .copied()
        .filter(|&interval| {
            (phase..length).step_by(stride).all(|s| {
                bass[s].rest
                    || matches!(
                        (root + i32::from(interval) - pitch(bass[s])).rem_euclid(12),
                        0 | 3 | 4 | 5 | 7 | 8 | 9
                    )
            })
        })
        .collect()
}

/// Linked lines with independent loop lengths. A step of A can meet a step of B only when
/// their indices agree modulo gcd(length A, length B). Each answer note is therefore chosen
/// against every bass note it will encounter over the complete common cycle, not just its
/// first occurrence. If that intersection is empty, that phase's bass notes move to the
/// nearest tonic-triad notes, keeping rhythm, accents and register. The selected scales all
/// contain that triad, which guarantees a bounded, consonant fallback even for coprime loops.
pub fn tb_pair_lengths(options: &Options, lengths: [u8; 2], seed: u64) -> [Vec<u8>; 2] {
    let lengths = lengths.map(|n| n.clamp(1, 16));
    if lengths[0] == lengths[1] {
        return tb_pair(options, lengths[0], seed);
    }
    let [a_len, b_len] = lengths.map(usize::from);
    let (mut divisor, mut remainder) = (a_len, b_len);
    while remainder != 0 {
        (divisor, remainder) = (remainder, divisor % remainder);
    }
    let root = i32::from(options.root % 12);
    let scale = options.scale.intervals();
    let mut bass = steps303(&tb_pair(options, lengths[0], seed)[0]);
    let mut answer = [Step303::default(); 16];
    let mut rng = Rng(seed ^ 0x6a09_e667_f3bc_c909);
    let third = if scale.contains(&3) { 3 } else { 4 };
    let mut palettes = Vec::new();
    let mut activity = Vec::new();
    for phase in 0..divisor {
        let mut candidates = phase_candidates(scale, root, &bass, a_len, phase, divisor);
        if candidates.is_empty() {
            for s in (phase..a_len).step_by(divisor) {
                if bass[s].rest {
                    continue;
                }
                let previous = bass[s];
                let old_pitch = pitch(previous);
                let nearest = [0, third, 7]
                    .into_iter()
                    .flat_map(|interval| {
                        (-1..=1).map(move |octave| (root + interval).rem_euclid(12) + 12 * octave)
                    })
                    .min_by_key(|p| (*p - old_pitch).abs())
                    .unwrap();
                bass[s] = note(nearest, previous.accent);
                bass[s].slide = previous.slide;
            }
            candidates = phase_candidates(scale, root, &bass, a_len, phase, divisor);
        }
        let voiced = (phase..a_len)
            .step_by(divisor)
            .filter(|&s| !bass[s].rest)
            .count();
        let total = (phase..a_len).step_by(divisor).count();
        activity.push((voiced, total));
        palettes.push(candidates);
    }
    let motif: [usize; 4] = std::array::from_fn(|_| rng.pick(scale.len()));
    for s in 0..b_len {
        let phase = s % divisor;
        let (voiced, total) = activity[phase];
        let density = usize::from(options.density.min(100));
        let high = (density + 20).min(100);
        let low = density / 4;
        let probability = high - (high - low) * voiced / total;
        let probability = if s >= b_len / 2 {
            probability
        } else {
            probability / 2
        };
        if density == 0 || !rng.chance(probability as u8) {
            continue;
        }
        let palette = &palettes[phase];
        let index = if rng.chance(options.variation) {
            rng.pick(palette.len())
        } else {
            motif[s % motif.len()] % palette.len()
        };
        answer[s] = note(
            root + i32::from(palette[index]) + 12,
            rng.chance(options.accents),
        );
    }
    if options.density > 0 && answer[..b_len].iter().all(|s| s.rest) {
        let s = b_len / 2;
        let palette = &palettes[s % divisor];
        answer[s] = note(
            root + i32::from(palette[rng.pick(palette.len())]) + 12,
            rng.chance(options.accents),
        );
    }
    finish_slides(&mut answer, b_len, options.slides, &mut rng);
    [bytes303(&bass, a_len), bytes303(&answer, b_len)]
}

/// A structured techno or broken beat. Columns follow the original machine, including the
/// 909's different hat indices and its 0/1/2/3 velocity/flam values.
pub fn drum_pattern(options: &Options, is808: bool, length: u8, seed: u64) -> Vec<u8> {
    rhythms::pattern(options, is808, length, seed)
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
