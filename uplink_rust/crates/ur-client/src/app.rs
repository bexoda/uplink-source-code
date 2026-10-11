//! The app shell: finds the player's Uplink data, owns the game, saves, and hands every frame to
//! the original interface (`uplink/`). Without the data there is no game screen, only a request
//! for the files: uplink_rust has no look of its own.

use eframe::egui::{self, Align2, Color32, FontId};
use ur_sim::*;

use crate::act::Act;
use crate::audio::Audio;
use crate::game::{Game, Level};
use crate::persist::Persist;
use crate::uplink::UplinkUi;

/// A save is under a kilobyte and seals in well under a millisecond. The browser saves more often
/// because a closed tab gives no chance to save on the way out.
#[cfg(not(target_arch = "wasm32"))]
const AUTOSAVE_SECS: f64 = 10.0;
#[cfg(target_arch = "wasm32")]
const AUTOSAVE_SECS: f64 = 1.0;

fn fresh_seed() -> u64 {
    // Handy for reproducing a world while developing or filing a bug.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(n) = std::env::var("UPLINK_RUST_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
    {
        return n;
    }
    let n = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5eed);
    Rng::new(n).next_u64()
}

/// A new career starts connected to Uplink's own server, as the original does
/// (`WorldGenerator::GeneratePlayer` connects the player to the public access server).
fn new_game() -> Game {
    let mut g = Game::new(fresh_seed());
    if let Some(&services) = g.services().first() {
        g.exec(Command::Connect {
            chain: Vec::new(),
            target: services,
        });
    }
    g.dirty = false;
    g
}

pub struct UplinkRustApp {
    game: Game,
    persist: Option<Persist>,
    /// Problems to show the player in the original message box once the interface is up.
    notices: Vec<String>,
    last_save: f64,
    last_frame: Option<f64>,
    /// The original Uplink interface, built from the player's own Uplink data.
    uplink: Option<UplinkUi>,
    /// Original sounds and music, from the same data.
    audio: Option<Audio>,
    /// Built from files picked in the browser; shown from the next frame, once egui has bound
    /// the fonts it registered (`set_fonts` applies at the start of a frame).
    #[cfg(target_arch = "wasm32")]
    pending: Option<UplinkUi>,
}

impl UplinkRustApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut notice = None;
        let (uplink, audio) = load_uplink_ui(&cc.egui_ctx, &mut notice);
        let mut notices: Vec<String> = notice.map(|(_, t)| t).into_iter().collect();
        let persist = match Persist::open() {
            Ok(p) => Some(p),
            Err(e) => {
                notices.push(e);
                None
            }
        };
        let mut game = None;
        if let Some(p) = &persist {
            match p.load() {
                Ok(Some(run)) => game = Some(Game::from_run(run, false)),
                Ok(None) => {}
                Err(e) => notices.push(format!(
                    "{e}. Starting a new game; the damaged save was left in place."
                )),
            }
        }
        UplinkRustApp {
            game: game.unwrap_or_else(new_game),
            persist,
            notices,
            last_save: 0.0,
            last_frame: None,
            uplink,
            audio,
            #[cfg(target_arch = "wasm32")]
            pending: None,
        }
    }

    fn save(&mut self) {
        if let Some(p) = &self.persist {
            match p.save(&self.game.run) {
                Ok(()) => self.game.dirty = false,
                Err(e) => self.notices.push(format!("Save failed: {e}")),
            }
        }
    }

    fn apply(&mut self, act: Act) {
        match self.game.apply_act(act) {
            Some(Act::SaveNow) => self.save(),
            Some(Act::NewGame) => {
                if let Some(p) = &self.persist {
                    p.clear();
                }
                self.game = new_game();
            }
            _ => {}
        }
    }
}

