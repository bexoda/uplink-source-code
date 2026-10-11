//! Loads assets from the player's own Uplink installation (or the developer CD) at runtime.
//!
//! Nothing here embeds, copies or converts those files into this repository or into a build.
//! The game must work, with its built-in look, when they are absent. That keeps us aligned with
//! the Uplink Developers License, which ties modified versions to a purchased copy of Uplink.

mod archive;
mod image;
mod store;

pub use archive::{decode_wrapper, Archive};
pub use image::{decode_tiff, Rgba};
pub use store::Store;
#[cfg(not(target_arch = "wasm32"))]
pub use store::{candidates, locate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetError {
    /// File or archive entry doesn't exist.
    NotFound(String),
    BadArchive(String),
    BadImage(String),
    Io(String),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for AssetError {}
