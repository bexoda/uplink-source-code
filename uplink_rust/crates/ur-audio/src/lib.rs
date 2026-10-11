//! uplink_rust sound: original Uplink effects (WAV) and music (MikMod UniMod), in pure Rust.
//!
//! No I/O and no audio device here. The client feeds bytes in and pulls f32 stereo frames out,
//! so the same code runs on desktop and in the browser.

pub mod mixer;
pub mod player;
pub mod uni;
pub mod wav;

pub use mixer::Mixer;
pub use player::ModPlayer;
pub use uni::Module;
pub use wav::Sound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    Truncated,
    Unsupported(String),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AudioError {}
