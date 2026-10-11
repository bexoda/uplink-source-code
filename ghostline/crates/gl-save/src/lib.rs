//! Encrypted save container, local store and sync protocol for Ghostline.
//!
//! The same sealed blob is what lives on disk (desktop) and what is uploaded
//! to the sync server (web). Encryption happens on the client, so the server
//! stores opaque bytes and can never read a save.

mod format;
pub mod slot;
pub mod sync;
mod wire;

#[cfg(not(target_arch = "wasm32"))]
pub mod store;

#[cfg(all(feature = "os-keyring", not(target_arch = "wasm32")))]
pub mod keychain;

pub use format::{
    open, open_run, peek, seal, seal_run, Header, Key, SaveError, Unlock, FORMAT_VERSION,
    RUN_FORMAT_VERSION,
};
pub use wire::{decode_segment, decode_state, digest, encode_segment, encode_state};
