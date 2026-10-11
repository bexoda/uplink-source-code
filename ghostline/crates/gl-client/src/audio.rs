//! Sound for the original interface: the effects from sounds.dat at the moments the original
//! played them, and the music.dat modules in the original "main" playlist order.
//!
//! Mixing is pure Rust (gl-audio); cpal only carries the samples to the device, on desktop and in
//! the browser. No device, no data, or `GHOSTLINE_AUDIO=off` all mean silence, never an error.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gl_assets::Store;
use gl_audio::{Mixer, ModPlayer, Module, Sound};

/// One `SgPlaySound` call site in the original, by the file it played.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cue {
    /// `button_click`: pressing any button.
    MouseClick,
    /// Opening the software menu or one of its categories.
    SoftwareMenu,
    /// Running a program from the software menu.
    RunSoftware,
    /// Adding a bounce to the connection on the world map.
    AddLink,
    /// Closing the world map or a local screen.
    Close,
    /// Getting past a remote system's login.
    Login,
    /// A password breaker finished.
    Done,
    /// A mission was completed and paid.
    Success,
    /// A purchase was refused.
    Failure,
    /// A new message or mission arrived.
    NewMail,
    /// Trace Tracker pulse while a trace is running.
    TraceBleep,
}

impl Cue {
    pub fn file(self) -> &'static str {
        match self {
            Cue::MouseClick => "sounds/mouseclick.wav",
            Cue::SoftwareMenu => "sounds/softwaremenu.wav",
            Cue::RunSoftware => "sounds/runsoftware.wav",
            Cue::AddLink => "sounds/addlink.wav",
            Cue::Close => "sounds/close.wav",
            Cue::Login => "sounds/login.wav",
            Cue::Done => "sounds/done.wav",
            Cue::Success => "sounds/success.wav",
            Cue::Failure => "sounds/failure.wav",
            Cue::NewMail => "sounds/newmail.wav",
            Cue::TraceBleep => "sounds/tracebleep.wav",
        }
    }
}

/// The original's "main" playlist (uplink.cpp `Init_Music`, full game).
pub const PLAYLIST: [&str; 6] = [
    "music/bluevalley.uni",
    "music/serenity.uni",
    "music/mystique.uni",
    "music/a94final.uni",
    "music/symphonic.uni",
    "music/myst2.uni",
];

pub struct Audio {
    mixer: Arc<Mutex<Mixer>>,
    out: Option<output::Output>,
    store: Store,
    sounds: HashMap<Cue, Option<Arc<Sound>>>,
    /// Index of the song playing now; the next one follows when it ends.
    song: Option<usize>,
    pub music_on: bool,
}

impl Audio {
    /// Opens the default output device. Without one the struct still works and stays silent.
    pub fn new(store: Store) -> Audio {
        let disabled = std::env::var("GHOSTLINE_AUDIO").is_ok_and(|v| v == "off" || v == "0");
        let out = if disabled {
            None
        } else {
            output::Output::open()
        };
        let rate = out.as_ref().map_or(44_100, |o| o.rate);
        let mixer = Arc::new(Mutex::new(Mixer::new(rate)));
        let out = out.and_then(|o| o.start(mixer.clone()));
        Audio {
            mixer,
            out,
            store,
            sounds: HashMap::new(),
            song: None,
            music_on: !disabled,
        }
    }

    /// True when samples actually reach a device.
    pub fn live(&self) -> bool {
        self.out.is_some()
    }

    pub fn play(&mut self, cue: Cue) {
        let store = &mut self.store;
        let sound = self
            .sounds
            .entry(cue)
            .or_insert_with(|| {
                store
                    .read("sounds.dat", cue.file())
                    .ok()
                    .and_then(|b| Sound::decode(&b).ok())
                    .map(Arc::new)
            })
            .clone();
        if let (Some(s), true) = (sound, self.live()) {
            if let Ok(mut m) = self.mixer.lock() {
                m.play(s);
            }
        }
    }

