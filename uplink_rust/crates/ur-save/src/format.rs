use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use serde::{de::DeserializeOwned, Serialize};
use ur_sim::replay::Run;
use ur_sim::{World, SCHEMA_VERSION};

/// Format 1: payload is a bare `World` (server-held state). Format 2: payload is a `Run` (client saves).
pub const FORMAT_VERSION: u16 = 1;
pub const RUN_FORMAT_VERSION: u16 = 2;
const MAGIC: &[u8; 4] = b"GLSV";
// magic | format u16 | schema u32 | key_mode u8 | salt[16] | nonce[24]
const HEADER_LEN: usize = 4 + 2 + 4 + 1 + 16 + 24;
/// Refuse to inflate more than this; a save is a few hundred KB at most.
const MAX_PLAINTEXT: usize = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    BadMagic,
    Truncated,
    UnsupportedFormat(u16),
    /// The save was written by a different `World` schema and no migration exists yet.
    SchemaMismatch {
        found: u32,
        expected: u32,
    },
    /// Wrong key/passphrase, or the file was modified.
    Decrypt,
    Kdf,
    Compress,
    Encode,
    KeyModeMismatch,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SaveError {}

/// 256-bit key. Zeroed on drop.
pub struct Key([u8; 32]);

impl Key {
    pub fn generate() -> Key {
        let mut k = [0u8; 32];
        OsRng.fill_bytes(&mut k);
        Key(k)
    }
    pub fn from_bytes(b: [u8; 32]) -> Key {
        Key(b)
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        for b in self.0.iter_mut() {
            // Volatile so the compiler can't elide the wipe.
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}

impl std::fmt::Debug for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Key(..)")
    }
}

pub enum Unlock<'a> {
    /// Raw key, e.g. fetched from the OS keychain.
    Key(&'a Key),
    /// Passphrase stretched with Argon2id. Used on the web where there is no keychain.
    Passphrase(&'a str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub format: u16,
    pub schema: u32,
    pub passphrase: bool,
}

fn derive(pass: &str, salt: &[u8; 16]) -> Result<Key, SaveError> {
    // OWASP minimum for Argon2id (19 MiB, t=2, p=1): affordable in wasm.
    let params = Params::new(19 * 1024, 2, 1, Some(32)).map_err(|_| SaveError::Kdf)?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(pass.as_bytes(), salt, &mut out)
        .map_err(|_| SaveError::Kdf)?;
    Ok(Key(out))
}

pub fn seal(world: &World, unlock: Unlock<'_>) -> Result<Vec<u8>, SaveError> {
    seal_payload(FORMAT_VERSION, world.schema, world, unlock)
}

/// Seal a whole `Run` (world + unsynced command log + seed) for local storage.
pub fn seal_run(run: &Run, unlock: Unlock<'_>) -> Result<Vec<u8>, SaveError> {
    seal_payload(RUN_FORMAT_VERSION, run.world.schema, run, unlock)
}

fn seal_payload<T: Serialize>(
    format: u16,
    schema: u32,
    value: &T,
    unlock: Unlock<'_>,
) -> Result<Vec<u8>, SaveError> {
    let raw = postcard::to_allocvec(value).map_err(|_| SaveError::Encode)?;
    let packed = lz4_flex::compress_prepend_size(&raw);

    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let (mode, derived);
    let key: &Key = match unlock {
        Unlock::Key(k) => {
            mode = 0u8;
            k
        }
        Unlock::Passphrase(p) => {
            mode = 1;
            OsRng.fill_bytes(&mut salt);
            derived = derive(p, &salt)?;
            &derived
        }
    };

    let mut out = Vec::with_capacity(HEADER_LEN + packed.len() + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&format.to_le_bytes());
    out.extend_from_slice(&schema.to_le_bytes());
    out.push(mode);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce);

    // The whole header is authenticated, so it can't be swapped or downgraded.
    let ct = XChaCha20Poly1305::new(key.as_bytes().into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &packed,
                aad: &out,
            },
        )
        .map_err(|_| SaveError::Decrypt)?;
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Read the unencrypted header without a key.
pub fn peek(bytes: &[u8]) -> Result<Header, SaveError> {
    if bytes.len() < HEADER_LEN {
        return Err(SaveError::Truncated);
    }
    if &bytes[..4] != MAGIC {
        return Err(SaveError::BadMagic);
    }
    Ok(Header {
        format: u16::from_le_bytes([bytes[4], bytes[5]]),
        schema: u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]),
        passphrase: bytes[10] == 1,
    })
}

pub fn open(bytes: &[u8], unlock: Unlock<'_>) -> Result<World, SaveError> {
    open_payload(FORMAT_VERSION, bytes, unlock)
}

pub fn open_run(bytes: &[u8], unlock: Unlock<'_>) -> Result<Run, SaveError> {
    open_payload(RUN_FORMAT_VERSION, bytes, unlock)
}

fn open_payload<T: DeserializeOwned>(
    format: u16,
    bytes: &[u8],
    unlock: Unlock<'_>,
) -> Result<T, SaveError> {
    let h = peek(bytes)?;
    if h.format != format {
        return Err(SaveError::UnsupportedFormat(h.format));
    }
    if h.schema != SCHEMA_VERSION {
        // Future: decode with the frozen struct for `h.schema`, then migrate forward.
        return Err(SaveError::SchemaMismatch {
            found: h.schema,
            expected: SCHEMA_VERSION,
        });
    }
    let (aad, ct) = bytes.split_at(HEADER_LEN);
    let salt: [u8; 16] = aad[11..27].try_into().unwrap();
    let nonce = &aad[27..51];

    let derived;
    let key: &Key = match (unlock, h.passphrase) {
        (Unlock::Key(k), false) => k,
        (Unlock::Passphrase(p), true) => {
            derived = derive(p, &salt)?;
            &derived
        }
        _ => return Err(SaveError::KeyModeMismatch),
    };

    let packed = XChaCha20Poly1305::new(key.as_bytes().into())
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| SaveError::Decrypt)?;
    // lz4 prepends the decompressed size; bound it before allocating.
    if packed.len() < 4
        || u32::from_le_bytes(packed[..4].try_into().unwrap()) as usize > MAX_PLAINTEXT
    {
        return Err(SaveError::Compress);
    }
    let raw = lz4_flex::decompress_size_prepended(&packed).map_err(|_| SaveError::Compress)?;
    postcard::from_bytes(&raw).map_err(|_| SaveError::Encode)
}
