//! Final mix: one music module plus short effects, like the original SDL_mixer setup
//! (16 channels with one reserved, effects at `MIX_MAX_VOLUME / 4`).

use crate::{ModPlayer, Sound};
use std::sync::Arc;

/// SDL_mixer channels left for effects after `Mix_ReserveChannels(1)`.
pub const SFX_CHANNELS: usize = 15;
/// `Mix_VolumeChunk(sample, MIX_MAX_VOLUME / 4)`.
pub const SFX_GAIN: f32 = 0.25;
/// SgSetModVolume(20) through the SDL backend's `musicVol()`: 20/20 * 128/1.5.
pub const MUSIC_VOLUME: u8 = 85;

struct Voice {
    sound: Arc<Sound>,
    pos: f64,
    step: f64,
}

pub struct Mixer {
    rate: u32,
    music: Option<ModPlayer>,
    voices: Vec<Voice>,
    scratch: Vec<f32>,
    /// User volume sliders, 0..=1, applied on top of the original levels.
    pub music_level: f32,
    pub sfx_level: f32,
}

impl Mixer {
    pub fn new(rate: u32) -> Mixer {
        Mixer {
            rate: rate.max(1),
            music: None,
            voices: Vec::new(),
            scratch: Vec::new(),
            music_level: 1.0,
            sfx_level: 1.0,
        }
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// Starts a module, replacing any playing one (`SgPlayMod`).
    pub fn play_music(&mut self, mut p: ModPlayer) {
        p.set_volume(MUSIC_VOLUME);
        self.music = Some(p);
    }

    pub fn stop_music(&mut self) {
        self.music = None;
    }

    /// No module, or the module reached its end (`SgModFinished`).
    pub fn music_finished(&self) -> bool {
        self.music.as_ref().is_none_or(|m| m.finished())
    }

    /// `Mix_PlayChannel(-1, ...)`: first free channel, or dropped when all are busy.
    pub fn play(&mut self, sound: Arc<Sound>) {
        if self.voices.len() >= SFX_CHANNELS || sound.samples.is_empty() {
            return;
        }
        let step = sound.rate as f64 / self.rate as f64;
        self.voices.push(Voice {
            sound,
            pos: 0.0,
            step,
        });
    }

    pub fn active_effects(&self) -> usize {
        self.voices.len()
    }

    /// Fills interleaved stereo f32 frames.
    pub fn render(&mut self, out: &mut [f32]) {
        match self.music.as_mut() {
            Some(m) => {
                m.render(out);
                let g = self.music_level;
                if g != 1.0 {
                    out.iter_mut().for_each(|x| *x *= g);
                }
            }
            None => out.iter_mut().for_each(|x| *x = 0.0),
        }
        let g = SFX_GAIN * self.sfx_level;
        let frames = out.len() / 2;
        self.scratch.clear();
        self.scratch.resize(frames, 0.0);
        self.voices.retain_mut(|v| {
            let s = &v.sound.samples;
            for o in self.scratch.iter_mut() {
                let i = v.pos as usize;
                if i >= s.len() {
                    return false;
                }
                let a = s[i];
                let b = s.get(i + 1).copied().unwrap_or(0.0);
                *o += a + (b - a) * (v.pos - i as f64) as f32;
                v.pos += v.step;
            }
            (v.pos as usize) < s.len()
        });
        for (f, &x) in out.chunks_exact_mut(2).zip(self.scratch.iter()) {
            f[0] = (f[0] + x * g).clamp(-1.0, 1.0);
            f[1] = (f[1] + x * g).clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(n: usize, rate: u32) -> Arc<Sound> {
        Arc::new(Sound {
            rate,
            samples: vec![1.0; n],
        })
    }

    #[test]
    fn effects_play_at_quarter_volume_then_end() {
        let mut m = Mixer::new(22050);
        m.play(tone(100, 22050));
        let mut out = vec![0.0; 2 * 64];
        m.render(&mut out);
        assert!(out.iter().all(|&x| (x - SFX_GAIN).abs() < 1e-6));
        m.render(&mut out);
        assert_eq!(m.active_effects(), 0);
        assert!(out[2 * 40..].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn resamples_to_device_rate() {
        let mut m = Mixer::new(44100);
        m.play(tone(100, 22050));
        let mut out = vec![0.0; 2 * 400];
        m.render(&mut out);
        let audible = out.chunks(2).filter(|f| f[0] > 0.0).count();
        assert!((199..=201).contains(&audible), "{audible}");
    }

    #[test]
    fn channel_limit_and_level() {
        let mut m = Mixer::new(22050);
        for _ in 0..40 {
            m.play(tone(1000, 22050));
        }
        assert_eq!(m.active_effects(), SFX_CHANNELS);
        m.sfx_level = 0.0;
        let mut out = vec![0.5; 32];
        m.render(&mut out);
        assert!(out.iter().all(|&x| x == 0.0));
        assert!(m.music_finished());
    }
}
