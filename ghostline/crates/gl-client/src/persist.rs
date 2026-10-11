//! Local save storage.
//!
//! Native: sealed file in the user's data directory. The key comes from the OS keychain when the
//! `os-keyring` feature is on and works, otherwise from a key file next to the save.
//! Either way this only makes casual edits of the file detectable. It is NOT anti-cheat: anyone
//! who can run the game can read the key. Cheating is stopped by server-side replay.
//!
//! Web: not implemented yet; saves there will go through the sync server.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::path::{Path, PathBuf};

    use gl_save::store::LocalStore;
    use gl_save::{open_run, seal_run, Key, Unlock};
    use gl_sim::replay::Run;

    const SLOT: &str = "autosave";

    pub struct Persist {
        store: LocalStore,
        key: Key,
        pub dir: PathBuf,
        pub key_source: &'static str,
    }

    pub fn data_dir() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let base = if cfg!(target_os = "windows") {
            std::env::var_os("APPDATA").map(PathBuf::from)?
        } else if cfg!(target_os = "macos") {
            home?.join("Library/Application Support")
        } else {
            std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .or_else(|| home.map(|h| h.join(".local/share")))?
        };
        Some(base.join("ghostline"))
    }

    fn key_from_file(dir: &Path) -> Result<Key, String> {
        let path = dir.join("local.key");
        match fs::read(&path) {
            Ok(b) => <[u8; 32]>::try_from(b.as_slice())
                .map(Key::from_bytes)
                .map_err(|_| "local.key is corrupt".to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let k = Key::generate();
                fs::write(&path, k.as_bytes()).map_err(|e| e.to_string())?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
                }
                Ok(k)
            }
            Err(e) => Err(e.to_string()),
        }
    }

    impl Persist {
        pub fn open() -> Result<Persist, String> {
            let dir = data_dir().ok_or("no data directory (HOME is not set)")?;
            fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            #[cfg(feature = "os-keyring")]
            if let Ok(key) = gl_save::keychain::get_or_create("default") {
                return Persist::with_key(&dir, key, "OS keychain");
            }
            let key = key_from_file(&dir)?;
            Persist::with_key(&dir, key, "key file")
        }

        pub fn with_key(dir: &Path, key: Key, key_source: &'static str) -> Result<Persist, String> {
            let store = LocalStore::new(dir).map_err(|e| e.to_string())?;
            Ok(Persist {
                store,
                key,
                dir: dir.to_path_buf(),
                key_source,
            })
        }

        /// `Ok(None)` means no save exists yet. A damaged save falls back to the previous one.
        pub fn load(&self) -> Result<Option<Run>, String> {
            let read = |bytes: Vec<u8>| {
                open_run(&bytes, Unlock::Key(&self.key)).map_err(|e| e.to_string())
            };
            match self.store.read(SLOT) {
                Ok(b) => match read(b) {
                    Ok(run) => Ok(Some(run)),
                    Err(e) => match self
                        .store
                        .read_backup(SLOT)
                        .map_err(|e| e.to_string())
                        .and_then(read)
                    {
                        Ok(run) => Ok(Some(run)),
                        Err(_) => Err(format!("save is unreadable or was modified ({e})")),
                    },
                },
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        }

        pub fn save(&self, run: &Run) -> Result<(), String> {
            let bytes = seal_run(run, Unlock::Key(&self.key)).map_err(|e| e.to_string())?;
            self.store.write(SLOT, &bytes).map_err(|e| e.to_string())
        }

        pub fn clear(&self) {
            let _ = fs::remove_file(self.dir.join(format!("{SLOT}.glsave")));
            let _ = fs::remove_file(self.dir.join(format!("{SLOT}.glsave.bak")));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn save_load_roundtrip_and_tamper_falls_back_to_backup() {
            let dir = tempdir();
            let p = Persist::with_key(&dir, Key::generate(), "test").unwrap();
            assert!(p.load().unwrap().is_none());

            let mut run = Run::new(9);
            p.save(&run).unwrap();
            run.apply(gl_sim::Command::AcceptMission(1), &mut vec![])
                .unwrap();
            p.save(&run).unwrap();
            let back = p.load().unwrap().unwrap();
            assert_eq!((back.seed, back.unsynced()), (9, 1));

            // Flip a byte in the live save: the previous save is used instead of losing everything.
            let path = dir.join("autosave.glsave");
            let mut b = fs::read(&path).unwrap();
            let n = b.len() / 2;
            b[n] ^= 0xFF;
            fs::write(&path, b).unwrap();
            let back = p.load().unwrap().unwrap();
            assert_eq!(back.unsynced(), 0, "fell back to the older, intact save");

            // Both damaged: an error the UI can show, not a silent new game.
            let bak = dir.join("autosave.glsave.bak");
            fs::write(&bak, b"garbage").unwrap();
            assert!(p.load().is_err());
            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn key_file_is_stable_across_opens() {
            let dir = tempdir();
            let a = key_from_file(&dir).unwrap();
            let b = key_from_file(&dir).unwrap();
            assert_eq!(a.as_bytes(), b.as_bytes());
            let _ = fs::remove_dir_all(&dir);
        }

        fn tempdir() -> PathBuf {
            let d = std::env::temp_dir().join(format!(
                "gl-client-test-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&d);
            fs::create_dir_all(&d).unwrap();
            d
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{data_dir, Persist};

#[cfg(target_arch = "wasm32")]
pub struct Persist;

#[cfg(target_arch = "wasm32")]
impl Persist {
    pub fn open() -> Result<Persist, String> {
        Err("Saving in the browser isn't built yet: progress is lost on reload.".into())
    }
    pub fn load(&self) -> Result<Option<gl_sim::replay::Run>, String> {
        Ok(None)
    }
    pub fn save(&self, _: &gl_sim::replay::Run) -> Result<(), String> {
        Ok(())
    }
    pub fn clear(&self) {}
}
