//! Portable, versioned Studio container. Samples and IR are embedded, never external paths.

use std::sync::Arc;

use orb_engine::studio::audio_file::{MAX_AUDIO_BYTES, MAX_FRAMES, Wave};
use orb_engine::studio::{Settings, Studio, convolution::Convolver};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8; 8] = b"ORB338S2";
const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub names: [String; 8],
    pub ir_name: String,
    pub closed_hat: Option<u8>,
    pub sidechain: crate::sidechain::Settings,
    pub settings: Settings,
}

#[derive(Clone)]
pub struct Project {
    pub metadata: Metadata,
    pub song: Vec<u8>,
    pub waves: [Option<Arc<Wave>>; 8],
    pub ir: Arc<Wave>,
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, &x| {
        (h ^ u64::from(x)).wrapping_mul(0x100000001b3)
    })
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), String> {
    if out.len() + bytes.len() + 12 > MAX_BYTES {
        return Err("Studio project exceeds 256 MiB".into());
    }
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&checksum(bytes).to_le_bytes());
    out.extend_from_slice(bytes);
    Ok(())
}

fn take<'a>(bytes: &mut &'a [u8], limit: usize) -> Result<&'a [u8], String> {
    let header = bytes.get(..12).ok_or("Truncated Studio project")?;
    let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
    let hash = u64::from_le_bytes(header[4..12].try_into().unwrap());
    if len > limit {
        return Err("Studio project section exceeds its size limit".into());
    }
    let payload = bytes
        .get(12..12 + len)
        .ok_or("Truncated Studio project section")?;
    if checksum(payload) != hash {
        return Err("Studio project checksum mismatch".into());
    }
    *bytes = &bytes[12 + len..];
    Ok(payload)
}

impl Project {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        validate_bank(self.waves.iter().flatten().map(|w| w.frames.len()))?;
        if self.ir.frames.len() > 6 * 44100 {
            return Err("Impulse response exceeds six seconds".into());
        }
        let mut out = MAGIC.to_vec();
        let metadata = toml::to_string(&self.metadata).map_err(|e| e.to_string())?;
        put(&mut out, metadata.as_bytes())?;
        put(&mut out, &self.song)?;
        for wave in self
            .waves
            .iter()
            .map(Option::as_ref)
            .chain([Some(&self.ir)])
        {
            let mut bytes = Vec::new();
            if let Some(wave) = wave {
                bytes.reserve(wave.frames.len() * 8);
                for &x in wave.frames.iter().flatten() {
                    bytes.extend_from_slice(&x.to_le_bytes());
                }
            }
            put(&mut out, &bytes)?;
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_BYTES || bytes.get(..8) != Some(MAGIC) {
            return Err("Not a supported Studio project (ORB338S2)".into());
        }
        let mut bytes = &bytes[8..];
        let mut metadata: Metadata = toml::from_str(
            std::str::from_utf8(take(&mut bytes, 1024 * 1024)?).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        if metadata
            .settings
            .lanes
            .iter()
            .any(|l| l.steps.len() != 1024)
        {
            return Err("Each Studio lane must contain exactly 1024 steps".into());
        }
        metadata.settings.sanitize();
        metadata.sidechain.sanitize();
        metadata.closed_hat = metadata.closed_hat.map(|x| x.min(127));
        let song = take(&mut bytes, 16 * 1024 * 1024)?.to_vec();
        rbs::Song::parse(&song).map_err(|e| format!("Invalid embedded song: {e}"))?;
        let mut assets: [Option<Arc<Wave>>; 9] = std::array::from_fn(|_| None);
        let mut bank_frames = 0;
        for (i, slot) in assets.iter_mut().enumerate() {
            let raw = take(
                &mut bytes,
                if i == 8 {
                    6 * 44100 * 8
                } else {
                    MAX_FRAMES * 8
                },
            )?;
            if !raw.len().is_multiple_of(8) {
                return Err("Invalid embedded audio length".into());
            }
            if raw.is_empty() {
                continue;
            }
            if i < 8 {
                bank_frames += raw.len() / 8;
                validate_bank([bank_frames])?;
            }
            let mut frames = Vec::with_capacity(raw.len() / 8);
            for chunk in raw.as_chunks::<8>().0 {
                let frame = [
                    f32::from_le_bytes(chunk[..4].try_into().unwrap()),
                    f32::from_le_bytes(chunk[4..].try_into().unwrap()),
                ];
                if frame.iter().any(|x| !x.is_finite() || x.abs() > 8.0) {
                    return Err("Invalid embedded audio value".into());
                }
                frames.push(frame.map(|x| if x.abs() < 1e-20 { 0.0 } else { x }));
            }
            *slot = Some(Arc::new(Wave { frames }));
        }
        if !bytes.is_empty() {
            return Err("Unexpected trailing Studio project data".into());
        }
        let ir = assets[8].take().ok_or("Project has no impulse response")?;
        Ok(Self {
            metadata,
            song,
            waves: std::array::from_fn(|i| assets[i].take()),
            ir,
        })
    }

    pub fn prepare(&self) -> Result<Box<Studio>, String> {
        let mut studio = Box::new(Studio::new(Box::new(Convolver::new(&self.ir.frames)?)));
        studio.settings = Box::new(self.metadata.settings.clone());
        for i in 0..8 {
            studio.sampler.waves[i] = self.waves[i].as_ref().map(|w| Box::new((**w).clone()));
        }
        Ok(studio)
    }
}

fn validate_bank(lengths: impl IntoIterator<Item = usize>) -> Result<(), String> {
    let frames = lengths.into_iter().try_fold(0usize, usize::checked_add);
    if frames.is_none_or(|n| n > MAX_AUDIO_BYTES / 8) {
        return Err("Sampler bank exceeds the 240 MiB decoded-audio memory budget".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_budget_counts_all_lanes_without_imposing_a_duration_limit() {
        assert!(validate_bank([44100 * 181, 44100 * 181]).is_ok());
        assert!(validate_bank([MAX_FRAMES]).is_ok());
        assert!(validate_bank([MAX_FRAMES, 1]).is_err());
        assert!(validate_bank([usize::MAX, 1]).is_err());
    }

    #[test]
    fn portable_roundtrip_retains_last_step_and_embedded_audio_and_rejects_corruption() {
        let mut settings = Settings::default();
        settings.lanes[7].bars = 64;
        settings.lanes[7].steps[1023] = 109;
        let project = Project {
            metadata: Metadata {
                settings,
                names: std::array::from_fn(|i| format!("Lane {i}")),
                ir_name: "Test".into(),
                sidechain: Default::default(),
                closed_hat: Some(37),
            },
            song: rbs::Song::empty().write().unwrap(),
            waves: std::array::from_fn(|_| None),
            ir: Arc::new(Wave {
                frames: vec![[0.5, -0.25]; 64],
            }),
        };
        let bytes = project.encode().unwrap();
        let loaded = Project::decode(&bytes).unwrap();
        assert_eq!(loaded.metadata.settings, project.metadata.settings);
        assert_eq!(loaded.ir.frames, project.ir.frames);
        assert_eq!(loaded.metadata.closed_hat, Some(37));
        let mut broken = bytes.clone();
        *broken.last_mut().unwrap() ^= 1;
        assert!(Project::decode(&broken).is_err());
        for len in [0, 7, 8, 19, bytes.len() - 1] {
            assert!(Project::decode(&bytes[..len]).is_err());
        }
    }
}
