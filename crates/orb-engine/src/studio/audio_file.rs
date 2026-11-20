//! Bounded WAV/AIFF import and offline bandlimited resampling. No callback file I/O.

pub const RATE: usize = 44100;
/// RAM budget, shared by imported audio and the portable sampler bank. No duration limit.
pub const MAX_AUDIO_BYTES: usize = 240 * 1024 * 1024;
pub const MAX_FILE_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_FRAMES: usize = MAX_AUDIO_BYTES / std::mem::size_of::<[f32; 2]>();

pub(super) fn output_frames(count: usize, rate: usize) -> Result<usize, String> {
    if count == 0 || !(8000..=192000).contains(&rate) {
        return Err("Sample must contain audio at a supported sample rate".into());
    }
    if count > MAX_FRAMES {
        return Err("Decoded audio exceeds the 240 MiB memory budget".into());
    }
    let output = (count as u64 * RATE as u64 / rate as u64) as usize;
    if output == 0 || output > MAX_FRAMES {
        return Err("Decoded audio exceeds the 240 MiB memory budget".into());
    }
    Ok(output)
}

#[derive(Clone)]
pub struct Wave {
    pub frames: Vec<[f32; 2]>,
}

fn word(b: &[u8], at: usize, n: usize, big: bool) -> Result<u64, String> {
    let s = b
        .get(at..at.checked_add(n).ok_or("Invalid audio offset")?)
        .ok_or("Truncated audio file")?;
    Ok(if big {
        s.iter().fold(0, |v, &x| (v << 8) | u64::from(x))
    } else {
        s.iter().rev().fold(0, |v, &x| (v << 8) | u64::from(x))
    })
}

impl Wave {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("Audio file exceeds 256 MiB".into());
        }
        let wav = bytes.get(..4) == Some(b"RIFF") && bytes.get(8..12) == Some(b"WAVE");
        let aiff = bytes.get(..4) == Some(b"FORM") && bytes.get(8..12) == Some(b"AIFF");
        if !wav && !aiff {
            return super::mp3::decode(bytes);
        }
        let mut channels = 0usize;
        let mut bits = 0usize;
        let mut rate = 0usize;
        let mut float = false;
        let mut data = None;
        let mut at = 12usize;
        let declared = word(bytes, 4, 4, aiff)? as usize + 8;
        if declared > bytes.len() {
            return Err("Truncated RIFF/FORM container".into());
        }
        while at + 8 <= declared {
            let len = word(bytes, at + 4, 4, aiff)? as usize;
            let end = at.checked_add(8 + len).ok_or("Invalid audio chunk")?;
            if end > declared {
                return Err("Truncated audio chunk".into());
            }
            let chunk = &bytes[at + 8..end];
            match &bytes[at..at + 4] {
                b"fmt " if wav => {
                    let mut format = word(chunk, 0, 2, false)?;
                    channels = word(chunk, 2, 2, false)? as usize;
                    rate = word(chunk, 4, 4, false)? as usize;
                    bits = word(chunk, 14, 2, false)? as usize;
                    if format == 0xfffe {
                        format = word(chunk, 24, 2, false)?;
                    }
                    if format != 1 && format != 3 {
                        return Err("Compressed WAV is unsupported".into());
                    }
                    float = format == 3;
                    if word(chunk, 12, 2, false)? as usize != channels * bits.div_ceil(8) {
                        return Err("Invalid WAV block alignment".into());
                    }
                }
                b"data" if wav => data = Some(chunk),
                b"COMM" if aiff => {
                    channels = word(chunk, 0, 2, true)? as usize;
                    bits = word(chunk, 6, 2, true)? as usize;
                    let exponent = word(chunk, 8, 2, true)? as i32;
                    let mantissa = word(chunk, 10, 8, true)? as f64;
                    let value = mantissa * 2.0f64.powi(exponent - 16383 - 63);
                    if !value.is_finite() || !(8000.0..=192000.0).contains(&value) {
                        return Err("Unsupported AIFF sample rate".into());
                    }
                    rate = value.round() as usize;
                }
                b"SSND" if aiff => {
                    let offset = word(chunk, 0, 4, true)? as usize;
                    data = Some(chunk.get(8 + offset..).ok_or("Invalid AIFF sound offset")?);
                }
                _ => {}
            }
            at = end + (len & 1);
        }
        if !(1..=2).contains(&channels)
            || !(8000..=192000).contains(&rate)
            || !matches!(bits, 8 | 16 | 24 | 32 | 64)
            || (float && !matches!(bits, 32 | 64))
            || (!float && bits == 64)
        {
            return Err("Use mono/stereo PCM 8/16/24/32-bit or float 32/64-bit audio".into());
        }
        let data = data.ok_or("Audio file has no sound data")?;
        let width = bits / 8;
        let stride = width * channels;
        let count = data.len() / stride;
        if !data.len().is_multiple_of(stride) {
            return Err("Incomplete audio frame".into());
        }
        output_frames(count, rate)?;
        let mut input = Vec::with_capacity(count);
        for i in 0..count {
            let mut frame = [0.0; 2];
            for (c, value) in frame.iter_mut().enumerate().take(channels) {
                let raw = word(data, i * stride + c * width, width, aiff)?;
                let x = if float {
                    if bits == 32 {
                        f64::from(f32::from_bits(raw as u32))
                    } else {
                        f64::from_bits(raw)
                    }
                } else if bits == 8 && wav {
                    (raw as f64 - 128.0) / 128.0
                } else {
                    ((raw << (64 - bits)) as i64 >> (64 - bits)) as f64
                        / (1u64 << (bits - 1)) as f64
                };
                if !x.is_finite() {
                    return Err("Sample contains NaN or infinite audio".into());
                }
                *value = if x.abs() < 1e-20 {
                    0.0
                } else {
                    x.clamp(-8.0, 8.0) as f32
                };
            }
            if channels == 1 {
                frame[1] = frame[0];
            }
            input.push(frame);
        }
        Self::from_frames(input, rate)
    }

    pub(super) fn from_frames(input: Vec<[f32; 2]>, rate: usize) -> Result<Self, String> {
        let count = output_frames(input.len(), rate)?;
        if rate == RATE {
            return Ok(Self { frames: input });
        }
        let mut frames = Vec::with_capacity(count);
        let cutoff = (RATE as f64 / rate as f64).min(1.0) * 0.95;
        let kernels: Vec<[f64; 64]> = (0..1024)
            .map(|phase| {
                let mut weights = std::array::from_fn(|tap| {
                    let distance = tap as f64 - 31.0 - f64::from(phase) / 1024.0;
                    sinc(distance * cutoff)
                        * (0.5 + 0.5 * (distance * std::f64::consts::PI / 33.0).cos())
                });
                let norm: f64 = weights.iter().sum();
                for weight in &mut weights {
                    *weight /= norm;
                }
                weights
            })
            .collect();
        for i in 0..count {
            let position = i as f64 * rate as f64 / RATE as f64;
            let center = position.floor() as isize;
            let phase = ((position - position.floor()) * 1024.0) as usize;
            let mut sum = [0.0f64; 2];
            for (tap, &weight) in kernels[phase.min(1023)].iter().enumerate() {
                let index =
                    (center + tap as isize - 31).clamp(0, input.len() as isize - 1) as usize;
                for (c, value) in sum.iter_mut().enumerate() {
                    *value += f64::from(input[index][c]) * weight;
                }
            }
            frames.push(sum.map(|x| x.clamp(-8.0, 8.0) as f32));
        }
        Ok(Self { frames })
    }
}

