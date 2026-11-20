//! Distinct rhythmic grammars, with deterministic variation rather than unrelated random hits.

use super::{DrumStyle, Options, Rng};

pub(super) fn pattern(o: &Options, is808: bool, length: u8, seed: u64) -> Vec<u8> {
    let length = usize::from(length.clamp(1, 16));
    let density = o.drum_density.min(100);
    let variation = o.variation.min(100);
    let mut rng = Rng(seed);
    let mut bytes = vec![0; 194];
    bytes[1] = length as u8;
    if density == 0 {
        return bytes;
    }
    let (closed, open) = if is808 { (11, 10) } else { (8, 9) };
    let dnb = rng.chance(50);
    let phase = rng.pick(3);
    let euclid = 3 + usize::from(density) / 22;
    let put = |step: &mut [u8], at: usize, velocity| {
        step[at] = if is808 { 1 } else { velocity };
    };
    for s in 0..length {
        let step = &mut bytes[2 + 12 * s..14 + 12 * s];
        match o.drum_style {
            DrumStyle::Techno => {
                if s % 4 == 0 || (s == 15 && rng.chance(variation / 5)) {
                    put(step, 1, 2);
                }
                if s % 8 == 4 && rng.chance(density.saturating_add(20)) {
                    put(step, 2, 2);
                    if rng.chance(density / 2) {
                        put(step, 7, 1);
                    }
                } else if s % 4 == 3 && rng.chance(density / 5) {
                    put(step, 2, 1);
                }
                if s % 4 == 2 && rng.chance(density.saturating_add(20)) {
                    put(step, open, 1);
                } else if s % 2 == 0 || rng.chance(density / 3) {
                    put(step, closed, if s % 2 == 0 { 2 } else { 1 });
                }
                if (s + phase).is_multiple_of(3) && s % 4 != 0 && rng.chance(density) {
                    let instruments = if is808 { [3, 4, 6, 8] } else { [3, 4, 6, 11] };
                    put(step, instruments[rng.pick(instruments.len())], 1);
                }
            }
            DrumStyle::Idm => {
                let pulse = ((s * euclid + phase) % 16) < euclid;
                if s == 0 || (pulse && rng.chance(density.saturating_add(10))) {
                    put(step, 1, if s == 0 || rng.chance(35) { 2 } else { 1 });
                }
                if (s % 5 == 2 || s == 12) && rng.chance(density.saturating_add(15)) {
                    put(
                        step,
                        2,
                        if !is808 && rng.chance(variation / 2) {
                            3
                        } else if s == 12 {
                            2
                        } else {
                            1
                        },
                    );
                }
                if rng.chance(density) {
                    put(
                        step,
                        if rng.chance(18) { open } else { closed },
                        if !is808 && rng.chance(variation / 3) {
                            3
                        } else {
                            1
                        },
                    );
                }
                if s % 4 != 0 && rng.chance(density.saturating_add(10)) {
                    let instruments = if is808 {
                        [3, 5, 6, 8, 9]
                    } else {
                        [3, 5, 6, 10, 11]
                    };
                    put(
                        step,
                        instruments[rng.pick(instruments.len())],
                        if !is808 && rng.chance(variation / 2) {
                            3
                        } else {
                            1
                        },
                    );
                }
            }
            DrumStyle::Break => {
                let anchor = s == 0 || s == 10 || (!dnb && s == 6);
                if anchor || ([3, 7, 14].contains(&s) && rng.chance(density / 3)) {
                    put(step, 1, if anchor { 2 } else { 1 });
                }
                if s == 4 || s == 12 {
                    put(step, 2, 2);
                } else if [3, 7, 11, 14, 15].contains(&s) && rng.chance(density / 2) {
                    put(
                        step,
                        2,
                        if !is808 && s == 15 && rng.chance(variation / 2) {
                            3
                        } else {
                            1
                        },
                    );
                }
                if (s == 2 || s == 14) && rng.chance(density) {
                    put(step, open, 1);
                } else if s % 2 == 0 || rng.chance(if dnb { density } else { density / 2 }) {
                    put(step, closed, if s % 4 == 0 { 2 } else { 1 });
                }
                if s % 8 == 4 && rng.chance(density / 2) {
                    put(step, 7, 1);
                }
                if s >= 12 && s % 2 != 0 && rng.chance(variation / 2) {
                    put(step, 3 + rng.pick(3), 1);
                }
            }
        }
        if s == 0 {
            put(step, 1, 2);
        }
        if o.accents > 0
            && step[1..].iter().any(|&v| v != 0)
            && (s % 4 == 0 || rng.chance(o.accents))
        {
            step[0] = if is808 { 1 } else { 2 };
        }
    }
    bytes
}