/// Looks for the player's own Uplink data and builds the original interface and sound from it.
fn load_uplink_ui(
    ctx: &egui::Context,
    notice: &mut Option<(Level, String)>,
) -> (Option<UplinkUi>, Option<Audio>) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(dir) = ur_assets::locate() else {
            return (None, None);
        };
        let built = ur_assets::Store::from_dir(&dir)
            .map_err(|e| e.to_string())
            .and_then(|store| UplinkUi::new(ctx, store));
        match built {
            Ok(ui) => {
                let audio = ur_assets::Store::from_dir(&dir).ok().map(Audio::new);
                (Some(ui), audio)
            }
            Err(e) => {
                *notice = Some((
                    Level::Warn,
                    format!(
                        "Found Uplink data in {} but couldn't use it: {e}",
                        dir.display()
                    ),
                ));
                (None, None)
            }
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        // The page offers a file picker; the files arrive later through `webassets`.
        let _ = notice;
        crate::webassets::set_context(ctx);
        (None, None)
    }
}

/// Builds the interface and sound from files the player picked in the browser.
#[cfg(target_arch = "wasm32")]
fn load_picked(
    ctx: &egui::Context,
    files: std::collections::HashMap<String, Vec<u8>>,
) -> Result<(UplinkUi, Audio), String> {
    let (ui_files, sound_files) = crate::webassets::split(files);
    let ui = UplinkUi::new(ctx, ur_assets::Store::from_memory(ui_files))?;
    Ok((ui, Audio::new(ur_assets::Store::from_memory(sound_files))))
}

/// Shown only while the player's Uplink files are missing. Plain text on black; no game.
fn needs_data(ui: &mut egui::Ui, notices: &[String]) {
    let rect = ui.max_rect();
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::BLACK);
    let mut lines: Vec<String> = vec!["uplink_rust runs on your own copy of Uplink.".into()];
    #[cfg(not(target_arch = "wasm32"))]
    lines.push(
        "Uplink data was not found. Set UPLINK_DIR to the folder with graphics.dat and restart."
            .into(),
    );
    // In the browser the page's own file picker says this; only problems are drawn here.
    #[cfg(target_arch = "wasm32")]
    lines.clear();
    lines.extend(notices.iter().cloned());
    for (i, l) in lines.iter().enumerate() {
        p.text(
            rect.center() + egui::vec2(0.0, -20.0 + 22.0 * i as f32),
            Align2::CENTER_CENTER,
            l,
            FontId::monospace(14.0),
            Color32::from_rgb(150, 170, 220),
        );
    }
}

impl eframe::App for UplinkRustApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        #[cfg(target_arch = "wasm32")]
        if let Some(u) = self.pending.take() {
            self.uplink = Some(u);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(files) = crate::webassets::take_ready() {
            match load_picked(&ctx, files) {
                Ok((u, a)) => {
                    self.pending = Some(u);
                    self.audio = Some(a);
                    ctx.request_repaint();
                }
                Err(e) => self
                    .notices
                    .push(format!("Couldn't use those Uplink files: {e}")),
            }
        }

        let Some(u) = &mut self.uplink else {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| needs_data(ui, &self.notices));
            return;
        };

        // Real elapsed time, not egui's smoothed `stable_dt` (which under-reports on slow frames and
        // made the game clock run slow). `Game::advance` still caps one frame at half a second.
        let now = ctx.input(|i| i.time);
        let dt = self.last_frame.map_or(0.0, |t| (now - t).max(0.0) as f32);
        self.last_frame = Some(now);
        self.game.advance(dt);

        for n in self.notices.drain(..) {
            u.message(&n);
        }
        let mut acts: Vec<Act> = Vec::new();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| u.show(ui, &self.game, dt, &mut acts));
        let ui_cues = u.take_cues();
        for a in acts {
            self.apply(a);
        }
        if self.game.dirty && now - self.last_save > AUTOSAVE_SECS {
            self.last_save = now;
            self.save();
        }
        if let Some(audio) = &mut self.audio {
            for c in ui_cues.into_iter().chain(self.game.take_cues()) {
                audio.play(c);
            }
            audio.update();
            if audio.live() {
                // Keep the playlist moving while the game sits paused.
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
        if !self.game.paused && !self.game.world().game_over {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}
