//! Unencrypted wire encodings used over TLS between client and server:
//! command-log segments (client -> server) and verified state (server -> client).
//! Unlike `seal`/`open` these carry no key; the server must be able to read them.

use gl_sim::replay::Segment;
use gl_sim::World;
use sha2::{Digest, Sha256};

use crate::SaveError;

const MAX_STATE: usize = 64 * 1024 * 1024;
const MAX_SEGMENT: usize = 16 * 1024 * 1024;

fn inflate(bytes: &[u8], max: usize) -> Result<Vec<u8>, SaveError> {
    if bytes.len() < 4 || u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize > max {
        return Err(SaveError::Compress);
    }
    lz4_flex::decompress_size_prepended(bytes).map_err(|_| SaveError::Compress)
}

pub fn encode_segment(seg: &Segment) -> Result<Vec<u8>, SaveError> {
    let raw = postcard::to_allocvec(seg).map_err(|_| SaveError::Encode)?;
    Ok(lz4_flex::compress_prepend_size(&raw))
}

pub fn decode_segment(bytes: &[u8]) -> Result<Segment, SaveError> {
    postcard::from_bytes(&inflate(bytes, MAX_SEGMENT)?).map_err(|_| SaveError::Encode)
}

pub fn encode_state(world: &World) -> Result<Vec<u8>, SaveError> {
    let raw = postcard::to_allocvec(world).map_err(|_| SaveError::Encode)?;
    Ok(lz4_flex::compress_prepend_size(&raw))
}

pub fn decode_state(bytes: &[u8]) -> Result<World, SaveError> {
    postcard::from_bytes(&inflate(bytes, MAX_STATE)?).map_err(|_| SaveError::Encode)
}

/// SHA-256 of the canonical encoding. Equal digests mean equal worlds.
pub fn digest(world: &World) -> Result<[u8; 32], SaveError> {
    let raw = postcard::to_allocvec(world).map_err(|_| SaveError::Encode)?;
    Ok(Sha256::digest(raw).into())
}
