//! MP3 decoding runs entirely on the import worker. Only decoded PCM reaches the callback.

use super::audio_file::{MAX_FILE_BYTES, MAX_FRAMES, Wave, output_frames};
use std::io::{Cursor, ErrorKind};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

pub(super) fn decode(bytes: &[u8]) -> Result<Wave, String> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err("Audio file exceeds 256 MiB".into());
    }
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let options = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &options, &MetadataOptions::default())
        .map_err(|e| format!("Choose a valid WAV, AIFF or MP3: {e}"))?;
    let mut format = probed.format;
    let track = format.default_track().ok_or("MP3 has no audio track")?;
    let id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let mut frames = Vec::new();
    let mut rate = 0;
    let mut channels = 0;
    let mut errors = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("MP3 read failed: {e}")),
        };
        while !format.metadata().is_latest() {
            format.metadata().pop();
        }
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(Error::DecodeError(_)) if errors < 16 => {
                errors += 1;
                continue;
            }
            Err(e) => return Err(format!("MP3 decode failed: {e}")),
        };
        let spec = *decoded.spec();
        let count = spec.channels.count();
        if !(1..=2).contains(&count) || !(8000..=192000).contains(&spec.rate) {
            return Err("MP3 must be mono or stereo at a supported sample rate".into());
        }
        if rate != 0 && (rate != spec.rate as usize || channels != count) {
            return Err("MP3 changes sample rate or channel count mid-stream".into());
        }
        rate = spec.rate as usize;
        channels = count;
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        if buffer.samples().is_empty() {
            continue;
        }
        let count = frames.len() + buffer.samples().len() / channels;
        output_frames(count, rate)?;
        if count > frames.capacity() {
            // Keep allocation growth amortized while respecting the decoded-audio budget.
            let capacity = count.next_power_of_two().min(MAX_FRAMES);
            frames.reserve_exact(capacity - frames.len());
        }
        for samples in buffer.samples().chunks(channels) {
            let frame = [samples[0], samples[channels - 1]];
            if frame.iter().any(|x| !x.is_finite()) {
                return Err("Invalid MP3 audio value".into());
            }
            frames.push(frame.map(|x| {
                if x.abs() < 1e-20 {
                    0.0
                } else {
                    x.clamp(-8.0, 8.0)
                }
            }));
        }
    }
    Wave::from_frames(frames, rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_mpeg_one_layer_three_silence_without_external_files() {
        // Valid 128 kb/s, 44.1 kHz stereo Layer III frames, zero spectral coefficients.
        let mut frame = vec![0u8; 417];
        frame[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x00]);
        let bytes = frame.repeat(32);
        let wave = Wave::decode(&bytes).unwrap();
        assert!(wave.frames.len() >= 1152);
        assert!(wave.frames.iter().flatten().all(|x| x.is_finite()));
        assert!(Wave::decode(&bytes[..3]).is_err());
    }
}
