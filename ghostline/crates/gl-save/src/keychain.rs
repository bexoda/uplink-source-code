//! OS keychain-backed key: macOS Keychain, Windows Credential Manager, Linux keyutils.
//! The save key never touches disk; the saves are useless without the login session.

use crate::Key;

const SERVICE: &str = "ghostline";

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
            let k = Key::generate();
            entry.set_secret(k.as_bytes()).map_err(e)?;
            Ok(k)
        }
        Err(err) => Err(e(err)),
    }
}
