//! Minimal RIFF/WAVE reader for the original sound effects: PCM, 8 or 16 bit, mono or stereo.

use crate::AudioError;

/// Decoded effect, mono f32 in -1..1 at its own sample rate (stereo files are downmixed).
#[derive(Debug, Clone)]
pub struct Sound {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Sound {
    pub fn decode(b: &[u8]) -> Result<Sound, AudioError> {
        if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
            return Err(AudioError::Unsupported("not a RIFF/WAVE file".into()));
        }
        let mut at = 12;
        let mut fmt: Option<(u16, u16, u32, u16)> = None;
        while at + 8 <= b.len() {
            let id = &b[at..at + 4];
            let len = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
            let body = at + 8;
            // Some writers put a short data length; never read past the file.
            let end = body.saturating_add(len).min(b.len());
            if id == b"fmt " {
                if end - body < 16 {
                    return Err(AudioError::Truncated);
                }
                let f = &b[body..end];
                fmt = Some((
                    u16::from_le_bytes([f[0], f[1]]),
                    u16::from_le_bytes([f[2], f[3]]),
                    u32::from_le_bytes([f[4], f[5], f[6], f[7]]),
                    u16::from_le_bytes([f[14], f[15]]),
                ));
            } else if id == b"data" {
                let (tag, ch, rate, bits) = fmt.ok_or(AudioError::Truncated)?;
                if tag != 1 || !(1..=2).contains(&ch) || rate == 0 {
                    return Err(AudioError::Unsupported(format!(
                        "wav tag {tag} ch {ch} rate {rate}"
                    )));
                }
                let raw = &b[body..end];
                let mono: Vec<f32> = match bits {
                    8 => raw.iter().map(|&x| (x as f32 - 128.0) / 128.0).collect(),
                    16 => raw
                        .chunks_exact(2)
                        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                        .collect(),
                    _ => return Err(AudioError::Unsupported(format!("{bits}-bit wav"))),
                };
                let samples = if ch == 2 {
                    mono.chunks_exact(2).map(|p| (p[0] + p[1]) * 0.5).collect()
                } else {
                    mono
                };
                return Ok(Sound { rate, samples });
            }
            at = body + len + (len & 1);
        }
        Err(AudioError::Truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(bits: u16, ch: u16, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend(b"RIFF");
        v.extend((36 + data.len() as u32).to_le_bytes());
        v.extend(b"WAVEfmt ");
        v.extend(16u32.to_le_bytes());
        v.extend(1u16.to_le_bytes());
        v.extend(ch.to_le_bytes());
        v.extend(22050u32.to_le_bytes());
        v.extend((22050 * ch as u32 * bits as u32 / 8).to_le_bytes());
        v.extend((ch * bits / 8).to_le_bytes());
        v.extend(bits.to_le_bytes());
        v.extend(b"data");
        v.extend((data.len() as u32).to_le_bytes());
        v.extend(data);
        v
    }

    #[test]
    fn decodes_8_and_16_bit() {
        let s = Sound::decode(&wav(8, 1, &[128, 255, 0])).unwrap();
        assert_eq!(s.rate, 22050);
        assert_eq!(s.samples, vec![0.0, 127.0 / 128.0, -1.0]);
        let s = Sound::decode(&wav(16, 1, &[0x00, 0x40, 0x00, 0xc0])).unwrap();
        assert_eq!(s.samples, vec![0.5, -0.5]);
        let s = Sound::decode(&wav(16, 2, &[0x00, 0x40, 0x00, 0x00])).unwrap();
        assert_eq!(s.samples, vec![0.25]);
    }

    #[test]
    fn rejects_garbage_and_truncation() {
        assert!(Sound::decode(b"nope").is_err());
        let mut w = wav(16, 1, &[1, 2, 3, 4]);
        w.truncate(30);
        assert!(Sound::decode(&w).is_err());
        // A data chunk that claims more bytes than exist is clipped, not a panic.
        let mut w = wav(16, 1, &[1, 2, 3, 4]);
        let n = w.len();
        w[n - 8..n - 4].copy_from_slice(&1000u32.to_le_bytes());
        assert_eq!(Sound::decode(&w).unwrap().samples.len(), 2);
    }
}
