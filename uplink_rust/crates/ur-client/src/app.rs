//! The app shell: finds the player's Uplink data, owns the game, saves, and hands every frame to
//! the original interface (`uplink/`). Without the data there is no game screen, only a request
//! for the files: uplink_rust has no look of its own.

use eframe::egui::{self, Align2, Color32, FontId};
use ur_sim::*;

use crate::act::Act;
use crate::audio::Audio;
use crate::game::{Game, Level};
use crate::persist::Persist;
use crate::uplink::{MenuAct, UplinkUi};

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
    /// The agent being played and their game; `None` while in the main menu.
    playing: Option<(String, Game)>,
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
        let persist = match crate::persist::open() {
            Ok(p) => Some(p),
            Err(e) => {
                notices.push(e);
                None
            }
        };
        UplinkRustApp {
            playing: None,
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

    fn agents(&self) -> Vec<String> {
        self.persist
            .as_ref()
            .map(|p| p.agents())
            .unwrap_or_default()
    }

    fn save(&mut self) {
        let (Some(p), Some((agent, game))) = (&self.persist, &mut self.playing) else {
            return;
        };
        match p.save(agent, &game.run) {
            Ok(()) => game.dirty = false,
            Err(e) => self.notices.push(format!("Save failed: {e}")),
        }
    }

    /// Leaves the game for the login screen, saving first.
    fn log_off(&mut self) {
        self.save();
        let agent = self.playing.take().map(|(a, _)| a);
        if let Some(u) = &mut self.uplink {
            u.show_login(agent.as_deref());
        }
    }

    fn apply(&mut self, act: Act) {
        let Some((_, game)) = &mut self.playing else {
            return;
        };
        match game.apply_act(act) {
            Some(Act::SaveNow) => self.save(),
            Some(Act::LogOff) => self.log_off(),
            Some(Act::NewGame) => {
                // Three strikes: the agent's career is over, so they leave the roster.
                if let (Some(p), Some((agent, _))) = (&self.persist, &self.playing) {
                    let _ = p.retire(agent);
                }
                self.playing = None;
                if let Some(u) = &mut self.uplink {
                    u.show_login(None);
                }
            }
            _ => {}
        }
    }

    fn menu_act(&mut self, act: MenuAct, ctx: &egui::Context) {
        match act {
            MenuAct::NewAgent(agent) => {
                let game = new_game();
                self.playing = Some((agent, game));
                self.save();
            }
            MenuAct::Enter(agent) => {
                let loaded = match &self.persist {
                    Some(p) => p.load(&agent),
                    None => Err("saving is unavailable in this browser".into()),
                };
                match loaded {
                    Ok(Some(run)) => self.playing = Some((agent, Game::from_run(run, false))),
                    Ok(None) => self.back_with(&agent, "That agent has no save."),
                    Err(e) => self.back_with(&agent, &format!("Could not load '{agent}': {e}.")),
                }
            }
            MenuAct::Retire(agent) => {
                if let Some(p) = &self.persist {
                    if let Err(e) = p.retire(&agent) {
                        self.notices
                            .push(format!("Could not retire '{agent}': {e}"));
                    }
                }
            }
            MenuAct::Exit => {
                #[cfg(not(target_arch = "wasm32"))]
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = ctx;
                    self.notices
                        .push("Close the browser tab to exit. Your agents are saved.".into());
                }
            }
        }
    }

    fn back_with(&mut self, agent: &str, msg: &str) {
        if let Some(u) = &mut self.uplink {
            u.show_login(Some(agent));
        }
        self.notices.push(msg.to_string());
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

        if self.uplink.is_none() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| needs_data(ui, &self.notices));
            return;
        }

        // Real elapsed time, not egui's smoothed `stable_dt` (which under-reports on slow frames and
        // made the game clock run slow). `Game::advance` still caps one frame at half a second.
        let now = ctx.input(|i| i.time);
        let dt = self.last_frame.map_or(0.0, |t| (now - t).max(0.0) as f32);
        self.last_frame = Some(now);

        let agents = self.agents();
        let notices: Vec<String> = self.notices.drain(..).collect();
        let u = self.uplink.as_mut().expect("checked above");
        for n in &notices {
            u.message(n);
        }

        let mut acts: Vec<Act> = Vec::new();
        let mut menu_acts: Vec<MenuAct> = Vec::new();
        match &mut self.playing {
            Some((_, game)) => {
                game.advance(dt);
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| u.show(ui, game, dt, &mut acts));
            }
            None => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| u.show_menu(ui, &agents, dt, &mut menu_acts));
            }
        }
        let ui_cues = u.take_cues();
        for a in acts {
            self.apply(a);
        }
        for a in menu_acts {
            self.menu_act(a, &ctx);
        }
        if self.playing.as_ref().is_some_and(|(_, g)| g.dirty)
            && now - self.last_save > AUTOSAVE_SECS
        {
            self.last_save = now;
            self.save();
        }
        let game_cues = self
            .playing
            .as_ref()
            .map(|(_, g)| g.take_cues())
            .unwrap_or_default();
        if let Some(audio) = &mut self.audio {
            for c in ui_cues.into_iter().chain(game_cues) {
                audio.play(c);
            }
            audio.update();
            if audio.live() {
                // Keep the playlist moving while the game sits paused.
                ctx.request_repaint_after(std::time::Duration::from_millis(500));
            }
        }
        let running = match &self.playing {
            Some((_, g)) => !g.paused && !g.world().game_over,
            None => true, // the menu animates (cursor, start sequence, loading)
        };
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}
