//! OS keychain-backed key: macOS Keychain, Windows Credential Manager, Linux keyutils.
//! The save key never touches disk; the saves are useless without the login session.

use crate::Key;

const SERVICE: &str = "uplink_rust";
/// The keychain entry from when the project was called Ghostline.
const OLD_SERVICE: &str = "ghostline";

#[derive(Debug)]
pub struct KeychainError(pub String);

/// Fetch the profile's key, creating and storing one on first use.
pub fn get_or_create(profile: &str) -> Result<Key, KeychainError> {
    let e = |e: keyring::Error| KeychainError(e.to_string());
    let entry = keyring::Entry::new(SERVICE, profile).map_err(e)?;
    match entry.get_secret() {
        Ok(bytes) => {
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| KeychainError("stored key has wrong length".into()))?;
            Ok(Key::from_bytes(arr))
        }
        Err(keyring::Error::NoEntry) => {
            // Reuse the key stored under the old name, or older saves could not be opened.
            let old = keyring::Entry::new(OLD_SERVICE, profile)
                .and_then(|o| o.get_secret())
                .ok()
                .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok());
            if let Some(arr) = old {
                entry.set_secret(&arr).map_err(e)?;
                return Ok(Key::from_bytes(arr));
            }
            let k = Key::generate();
            entry.set_secret(k.as_bytes()).map_err(e)?;
            Ok(k)
        }
        Err(err) => Err(e(err)),
    }
}
