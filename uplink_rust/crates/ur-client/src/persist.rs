//! Local saves: one sealed save per agent, like the original's one `.usr` file per agent.
//!
//! Desktop: files in the user's data directory. The key comes from the OS keychain when the
//! `os-keyring` feature is on and works, otherwise from a key file next to the saves.
//! Browser: the same sealed container in localStorage.
//! Either way the key sits with the saves, so this only makes casual edits detectable. It is NOT
//! anti-cheat: cheating is stopped by the server replaying the run.

use ur_save::{open_run, seal_run, Key, Unlock};
use ur_sim::replay::Run;

/// The name a single pre-roster save is listed under after the upgrade.
pub const MIGRATED_AGENT: &str = "agent";

/// Turns what the player typed into an agent (save slot) name: lower case, letters, digits,
/// `_` and `-`, at most 32 characters. `None` when nothing usable is left.
pub fn agent_name(typed: &str) -> Option<String> {
    let s: String = typed
        .trim()
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| if c == ' ' { '_' } else { c })
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-')
        .take(32)
        .collect();
    (!s.is_empty()).then_some(s)
}

/// Raw storage for sealed saves, by agent name.
pub trait Backend {
    fn list(&self) -> Result<Vec<String>, String>;
    fn read(&self, agent: &str) -> Result<Option<Vec<u8>>, String>;
    fn read_backup(&self, agent: &str) -> Result<Option<Vec<u8>>, String>;
    /// Writes a save, keeping the previous one as the backup.
    fn write(&self, agent: &str, sealed: &[u8]) -> Result<(), String>;
    fn remove(&self, agent: &str) -> Result<(), String>;
}

pub struct Saves<B: Backend> {
    backend: B,
    key: Key,
}

impl<B: Backend> Saves<B> {
    pub fn new(backend: B, key: Key) -> Self {
        Saves { backend, key }
    }

    /// The roster, sorted.
    pub fn agents(&self) -> Vec<String> {
        let mut v = self.backend.list().unwrap_or_default();
        v.sort();
        v
    }

    fn open(&self, bytes: Option<Vec<u8>>) -> Result<Option<Run>, String> {
        bytes
            .map(|b| open_run(&b, Unlock::Key(&self.key)).map_err(|e| e.to_string()))
            .transpose()
    }

    /// `Ok(None)`: no such agent. A damaged save falls back to the previous one.
    pub fn load(&self, agent: &str) -> Result<Option<Run>, String> {
        match self.backend.read(agent).and_then(|b| self.open(b)) {
            Ok(r) => Ok(r),
            Err(e) => match self.backend.read_backup(agent).and_then(|b| self.open(b)) {
                Ok(Some(run)) => Ok(Some(run)),
                _ => Err(format!("the save is unreadable or was modified ({e})")),
            },
        }
    }

    pub fn save(&self, agent: &str, run: &Run) -> Result<(), String> {
        let sealed = seal_run(run, Unlock::Key(&self.key)).map_err(|e| e.to_string())?;
        self.backend.write(agent, &sealed)
    }

    /// Removes the agent from the roster ("Retire" on the login screen).
    pub fn retire(&self, agent: &str) -> Result<(), String> {
        self.backend.remove(agent)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::path::{Path, PathBuf};

    use ur_save::store::LocalStore;
    use ur_save::Key;

    use super::{Backend, Saves};

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
        let dir = base.join("uplink_rust");
        // The project was called Ghostline before; carry its saves over once.
        let old = base.join("ghostline");
        if !dir.exists() && old.is_dir() {
            let _ = fs::rename(&old, &dir);
        }
        Some(dir)
    }

