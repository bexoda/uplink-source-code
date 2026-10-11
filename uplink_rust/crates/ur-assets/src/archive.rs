use std::borrow::Cow;
use std::collections::HashMap;
use std::io::{Cursor, Read};

use crate::AssetError;

const V1: &[u8] = b"REDSHIRT\0";
const V2: &[u8] = b"REDSHRT2\0";
/// REDSHRT2 stores a 20-byte SHA-1 after the marker. (No sample of this variant ships on the dev CD,
/// so it is implemented from the original reader but untested.)
const V2_HASH: usize = 20;
/// Refuse to inflate any single entry beyond this; the largest real asset is a few hundred KB.
const MAX_ENTRY: u64 = 64 * 1024 * 1024;

/// Undoes the "REDSHIRT" wrapper: a marker, then every byte of the payload offset by 128.
/// Files without the marker (most `.dat` files are plain ZIPs) pass through untouched.
pub fn decode_wrapper(bytes: &[u8]) -> Cow<'_, [u8]> {
    let skip = if bytes.starts_with(V1) {
        V1.len()
    } else if bytes.starts_with(V2) {
        V2.len() + V2_HASH
    } else {
        return Cow::Borrowed(bytes);
    };
    Cow::Owned(
        bytes
            .get(skip..)
            .unwrap_or_default()
            .iter()
            .map(|b| b.wrapping_sub(128))
            .collect(),
    )
}

/// A `.dat` archive: a ZIP, optionally inside the REDSHIRT wrapper.
pub struct Archive {
    zip: zip::ZipArchive<Cursor<Vec<u8>>>,
    /// lower-case name -> real name, because the game's own paths mix case (`BATTLE3.TTF`).
    index: HashMap<String, String>,
}

impl Archive {
    pub fn from_bytes(bytes: &[u8]) -> Result<Archive, AssetError> {
        let payload = decode_wrapper(bytes).into_owned();
        let zip = zip::ZipArchive::new(Cursor::new(payload))
            .map_err(|e| AssetError::BadArchive(e.to_string()))?;
        let index = zip
            .file_names()
            .filter(|n| !n.ends_with('/'))
            .map(|n| (n.to_ascii_lowercase(), n.to_string()))
            .collect();
        Ok(Archive { zip, index })
    }

    pub fn names(&self) -> Vec<&str> {
        let mut v: Vec<_> = self.index.values().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    pub fn contains(&self, name: &str) -> bool {
        self.index.contains_key(&name.to_ascii_lowercase())
    }

    pub fn read(&mut self, name: &str) -> Result<Vec<u8>, AssetError> {
        let real = self
            .index
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| AssetError::NotFound(name.to_string()))?
            .clone();
        let file = self
            .zip
            .by_name(&real)
            .map_err(|e| AssetError::BadArchive(e.to_string()))?;
        if file.size() > MAX_ENTRY {
            return Err(AssetError::BadArchive(format!(
                "{real} is implausibly large"
            )));
        }
        let mut out = Vec::with_capacity(file.size() as usize);
        file.take(MAX_ENTRY)
            .read_to_end(&mut out)
            .map_err(|e| AssetError::BadArchive(e.to_string()))?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapper_roundtrip_and_passthrough() {
        let plain = b"PK\x03\x04hello";
        assert!(matches!(decode_wrapper(plain), Cow::Borrowed(_)));
        let mut wrapped = V1.to_vec();
        wrapped.extend(plain.iter().map(|b| b.wrapping_add(128)));
        assert_eq!(decode_wrapper(&wrapped).as_ref(), plain);
        let mut v2 = V2.to_vec();
        v2.extend([0u8; V2_HASH]);
        v2.extend(plain.iter().map(|b| b.wrapping_add(128)));
        assert_eq!(decode_wrapper(&v2).as_ref(), plain);
        assert!(
            decode_wrapper(V1).is_empty(),
            "truncated wrapper must not panic"
        );
    }

    #[test]
    fn rejects_non_archives() {
        assert!(Archive::from_bytes(b"not a zip at all").is_err());
        assert!(Archive::from_bytes(b"").is_err());
    }
}
