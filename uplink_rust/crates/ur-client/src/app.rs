//! The app shell: finds the player's Uplink data, owns the game, saves, and hands every frame to
//! the original interface (`uplink/`). Without the data there is no game screen, only a request
//! for the files: uplink_rust has no look of its own.

use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Align2, Color32, FontId};
use ur_sim::replay::Run;
use ur_sim::*;

use crate::account::{Account, Event as AccountEvent};
use crate::act::Act;
use crate::audio::Audio;
use crate::game::{Game, Level};
use crate::persist::Persist;
use crate::uplink::{AccountView, MenuAct, RosterEntry, UplinkUi};

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
/// `seed` comes from the account server for an online agent, so its play can be verified.
fn new_game(seed: Option<u64>) -> Game {
    let mut g = match seed {
        Some(s) => Game::from_run(Run::new(s), true),
        None => Game::new(fresh_seed()),
    };
    if let Some(&services) = g.services().first() {
        g.exec(Command::Connect {
            chain: Vec::new(),
            target: services,
        });
    }
    g
}

pub struct UplinkRustApp {
    /// The agent being played and their game; `None` while in the main menu.
    playing: Option<(String, Game)>,
    persist: Option<Persist>,
    account: Account,
    /// Problems to show the player in the original message box once the interface is up.
    notices: Vec<String>,
    last_save: f64,
    last_frame: Option<f64>,
    now: f64,
    /// The original Uplink interface, built from the player's own Uplink data.
    uplink: Option<UplinkUi>,
    /// Original sounds and music, from the same data.
    audio: Option<Audio>,
    /// New User while signed in: the agent being created on the account.
    creating: Option<String>,
    /// Seeds that arrived before the start sequence finished.
    seeds: HashMap<String, u64>,
    /// Agents whose online creation failed; they start on this device only.
    local_only: HashSet<String>,
    /// The start sequence ended and is waiting for this agent's seed.
    waiting_new: Option<String>,
    /// The loading screen is waiting for this agent's verified state.
    entering: Option<String>,
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
        let account = Account::new(&cc.egui_ctx, crate::account::configured_server());
        UplinkRustApp {
            playing: None,
            persist,
            account,
            notices,
            last_save: 0.0,
            last_frame: None,
            now: 0.0,
            uplink,
            audio,
            creating: None,
            seeds: HashMap::new(),
            local_only: HashSet::new(),
            waiting_new: None,
            entering: None,
            #[cfg(target_arch = "wasm32")]
            pending: None,
        }
    }

    /// Local agents and, when signed in, the account's: one entry per name.
    fn roster(&self) -> Vec<RosterEntry> {
        let local = self
            .persist
            .as_ref()
            .map(|p| p.agents())
            .unwrap_or_default();
        let mut v: Vec<RosterEntry> = local
            .iter()
            .map(|n| RosterEntry {
                name: n.clone(),
                local: true,
                online: false,
            })
            .collect();
        for a in &self.account.agents {
            if a.game_over {
                continue;
            }
            match v.iter_mut().find(|e| e.name == a.name) {
                Some(e) => e.online = true,
                None => v.push(RosterEntry {
                    name: a.name.clone(),
                    local: false,
                    online: true,
                }),
            }
        }
        v.sort_by(|a, b| a.name.cmp(&b.name));
        v
    }

    fn account_view(&self) -> AccountView {
        AccountView {
            status: self.account.status.clone(),
            providers: self.account.providers.clone(),
        }
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

    /// Starts playing `agent`, saving at once so it is on the roster.
    fn play(&mut self, agent: String, game: Game) {
        self.account.reset_stopped(&agent);
        self.playing = Some((agent, game));
        self.save();
    }

    fn start_new(&mut self, agent: String, seed: Option<u64>) {
        self.waiting_new = None;
        self.creating = None;
        let game = new_game(seed);
        self.play(agent, game);
    }

    /// Leaves the game for the login screen, saving first and uploading what's left.
    fn log_off(&mut self) {
        self.save();
        if let Some((agent, game)) = &self.playing {
            self.account.sync(self.now, agent, &game.run, true);
        }
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
                if let Some((agent, _)) = self.playing.take() {
                    if let Some(p) = &self.persist {
                        let _ = p.retire(&agent);
                    }
                    if self.account.agent(&agent).is_some() {
                        self.account.retire_agent(&agent);
                    }
                }
                if let Some(u) = &mut self.uplink {
                    u.show_login(None);
                }
            }
            _ => {}
        }
    }

    fn load_local(&self, agent: &str) -> Result<Option<Run>, String> {
        match &self.persist {
            Some(p) => p.load(agent),
            None => Ok(None),
        }
    }

    fn enter(&mut self, agent: String) {
        let local = self.load_local(&agent);
        let online = self
            .account
            .agent(&agent)
            .and_then(|a| a.seed().map(|s| (s, a.verified_tick.max(0) as u64)));
        match (online, local) {
            // The account holds this agent: use the local save only if it has everything the
            // server verified; otherwise continue from the server.
            (Some((seed, verified)), Ok(Some(run)))
                if run.seed == seed && run.synced_tick() >= verified =>
            {
                self.play(agent, Game::from_run(run, true));
            }
            (Some((seed, _)), _) => {
                self.entering = Some(agent.clone());
                self.account.fetch_state(&agent, seed);
            }
            (None, Ok(Some(run))) => self.play(agent, Game::from_run(run, false)),
            (None, Ok(None)) => self.back_with(&agent, "That agent has no save."),
            (None, Err(e)) => self.back_with(&agent, &format!("Could not load '{agent}': {e}.")),
        }
    }

    fn menu_act(&mut self, act: MenuAct, ctx: &egui::Context) {
        match act {
            MenuAct::Starting(agent) => {
                self.account.create_agent(&agent);
                self.creating = Some(agent);
            }
            MenuAct::NewAgent(agent) => {
                if let Some(seed) = self.seeds.remove(&agent) {
                    self.start_new(agent, Some(seed));
                } else if self.creating.as_deref() == Some(agent.as_str())
                    && !self.local_only.contains(&agent)
                {
                    // The account server hasn't answered yet; the sequence holds until it does.
                    self.waiting_new = Some(agent);
                } else {
                    self.local_only.remove(&agent);
                    self.start_new(agent, None);
                }
            }
            MenuAct::Enter(agent) => self.enter(agent),
            MenuAct::Retire(agent) => {
                if let Some(p) = &self.persist {
                    if let Err(e) = p.retire(&agent) {
                        self.notices
                            .push(format!("Could not retire '{agent}': {e}"));
                    }
                }
                if self.account.agent(&agent).is_some() {
                    self.account.retire_agent(&agent);
                }
            }
            MenuAct::SignIn(provider) => self.account.sign_in(&provider),
            MenuAct::CancelSignIn => self.account.cancel_sign_in(),
            MenuAct::SignOut => self.account.sign_out(),
            MenuAct::SetHandle(h) => self.account.set_handle(&h),
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

    fn account_event(&mut self, e: AccountEvent) {
        match e {
            AccountEvent::Notice(t) => self.notices.push(t),
            AccountEvent::AgentReady { name, seed } => {
                if self.waiting_new.as_deref() == Some(name.as_str()) {
                    self.start_new(name, Some(seed));
                } else {
                    self.seeds.insert(name, seed);
                }
            }
            AccountEvent::AgentLocalOnly { why, .. } => {
                let Some(name) = self.creating.take() else {
                    return;
                };
                self.notices.push(format!(
                    "'{name}' couldn't be created on your Uplink account ({why}). It is saved on this device only."
                ));
                if self.waiting_new.as_deref() == Some(name.as_str()) {
                    self.start_new(name, None);
                } else {
                    self.local_only.insert(name);
                }
            }
            AccountEvent::Acknowledge { agent, seg } => {
                if let Some((a, game)) = &mut self.playing {
                    if *a == agent {
                        game.run.acknowledge(&seg);
                        game.dirty = true;
                    }
                }
                if self.playing.is_none() {
                    // Uploaded on the way out: record it in the save too.
                    if let Ok(Some(mut run)) = self.load_local(&agent) {
                        run.acknowledge(&seg);
                        if let Some(p) = &self.persist {
                            let _ = p.save(&agent, &run);
                        }
                    }
                }
            }
            AccountEvent::Unverified { agent } => {
                if let Some((a, game)) = &mut self.playing {
                    if *a == agent {
                        game.verified = false;
                    }
                }
            }
            AccountEvent::Resume { agent, seed, world } => {
                if self.entering.as_deref() != Some(agent.as_str()) {
                    return;
                }
                self.entering = None;
                let had_local = matches!(self.load_local(&agent), Ok(Some(_)));
                let run = match world {
                    Some(w) => Run::resume(seed, *w),
                    None => Run::new(seed),
                };
                if had_local {
                    self.notices.push(format!(
                        "'{agent}' continues from the latest play saved on your Uplink account."
                    ));
                }
                self.play(agent, Game::from_run(run, true));
            }
            AccountEvent::ResumeFailed { agent, why } => {
                if self.entering.as_deref() != Some(agent.as_str()) {
                    return;
                }
                self.entering = None;
                match self.load_local(&agent) {
                    Ok(Some(run)) => {
                        self.notices.push(format!(
                            "The Uplink server can't be reached ({why}). '{agent}' continues from this device's save."
                        ));
                        self.play(agent, Game::from_run(run, true));
                    }
                    _ => self.back_with(
                        &agent,
                        &format!("Could not fetch '{agent}' from your Uplink account ({why})."),
                    ),
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

        // Real elapsed time, not egui's smoothed `stable_dt` (which under-reports on slow frames and
        // made the game clock run slow). `Game::advance` still caps one frame at half a second.
        let now = ctx.input(|i| i.time);
        let dt = self.last_frame.map_or(0.0, |t| (now - t).max(0.0) as f32);
        self.last_frame = Some(now);
        self.now = now;

        self.account.update(now);
        for e in self.account.take_events() {
            self.account_event(e);
        }

        if self.uplink.is_none() {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| needs_data(ui, &self.notices));
            return;
        }

        let roster = self.roster();
        let view = self.account_view();
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
                    .show(ui, |ui| u.show_menu(ui, &roster, &view, dt, &mut menu_acts));
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
        if let Some((agent, game)) = &self.playing {
            self.account.sync(now, agent, &game.run, false);
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
        } else {
            // Paused: still wake for background uploads and account replies.
            ctx.request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}