pub(super) fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let a = x * std::f64::consts::PI;
        a.sin() / a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_length_follows_memory_instead_of_a_three_minute_timer() {
        assert_eq!(output_frames(RATE * 181, RATE).unwrap(), RATE * 181);
        assert_eq!(output_frames(RATE * 600, RATE).unwrap(), RATE * 600);
        assert_eq!(output_frames(48000 * 181, 48000).unwrap(), RATE * 181);
        assert_eq!(output_frames(MAX_FRAMES, RATE).unwrap(), MAX_FRAMES);
        assert!(output_frames(MAX_FRAMES + 1, RATE).is_err());
        assert!(output_frames(MAX_FRAMES, 8000).is_err());
        assert!(output_frames(0, RATE).is_err());
        assert!(output_frames(100, 0).is_err());
    }

    fn wav(bits: u16, format: u16, payload: &[u8]) -> Vec<u8> {
        let mut out = b"RIFF".to_vec();
        out.extend_from_slice(&(36 + payload.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt \x10\0\0\0");
        out.extend_from_slice(&format.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&44100u32.to_le_bytes());
        out.extend_from_slice(&(44100u32 * u32::from(bits / 8)).to_le_bytes());
        out.extend_from_slice(&(bits / 8).to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out
    }

    #[test]
    fn pcm_endpoints_mono_duplication_and_invalid_float() {
        let w = Wave::decode(&wav(16, 1, &[0, 128, 255, 127])).unwrap();
        assert_eq!(w.frames[0], [-1.0; 2]);
        assert_eq!(w.frames[1], [32767.0 / 32768.0; 2]);
        assert!(Wave::decode(&wav(32, 3, &f32::NAN.to_le_bytes())).is_err());
        for n in 0..44 {
            assert!(Wave::decode(&wav(16, 1, &[0, 0])[..n]).is_err());
        }
    }

    #[test]
    fn resampling_preserves_duration_and_dc_level() {
        let payload: Vec<u8> = (0..480).flat_map(|_| 16384i16.to_le_bytes()).collect();
        let mut bytes = wav(16, 1, &payload);
        bytes[24..28].copy_from_slice(&48000u32.to_le_bytes());
        bytes[28..32].copy_from_slice(&96000u32.to_le_bytes());
        let wave = Wave::decode(&bytes).unwrap();
        assert_eq!(wave.frames.len(), 441);
        assert!(
            wave.frames
                .iter()
                .flatten()
                .all(|&x| (x - 0.5).abs() < 1e-6)
        );
    }
}
