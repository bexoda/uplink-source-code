use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};

use crate::{decode_tiff, decode_wrapper, Archive, AssetError, Rgba};

/// Where an Uplink install might be, most specific first. `UPLINK_DIR` always wins.
#[cfg(not(target_arch = "wasm32"))]
pub fn candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = std::env::var_os("UPLINK_DIR") {
        v.push(PathBuf::from(d));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(h) = &home {
        for steam in [
            ".steam/steam",
            ".local/share/Steam",
            "Library/Application Support/Steam",
        ] {
            v.push(h.join(steam).join("steamapps/common/Uplink"));
        }
        v.push(h.join("Uplink"));
        v.push(h.join("uplink"));
    }
    for p in [
        "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Uplink",
        "C:\\Program Files\\Steam\\steamapps\\common\\Uplink",
        "C:\\Program Files (x86)\\GOG Galaxy\\Games\\Uplink",
        "C:\\GOG Games\\Uplink",
        "/Applications/Uplink.app/Contents/Resources",
        "/opt/uplink",
        "/usr/local/games/uplink",
        "/usr/share/games/uplink",
    ] {
        v.push(PathBuf::from(p));
    }
    v
}

/// First candidate that actually contains the game's graphics.
#[cfg(not(target_arch = "wasm32"))]
pub fn locate() -> Option<PathBuf> {
    candidates()
        .into_iter()
        .find(|d| d.join("graphics.dat").is_file())
}

/// Order the game itself applies: later patches override earlier files.
const IMAGE_SOURCES: [&str; 5] = [
    "patch3.dat",
    "patch2.dat",
    "patch.dat",
    "graphics.dat",
    "loading.dat",
];

enum Source {
    #[cfg(not(target_arch = "wasm32"))]
    Dir(PathBuf),
    /// Files the user handed us (e.g. from a browser file picker). Never uploaded anywhere.
    Memory(HashMap<String, Vec<u8>>),
}

/// Lazily opens `.dat` archives and decodes things out of them.
pub struct Store {
    source: Source,
    archives: HashMap<String, Option<Archive>>,
}

impl Store {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn from_dir(dir: impl AsRef<Path>) -> Result<Store, AssetError> {
        let dir = dir.as_ref().to_path_buf();
        if !dir.join("graphics.dat").is_file() {
            return Err(AssetError::NotFound(format!(
                "graphics.dat in {}",
                dir.display()
            )));
        }
        Ok(Store {
            source: Source::Dir(dir),
            archives: HashMap::new(),
        })
    }

    /// `files` maps a lower-case file name such as `"graphics.dat"` to its bytes.
    pub fn from_memory(files: HashMap<String, Vec<u8>>) -> Store {
        Store {
            source: Source::Memory(files),
            archives: HashMap::new(),
        }
    }

    fn raw(&self, dat: &str) -> Result<Vec<u8>, AssetError> {
        match &self.source {
            #[cfg(not(target_arch = "wasm32"))]
            Source::Dir(d) => std::fs::read(d.join(dat)).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AssetError::NotFound(dat.to_string())
                } else {
                    AssetError::Io(e.to_string())
                }
            }),
            Source::Memory(m) => m
                .get(&dat.to_ascii_lowercase())
                .cloned()
                .ok_or_else(|| AssetError::NotFound(dat.to_string())),
        }
    }

    /// The archive `dat`, loaded on first use. A missing optional patch file is remembered, not re-read.
    pub fn archive(&mut self, dat: &str) -> Result<&mut Archive, AssetError> {
        if !self.archives.contains_key(dat) {
            let loaded = match self.raw(dat) {
                Ok(bytes) => Some(Archive::from_bytes(&bytes)?),
                Err(AssetError::NotFound(_)) => None,
                Err(e) => return Err(e),
            };
            self.archives.insert(dat.to_string(), loaded);
        }
        self.archives
            .get_mut(dat)
            .and_then(Option::as_mut)
            .ok_or_else(|| AssetError::NotFound(dat.to_string()))
    }

    pub fn read(&mut self, dat: &str, name: &str) -> Result<Vec<u8>, AssetError> {
        self.archive(dat)?.read(name)
    }

    /// A sprite such as `"hud/status.tif"` (path relative to `graphics/`), honouring patch overrides.
    pub fn image(&mut self, name: &str) -> Result<Rgba, AssetError> {
        let full = format!("graphics/{name}");
        for dat in IMAGE_SOURCES {
            match self.archive(dat) {
                Ok(a) if a.contains(&full) => return decode_tiff(&a.read(&full)?),
                Ok(_) | Err(AssetError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        Err(AssetError::NotFound(full))
    }

    pub fn font(&mut self, name: &str) -> Result<Vec<u8>, AssetError> {
        self.read("fonts.dat", &format!("fonts/{name}"))
    }

    /// A file that is wrapped but isn't an archive (e.g. `world.dat`, an MP3).
    pub fn unwrapped(&mut self, dat: &str) -> Result<Vec<u8>, AssetError> {
        Ok(decode_wrapper(&self.raw(dat)?).into_owned())
    }
}
