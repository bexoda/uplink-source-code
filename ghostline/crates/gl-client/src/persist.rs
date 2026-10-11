//! Local save storage.
//!
//! Native: sealed file in the user's data directory. The key comes from the OS keychain when the
//! `os-keyring` feature is on and works, otherwise from a key file next to the save.
//! Either way this only makes casual edits of the file detectable. It is NOT anti-cheat: anyone
//! who can run the game can read the key. Cheating is stopped by server-side replay.
//!
//! Web: the same sealed container in localStorage (`browser`), with the same backup fallback.

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

/// Browser saves: the same sealed container, kept in a key-value store (localStorage on the web).
/// The key sits in the same store, so as on desktop this only detects casual edits; the server's
/// replay check is what stops cheating.
pub mod browser {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    use gl_save::{open_run, seal_run, Key, Unlock};
    use gl_sim::replay::Run;

    pub const KEY: &str = "ghostline.key";
    pub const SLOT: &str = "ghostline.autosave";
    pub const BACKUP: &str = "ghostline.autosave.bak";

    /// String key-value storage, like `window.localStorage`.
    pub trait Kv {
        fn get(&self, k: &str) -> Result<Option<String>, String>;
        fn set(&self, k: &str, v: &str) -> Result<(), String>;
        fn remove(&self, k: &str);
    }

    pub struct Persist<S: Kv> {
        kv: S,
        key: Key,
    }

    impl<S: Kv> Persist<S> {
        pub fn open(kv: S) -> Result<Self, String> {
            let key = match kv.get(KEY)? {
                Some(t) => {
                    let b = B64
                        .decode(t.trim())
                        .map_err(|_| "save key is corrupt".to_string())?;
                    Key::from_bytes(
                        <[u8; 32]>::try_from(b.as_slice())
                            .map_err(|_| "save key is corrupt".to_string())?,
                    )
                }
                None => {
                    // Never replace a key that sealed an existing save: it would orphan the save.
                    if kv.get(SLOT)?.is_some() {
                        return Err("the save key is missing, so the save can't be opened".into());
                    }
                    let k = Key::generate();
                    kv.set(KEY, &B64.encode(k.as_bytes()))?;
                    k
                }
            };
            Ok(Persist { kv, key })
        }

        fn read(&self, slot: &str) -> Result<Option<Run>, String> {
            let Some(t) = self.kv.get(slot)? else {
                return Ok(None);
            };
            let bytes = B64.decode(t.trim()).map_err(|e| e.to_string())?;
            open_run(&bytes, Unlock::Key(&self.key))
                .map(Some)
                .map_err(|e| e.to_string())
        }

        /// `Ok(None)` means no save yet. A damaged save falls back to the previous one.
        pub fn load(&self) -> Result<Option<Run>, String> {
            match self.read(SLOT) {
                Ok(r) => Ok(r),
                Err(e) => match self.read(BACKUP) {
                    Ok(Some(run)) => Ok(Some(run)),
                    _ => Err(format!("save is unreadable or was modified ({e})")),
                },
            }
        }

        pub fn save(&self, run: &Run) -> Result<(), String> {
            let sealed = seal_run(run, Unlock::Key(&self.key)).map_err(|e| e.to_string())?;
            let text = B64.encode(sealed);
            if let Some(prev) = self.kv.get(SLOT)? {
                self.kv.set(BACKUP, &prev)?;
            }
            self.kv.set(SLOT, &text)
        }

        pub fn clear(&self) {
            self.kv.remove(SLOT);
            self.kv.remove(BACKUP);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::cell::RefCell;
        use std::collections::HashMap;

        #[derive(Default)]
        struct Mem(RefCell<HashMap<String, String>>, RefCell<Option<usize>>);
        impl Kv for &Mem {
            fn get(&self, k: &str) -> Result<Option<String>, String> {
                Ok(self.0.borrow().get(k).cloned())
            }
            fn set(&self, k: &str, v: &str) -> Result<(), String> {
                if self.1.borrow().is_some_and(|max| v.len() > max) {
                    return Err("QuotaExceededError".into());
                }
                self.0.borrow_mut().insert(k.into(), v.into());
                Ok(())
            }
            fn remove(&self, k: &str) {
                self.0.borrow_mut().remove(k);
            }
        }

        #[test]
        fn roundtrip_backup_and_tamper() {
            let mem = Mem::default();
            let p = Persist::open(&mem).unwrap();
            assert!(p.load().unwrap().is_none());
            let mut run = Run::new(5);
            p.save(&run).unwrap();
            run.apply(gl_sim::Command::AcceptMission(1), &mut vec![])
                .unwrap();
            p.save(&run).unwrap();

            // A new session with the same storage reads it back.
            let p2 = Persist::open(&mem).unwrap();
            let back = p2.load().unwrap().unwrap();
            assert_eq!((back.seed, back.unsynced()), (5, 1));

            // Edit the live save: the older backup is used.
            let mut t = mem.0.borrow()[SLOT].clone().into_bytes();
            let n = t.len() / 2;
            t[n] = if t[n] == b'A' { b'B' } else { b'A' };
            mem.0
                .borrow_mut()
                .insert(SLOT.into(), String::from_utf8(t).unwrap());
            assert_eq!(p2.load().unwrap().unwrap().unsynced(), 0);

            // Both broken: an error, never a silent new game.
            mem.0.borrow_mut().insert(BACKUP.into(), "garbage".into());
            assert!(p2.load().is_err());

            p2.clear();
            assert!(p2.load().unwrap().is_none());
        }

        #[test]
        fn a_lost_key_is_not_silently_replaced() {
            let mem = Mem::default();
            Persist::open(&mem).unwrap().save(&Run::new(1)).unwrap();
            mem.0.borrow_mut().remove(KEY);
            assert!(Persist::open(&mem).is_err());
            assert!(mem.0.borrow().contains_key(SLOT), "the save is kept");
        }

        #[test]
        fn quota_errors_surface() {
            let mem = Mem::default();
            let p = Persist::open(&mem).unwrap();
            *mem.1.borrow_mut() = Some(10);
            assert!(p.save(&Run::new(1)).is_err());
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub struct LocalStorage(web_sys::Storage);

#[cfg(target_arch = "wasm32")]
impl browser::Kv for LocalStorage {
    fn get(&self, k: &str) -> Result<Option<String>, String> {
        self.0.get_item(k).map_err(|e| format!("{e:?}"))
    }
    fn set(&self, k: &str, v: &str) -> Result<(), String> {
        self.0
            .set_item(k, v)
            .map_err(|_| "the browser refused to store the save (storage full or blocked)".into())
    }
    fn remove(&self, k: &str) {
        let _ = self.0.remove_item(k);
    }
}

#[cfg(target_arch = "wasm32")]
pub struct Persist(browser::Persist<LocalStorage>);

#[cfg(target_arch = "wasm32")]
impl Persist {
    pub fn open() -> Result<Persist, String> {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .ok_or("this browser blocks local storage, so progress is lost on reload")?;
        browser::Persist::open(LocalStorage(storage)).map(Persist)
    }
    pub fn load(&self) -> Result<Option<gl_sim::replay::Run>, String> {
        self.0.load()
    }
    pub fn save(&self, run: &gl_sim::replay::Run) -> Result<(), String> {
        self.0.save(run)
    }
    pub fn clear(&self) {
        self.0.clear()
    }
}