    pub(super) fn key_from_file(dir: &Path) -> Result<Key, String> {
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

    pub struct Files(pub LocalStore);

    fn opt(r: std::io::Result<Vec<u8>>) -> Result<Option<Vec<u8>>, String> {
        match r {
            Ok(b) => Ok(Some(b)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    impl Backend for Files {
        fn list(&self) -> Result<Vec<String>, String> {
            self.0.slots().map_err(|e| e.to_string())
        }
        fn read(&self, agent: &str) -> Result<Option<Vec<u8>>, String> {
            opt(self.0.read(agent))
        }
        fn read_backup(&self, agent: &str) -> Result<Option<Vec<u8>>, String> {
            opt(self.0.read_backup(agent))
        }
        fn write(&self, agent: &str, sealed: &[u8]) -> Result<(), String> {
            self.0.write(agent, sealed).map_err(|e| e.to_string())
        }
        fn remove(&self, agent: &str) -> Result<(), String> {
            self.0.remove(agent).map_err(|e| e.to_string())
        }
    }

    pub type Persist = Saves<Files>;

    /// Opens the saves in `dir` (the data directory by default).
    pub fn open_in(dir: &Path) -> Result<Persist, String> {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let store = LocalStore::new(dir).map_err(|e| e.to_string())?;
        // Before the roster there was one save called "autosave".
        let _ = store.rename("autosave", super::MIGRATED_AGENT);
        #[cfg(feature = "os-keyring")]
        if let Ok(key) = ur_save::keychain::get_or_create("default") {
            return Ok(Saves::new(Files(store), key));
        }
        let key = key_from_file(dir)?;
        Ok(Saves::new(Files(store), key))
    }

    #[cfg(test)]
    pub(super) fn key_from_file_for_test(dir: &Path) -> Key {
        fs::create_dir_all(dir).unwrap();
        key_from_file(dir).unwrap()
    }

    pub fn open() -> Result<Persist, String> {
        let dir = data_dir().ok_or("no data directory (HOME is not set)")?;
        open_in(&dir)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{data_dir, open, Persist};

/// Browser saves over a string key-value store (localStorage on the web), base64.
pub mod browser {
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;
    use ur_save::Key;

    use super::{Backend, Saves};

    pub const KEY: &str = "uplink_rust.key";
    const PREFIX: &str = "uplink_rust.agent.";
    const BAK: &str = ".bak";
    /// Single-save keys from before the roster, and from when the project was called Ghostline.
    const OLD: [(&str, &str); 5] = [
        ("ghostline.key", KEY),
        ("ghostline.autosave", "uplink_rust.agent.agent"),
        ("ghostline.autosave.bak", "uplink_rust.agent.agent.bak"),
        ("uplink_rust.autosave", "uplink_rust.agent.agent"),
        ("uplink_rust.autosave.bak", "uplink_rust.agent.agent.bak"),
    ];

    /// String key-value storage, like `window.localStorage`.
    pub trait Kv {
        fn get(&self, k: &str) -> Result<Option<String>, String>;
        fn set(&self, k: &str, v: &str) -> Result<(), String>;
        fn remove(&self, k: &str);
        fn keys(&self) -> Vec<String>;
    }

    pub struct KvBackend<S: Kv>(pub S);

    fn decode(t: Option<String>) -> Result<Option<Vec<u8>>, String> {
        t.map(|t| B64.decode(t.trim()).map_err(|e| e.to_string()))
            .transpose()
    }

    impl<S: Kv> Backend for KvBackend<S> {
        fn list(&self) -> Result<Vec<String>, String> {
            Ok(self
                .0
                .keys()
                .into_iter()
                .filter_map(|k| k.strip_prefix(PREFIX).map(str::to_string))
                .filter(|a| !a.ends_with(BAK))
                .collect())
        }
        fn read(&self, agent: &str) -> Result<Option<Vec<u8>>, String> {
            decode(self.0.get(&format!("{PREFIX}{agent}"))?)
        }
        fn read_backup(&self, agent: &str) -> Result<Option<Vec<u8>>, String> {
            decode(self.0.get(&format!("{PREFIX}{agent}{BAK}"))?)
        }
        fn write(&self, agent: &str, sealed: &[u8]) -> Result<(), String> {
            let k = format!("{PREFIX}{agent}");
            if let Some(prev) = self.0.get(&k)? {
                self.0.set(&format!("{k}{BAK}"), &prev)?;
            }
            self.0.set(&k, &B64.encode(sealed))
        }
        fn remove(&self, agent: &str) -> Result<(), String> {
            self.0.remove(&format!("{PREFIX}{agent}"));
            self.0.remove(&format!("{PREFIX}{agent}{BAK}"));
            Ok(())
        }
    }

    pub fn open<S: Kv>(kv: S) -> Result<Saves<KvBackend<S>>, String> {
        for (old, new) in OLD.iter() {
            if kv.get(new)?.is_none() {
                if let Some(v) = kv.get(old)? {
                    kv.set(new, &v)?;
                    kv.remove(old);
                }
            }
        }
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
                // Never replace a key that sealed existing saves: they could not be opened.
                if kv.keys().iter().any(|k| k.starts_with(PREFIX)) {
                    return Err("the save key is missing, so the saves can't be opened".into());
                }
                let k = Key::generate();
                kv.set(KEY, &B64.encode(k.as_bytes()))?;
                k
            }
        };
        Ok(Saves::new(KvBackend(kv), key))
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
    fn keys(&self) -> Vec<String> {
        let n = self.0.length().unwrap_or(0);
        (0..n)
            .filter_map(|i| self.0.key(i).ok().flatten())
            .collect()
    }
}

#[cfg(target_arch = "wasm32")]
pub type Persist = Saves<browser::KvBackend<LocalStorage>>;

#[cfg(target_arch = "wasm32")]
pub fn open() -> Result<Persist, String> {
    let storage = web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .ok_or("this browser blocks local storage, so progress is lost on reload")?;
    browser::open(LocalStorage(storage))
}

#[cfg(test)]
mod tests {
    use super::browser::{self, Kv, KEY};
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
        fn keys(&self) -> Vec<String> {
            self.0.borrow().keys().cloned().collect()
        }
    }

    #[test]
    fn agent_names_are_sanitised() {
        assert_eq!(
            agent_name(" Neo Anderson! ").as_deref(),
            Some("neo_anderson")
        );
        assert_eq!(agent_name("***"), None);
        assert_eq!(agent_name(&"x".repeat(50)).unwrap().len(), 32);
        assert!(ur_save::slot::is_valid(&agent_name("Ab-c_9").unwrap()));
    }

    #[test]
    fn roster_save_load_retire_and_tamper() {
        let mem = Mem::default();
        let p = browser::open(&mem).unwrap();
        assert!(p.agents().is_empty());
        assert!(p.load("neo").unwrap().is_none());

        let mut run = Run::new(5);
        p.save("neo", &run).unwrap();
        p.save("trinity", &Run::new(6)).unwrap();
        run.apply(ur_sim::Command::AcceptMission(1), &mut vec![])
            .unwrap();
        p.save("neo", &run).unwrap();
        assert_eq!(p.agents(), vec!["neo", "trinity"]);

        let p2 = browser::open(&mem).unwrap();
        let back = p2.load("neo").unwrap().unwrap();
        assert_eq!((back.seed, back.unsynced()), (5, 1));

        // Edit the live save: the backup is used.
        let k = "uplink_rust.agent.neo".to_string();
        let mut t = mem.0.borrow()[&k].clone().into_bytes();
        let n = t.len() / 2;
        t[n] = if t[n] == b'A' { b'B' } else { b'A' };
        mem.0
            .borrow_mut()
            .insert(k.clone(), String::from_utf8(t).unwrap());
        assert_eq!(p2.load("neo").unwrap().unwrap().unsynced(), 0);
        mem.0
            .borrow_mut()
            .insert(format!("{k}.bak"), "garbage".into());
        assert!(
            p2.load("neo").is_err(),
            "both broken: an error, not a new game"
        );

        p2.retire("neo").unwrap();
        assert_eq!(p2.agents(), vec!["trinity"]);
    }

    #[test]
    fn old_single_saves_become_an_agent() {
        let mem = Mem::default();
        let first = browser::open(&mem).unwrap();
        first.save("x", &Run::new(7)).unwrap();
        // Recreate the pre-roster layout under the old project name.
        let save = mem.0.borrow_mut().remove("uplink_rust.agent.x").unwrap();
        let key = mem.0.borrow_mut().remove(KEY).unwrap();
        mem.0.borrow_mut().insert("ghostline.autosave".into(), save);
        mem.0.borrow_mut().insert("ghostline.key".into(), key);

        let p = browser::open(&mem).unwrap();
        assert_eq!(p.agents(), vec![MIGRATED_AGENT]);
        assert_eq!(p.load(MIGRATED_AGENT).unwrap().unwrap().seed, 7);
        assert!(!mem.0.borrow().keys().any(|k| k.starts_with("ghostline")));
    }

    #[test]
    fn a_lost_key_is_not_silently_replaced() {
        let mem = Mem::default();
        browser::open(&mem)
            .unwrap()
            .save("a", &Run::new(1))
            .unwrap();
        mem.0.borrow_mut().remove(KEY);
        assert!(browser::open(&mem).is_err());
        assert!(mem.0.borrow().contains_key("uplink_rust.agent.a"));
    }

    #[test]
    fn quota_errors_surface() {
        let mem = Mem::default();
        let p = browser::open(&mem).unwrap();
        *mem.1.borrow_mut() = Some(10);
        assert!(p.save("a", &Run::new(1)).is_err());
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn desktop_roster_and_autosave_migration() {
        use ur_save::store::LocalStore;
        let dir = std::env::temp_dir().join(format!("ur-persist-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // A pre-roster save.
        {
            let key = native::key_from_file_for_test(&dir);
            let sealed = seal_run(&Run::new(3), Unlock::Key(&key)).unwrap();
            LocalStore::new(&dir)
                .unwrap()
                .write("autosave", &sealed)
                .unwrap();
        }
        let p = native::open_in(&dir).unwrap();
        assert_eq!(p.agents(), vec![MIGRATED_AGENT]);
        assert_eq!(p.load(MIGRATED_AGENT).unwrap().unwrap().seed, 3);
        p.save("neo", &Run::new(4)).unwrap();
        p.save("neo", &Run::new(4)).unwrap();
        assert_eq!(p.agents(), vec![MIGRATED_AGENT, "neo"]);
        p.retire("neo").unwrap();
        assert_eq!(p.agents(), vec![MIGRATED_AGENT]);
        assert!(!dir.join("neo.glsave.bak").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
