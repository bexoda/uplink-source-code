//! Desktop save store: one sealed file per slot, atomic writes, one backup.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::slot;
use crate::sync::SyncState;

pub struct LocalStore {
    dir: PathBuf,
}

fn bad_slot() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid slot name")
}

impl LocalStore {
    pub fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        fs::create_dir_all(dir.as_ref())?;
        Ok(Self {
            dir: dir.as_ref().to_path_buf(),
        })
    }

    fn path(&self, slot_name: &str, ext: &str) -> io::Result<PathBuf> {
        if !slot::is_valid(slot_name) {
            return Err(bad_slot());
        }
        Ok(self.dir.join(format!("{slot_name}.{ext}")))
    }

    /// Write via temp file + fsync + rename so a crash never leaves a torn save.
    /// The previous save is kept as `<slot>.glsave.bak`.
    pub fn write(&self, slot_name: &str, sealed: &[u8]) -> io::Result<()> {
        let path = self.path(slot_name, "glsave")?;
        let tmp = self.path(slot_name, "glsave.tmp")?;
        {
            let mut f = fs::File::create(&tmp)?;
            f.write_all(sealed)?;
            f.sync_all()?;
        }
        if path.exists() {
            fs::copy(&path, self.path(slot_name, "glsave.bak")?)?;
        }
        fs::rename(&tmp, &path)?;
        if let Ok(d) = fs::File::open(&self.dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    pub fn read(&self, slot_name: &str) -> io::Result<Vec<u8>> {
        fs::read(self.path(slot_name, "glsave")?)
    }

    pub fn read_backup(&self, slot_name: &str) -> io::Result<Vec<u8>> {
        fs::read(self.path(slot_name, "glsave.bak")?)
    }

    pub fn slots(&self) -> io::Result<Vec<String>> {
        let mut v: Vec<String> = fs::read_dir(&self.dir)?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let n = e.file_name().into_string().ok()?;
                let s = n.strip_suffix(".glsave")?;
                slot::is_valid(s).then(|| s.to_string())
            })
            .collect();
        v.sort();
        Ok(v)
    }

    /// Sync bookkeeping is not secret (two integers) so it is plain text.
    pub fn read_sync_state(&self, slot_name: &str) -> io::Result<Option<SyncState>> {
        match fs::read_to_string(self.path(slot_name, "sync")?) {
            Ok(s) => {
                let mut it = s.split_whitespace();
                let base = it.next().and_then(|x| x.parse().ok());
                let dirty = it.next().map(|x| x == "1");
                Ok(base
                    .zip(dirty)
                    .map(|(base, dirty)| SyncState { base, dirty }))
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn write_sync_state(&self, slot_name: &str, st: SyncState) -> io::Result<()> {
        fs::write(
            self.path(slot_name, "sync")?,
            format!("{} {}", st.base, st.dirty as u8),
        )
    }
}