    /// Call once per frame: starts the next song when one ends (`SgUpdate`).
    pub fn update(&mut self) {
        if let Some(o) = &self.out {
            o.resume();
        }
        if !self.live() {
            return;
        }
        if !self.music_on {
            if self.song.take().is_some() {
                if let Ok(mut m) = self.mixer.lock() {
                    m.stop_music();
                }
            }
            return;
        }
        let finished = self
            .mixer
            .lock()
            .map(|m| m.music_finished())
            .unwrap_or(false);
        if !finished {
            return;
        }
        // Try each song once; a damaged archive must not spin here every frame.
        for _ in 0..PLAYLIST.len() {
            let next = self.song.map_or(0, |i| (i + 1) % PLAYLIST.len());
            self.song = Some(next);
            let rate = self.mixer.lock().map(|m| m.rate()).unwrap_or(44_100);
            let loaded = self
                .store
                .read("music.dat", PLAYLIST[next])
                .ok()
                .and_then(|b| Module::load(&b).ok());
            if let Some(module) = loaded {
                if let Ok(mut m) = self.mixer.lock() {
                    m.play_music(ModPlayer::new(module, rate));
                }
                return;
            }
        }
        self.music_on = false;
    }
}

/// The device side: cpal on both desktop and wasm (Web Audio).
mod output {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

    pub struct Output {
        pub rate: u32,
        device: Option<cpal::Device>,
        config: Option<cpal::SupportedStreamConfig>,
        stream: Option<cpal::Stream>,
    }

    impl Output {
        pub fn open() -> Option<Output> {
            let device = cpal::default_host().default_output_device()?;
            let config = device.default_output_config().ok()?;
            Some(Output {
                rate: config.sample_rate(),
                device: Some(device),
                config: Some(config),
                stream: None,
            })
        }

        pub fn start(mut self, mixer: Arc<Mutex<Mixer>>) -> Option<Output> {
            let device = self.device.take()?;
            let config = self.config.take()?;
            let channels = config.channels() as usize;
            let sc: cpal::StreamConfig = config.config();
            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => build::<f32>(&device, sc, channels, mixer),
                cpal::SampleFormat::I16 => build::<i16>(&device, sc, channels, mixer),
                cpal::SampleFormat::U16 => build::<u16>(&device, sc, channels, mixer),
                cpal::SampleFormat::I32 => build::<i32>(&device, sc, channels, mixer),
                _ => None,
            }?;
            stream.play().ok()?;
            self.stream = Some(stream);
            Some(self)
        }

        /// Browsers start audio suspended until the page gets a gesture; asking again is cheap.
        pub fn resume(&self) {
            #[cfg(target_arch = "wasm32")]
            if let Some(s) = &self.stream {
                let _ = s.play();
            }
        }
    }

    fn build<T>(
        device: &cpal::Device,
        config: cpal::StreamConfig,
        channels: usize,
        mixer: Arc<Mutex<Mixer>>,
    ) -> Option<cpal::Stream>
    where
        T: cpal::SizedSample + cpal::FromSample<f32>,
    {
        let mut stereo: Vec<f32> = Vec::new();
        device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    let frames = data.len() / channels.max(1);
                    stereo.resize(frames * 2, 0.0);
                    match mixer.lock() {
                        Ok(mut m) => m.render(&mut stereo),
                        Err(_) => stereo.iter_mut().for_each(|x| *x = 0.0),
                    }
                    for (f, s) in data.chunks_mut(channels.max(1)).zip(stereo.chunks(2)) {
                        match f.len() {
                            1 => f[0] = T::from_sample((s[0] + s[1]) * 0.5),
                            _ => {
                                f[0] = T::from_sample(s[0]);
                                f[1] = T::from_sample(s[1]);
                                for x in f.iter_mut().skip(2) {
                                    *x = T::from_sample(0.0);
                                }
                            }
                        }
                    }
                },
                |_err| {},
                None,
            )
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_cue_names_an_original_sound() {
        for c in [
            Cue::MouseClick,
            Cue::SoftwareMenu,
            Cue::RunSoftware,
            Cue::AddLink,
            Cue::Close,
            Cue::Login,
            Cue::Done,
            Cue::Success,
            Cue::Failure,
            Cue::NewMail,
            Cue::TraceBleep,
        ] {
            assert!(c.file().starts_with("sounds/") && c.file().ends_with(".wav"));
        }
    }

    #[test]
    fn silent_without_a_device_or_data() {
        std::env::set_var("GHOSTLINE_AUDIO", "off");
        let mut a = Audio::new(Store::from_memory(HashMap::new()));
        assert!(!a.live());
        a.play(Cue::Done);
        a.update();
        assert!(a.song.is_none());
    }
}
