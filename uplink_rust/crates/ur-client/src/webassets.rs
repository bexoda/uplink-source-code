//! Browser side of "use your own Uplink files": the page hands us the `.dat` files the player
//! picked (or that it cached for them in IndexedDB). They never leave the browser.

use std::cell::RefCell;
use std::collections::HashMap;

use eframe::egui;
use wasm_bindgen::prelude::*;

/// Archives the client reads. Anything else the player picks is ignored.
pub const WANTED: [&str; 7] = [
    "graphics.dat",
    "fonts.dat",
    "patch.dat",
    "patch2.dat",
    "patch3.dat",
    "sounds.dat",
    "music.dat",
];
const REQUIRED: [&str; 3] = ["graphics.dat", "fonts.dat", "patch.dat"];

thread_local! {
    static STAGING: RefCell<HashMap<String, Vec<u8>>> = RefCell::new(HashMap::new());
    static READY: RefCell<Option<HashMap<String, Vec<u8>>>> = const { RefCell::new(None) };
    static CTX: RefCell<Option<egui::Context>> = const { RefCell::new(None) };
}

pub fn set_context(ctx: &egui::Context) {
    CTX.with(|c| *c.borrow_mut() = Some(ctx.clone()));
}

/// Files handed over since the last call, once the page said it was done.
pub fn take_ready() -> Option<HashMap<String, Vec<u8>>> {
    READY.with(|r| r.borrow_mut().take())
}

/// Adds one picked file. Returns whether the client wants it (the page caches only those).
#[wasm_bindgen]
pub fn add_asset(name: &str, bytes: &[u8]) -> bool {
    let name = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(name)
        .to_ascii_lowercase();
    if !WANTED.contains(&name.as_str()) {
        return false;
    }
    STAGING.with(|s| s.borrow_mut().insert(name, bytes.to_vec()));
    true
}

/// Finishes a batch. Returns an empty string when the set is usable, or what is missing.
#[wasm_bindgen]
pub fn assets_done() -> String {
    let files = STAGING.with(|s| std::mem::take(&mut *s.borrow_mut()));
    let missing: Vec<&str> = REQUIRED
        .iter()
        .copied()
        .filter(|f| !files.contains_key(*f))
        .collect();
    if !missing.is_empty() {
        // Keep what we have so a second pick can add the rest.
        STAGING.with(|s| *s.borrow_mut() = files);
        return format!("Still needed: {}", missing.join(", "));
    }
    READY.with(|r| *r.borrow_mut() = Some(files));
    CTX.with(|c| {
        if let Some(ctx) = c.borrow().as_ref() {
            ctx.request_repaint();
        }
    });
    String::new()
}

/// Splits the picked files between the interface and the sound, so neither holds a copy of
/// archives it never reads.
pub fn split(
    mut files: HashMap<String, Vec<u8>>,
) -> (HashMap<String, Vec<u8>>, HashMap<String, Vec<u8>>) {
    let mut sound = HashMap::new();
    for k in ["sounds.dat", "music.dat"] {
        if let Some(v) = files.remove(k) {
            sound.insert(k.to_string(), v);
        }
    }
    (files, sound)
}
