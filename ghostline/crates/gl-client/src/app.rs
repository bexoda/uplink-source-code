use eframe::egui::{
    self, vec2, Align2, Color32, FontId, Pos2, ProgressBar, Rect, RichText, Sense, Stroke,
    StrokeKind,
};
use gl_sim::*;

use crate::act::Act;
use crate::audio::Audio;
use crate::game::{Game, HopState, Level};
use crate::persist::Persist;
use crate::uplink::UplinkUi;

// Palette: near-black green terminal.
const BG: Color32 = Color32::from_rgb(7, 11, 9);
const PANEL: Color32 = Color32::from_rgb(12, 19, 16);
const LINE: Color32 = Color32::from_rgb(32, 52, 43);
const TEXT: Color32 = Color32::from_rgb(190, 228, 205);
const DIM: Color32 = Color32::from_rgb(112, 142, 126);
const GREEN: Color32 = Color32::from_rgb(61, 220, 132);
const AMBER: Color32 = Color32::from_rgb(255, 189, 64);
const RED: Color32 = Color32::from_rgb(255, 84, 96);

/// A save is under a kilobyte and seals in well under a millisecond. The browser saves more often
/// because a closed tab gives no chance to save on the way out.
#[cfg(not(target_arch = "wasm32"))]
const AUTOSAVE_SECS: f64 = 10.0;
#[cfg(target_arch = "wasm32")]
const AUTOSAVE_SECS: f64 = 1.0;

fn level_color(l: Level) -> Color32 {
    match l {
        Level::Info => DIM,
        Level::Good => GREEN,
        Level::Warn => AMBER,
        Level::Bad => RED,
    }
}

fn layer_label(k: LayerKind) -> &'static str {
    match k {
        LayerKind::Password => "Password",
        LayerKind::Firewall => "Firewall",
        LayerKind::Proxy => "Proxy",
        LayerKind::Cipher => "Cipher",
    }
}

fn tool_label(t: Tool) -> &'static str {
    match t {
        Tool::PasswordBreaker => "Password Breaker",
        Tool::FirewallBypass => "Firewall Bypass",
        Tool::ProxyBypass => "Proxy Bypass",
        Tool::Decrypter => "Decrypter",
        Tool::LogDeleter => "Log Deleter",
        Tool::FileCopier => "File Copier",
    }
}

fn tool_blurb(t: Tool) -> &'static str {
    match t {
        Tool::PasswordBreaker => "Cracks password layers",
        Tool::FirewallBypass => "Gets past firewalls",
        Tool::ProxyBypass => "Disables proxies",
        Tool::Decrypter => "Breaks cipher layers",
        Tool::LogDeleter => "Erases your log entry; a trace can't cross a hop with no log",
        Tool::FileCopier => "Copies files to your gateway",
    }
}

const ALL_TOOLS: [Tool; 6] = [
    Tool::PasswordBreaker,
    Tool::FirewallBypass,
    Tool::ProxyBypass,
    Tool::Decrypter,
    Tool::LogDeleter,
    Tool::FileCopier,
];

fn fresh_seed() -> u64 {
    // Handy for reproducing a world while developing or filing a bug.
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(n) = std::env::var("GHOSTLINE_SEED")
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

pub struct GhostlineApp {
    game: Game,
    persist: Option<Persist>,
    notice: Option<(Level, String)>,
    last_save: f64,
    last_frame: Option<f64>,
    confirm_new: bool,
    /// The original Uplink interface, when the player's Uplink data was found.
    uplink: Option<UplinkUi>,
    /// Original sounds and music; only with the original interface (it needs the same data).
    audio: Option<Audio>,
    /// Built from files picked in the browser; shown from the next frame, once egui has bound
    /// the fonts it registered (`set_fonts` applies at the start of a frame).
    #[cfg(target_arch = "wasm32")]
    pending: Option<UplinkUi>,
}

impl GhostlineApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut notice = None;
        let (uplink, audio) = load_uplink_ui(&cc.egui_ctx, &mut notice);
        apply_theme(&cc.egui_ctx);
        let persist = match Persist::open() {
            Ok(p) => Some(p),
            Err(e) => {
                notice = Some((Level::Warn, e));
                None
            }
        };
        let mut game = None;
        if let Some(p) = &persist {
            match p.load() {
                Ok(Some(run)) => {
                    let mut g = Game::from_run(run, false);
                    g.log.push_back(crate::game::LogLine {
                        tick: g.world().tick_no,
                        level: Level::Info,
                        text: "Save loaded.".into(),
                    });
                    game = Some(g);
                }
                Ok(None) => {}
                Err(e) => {
                    notice = Some((
                        Level::Bad,
                        format!("{e}. Starting a new game; the damaged save was left on disk."),
                    ))
                }
            }
        }
        GhostlineApp {
            game: game.unwrap_or_else(|| Game::new(fresh_seed())),
            persist,
            notice,
            last_save: 0.0,
            last_frame: None,
            confirm_new: false,
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
                Err(e) => self.notice = Some((Level::Bad, format!("Save failed: {e}"))),
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
                self.game = Game::new(fresh_seed());
                self.confirm_new = false;
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
        let Some(dir) = gl_assets::locate() else {
            return (None, None);
        };
        let built = gl_assets::Store::from_dir(&dir)
            .map_err(|e| e.to_string())
            .and_then(|store| UplinkUi::new(ctx, store));
        match built {
            Ok(ui) => {
                let audio = gl_assets::Store::from_dir(&dir).ok().map(Audio::new);
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
    let ui = UplinkUi::new(ctx, gl_assets::Store::from_memory(ui_files))?;
    Ok((ui, Audio::new(gl_assets::Store::from_memory(sound_files))))
}

fn apply_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.extreme_bg_color = BG;
    v.faint_bg_color = Color32::from_rgb(16, 26, 21);
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = Color32::from_rgb(24, 90, 58);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = Color32::from_rgb(20, 34, 28);
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(20, 34, 28);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(28, 56, 42);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(36, 80, 56);
    ctx.set_visuals_of(egui::Theme::Dark, v);
    ctx.all_styles_mut(|s| {
        s.override_font_id = Some(FontId::monospace(13.0));
        s.spacing.item_spacing = vec2(8.0, 6.0);
        s.spacing.button_padding = vec2(8.0, 4.0);
    });
}

impl eframe::App for GhostlineApp {
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
                    self.notice = None;
                    ctx.request_repaint();
                }
                Err(e) => {
                    self.notice =
                        Some((Level::Warn, format!("Couldn't use those Uplink files: {e}")))
                }
            }
        }
        // Real elapsed time, not egui's smoothed `stable_dt` (which under-reports on slow frames and
        // made the game clock run slow). `Game::advance` still caps one frame at half a second.
        let now = ctx.input(|i| i.time);
        let dt = self.last_frame.map_or(0.0, |t| (now - t).max(0.0) as f32);
        self.last_frame = Some(now);
        self.game.advance(dt);
        if self.game.dirty && now - self.last_save > AUTOSAVE_SECS {
            self.last_save = now;
            self.save();
        }

        let mut acts: Vec<Act> = Vec::new();

        if let Some(u) = &mut self.uplink {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| u.show(ui, &self.game, dt, &mut acts));
            for a in acts {
                self.apply(a);
            }
            if let Some(audio) = &mut self.audio {
                let ui_cues = self
                    .uplink
                    .as_mut()
                    .map(|u| u.take_cues())
                    .unwrap_or_default();
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
            return;
        }

        let g = &self.game;

        egui::Panel::top("top").show(ui, |ui| {
            top_bar(
                ui,
                g,
                &self.notice,
                self.persist.is_some(),
                &mut self.confirm_new,
                &mut acts,
            )
        });
        egui::Panel::bottom("console")
            .resizable(true)
            .default_size(150.0)
            .show(ui, |ui| console(ui, g));
        egui::Panel::left("left")
            .default_size(310.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| network_list(ui, g, &mut acts));
            });
        egui::Panel::right("right")
            .default_size(340.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    trace_panel(ui, g);
                    ui.add_space(10.0);
                    task_manager(ui, g, &mut acts);
                    ui.add_space(10.0);
                    missions(ui, g, &mut acts);
                    ui.add_space(10.0);
                    shop(ui, g, &mut acts);
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            network_map(ui, g, &mut acts);
            ui.add_space(8.0);
            egui::ScrollArea::vertical().show(ui, |ui| target_panel(ui, g, &mut acts));
        });

        if g.world().game_over {
            egui::Window::new(RichText::new("TRACED").color(RED).strong())
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .show(&ctx, |ui| {
                    ui.label("Three strikes. Your career is over.");
                    ui.label(format!(
                        "Rating {}  ·  survived {}  ·  credits {}",
                        g.world().player.rating,
                        g.clock(),
                        g.world().player.credits
                    ));
                    ui.add_space(8.0);
                    if ui.button("Start a new career").clicked() {
                        acts.push(Act::NewGame);
                    }
                });
        }

        for a in acts {
            self.apply(a);
        }
        if !self.game.paused && !self.game.world().game_over {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save();
    }
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).color(GREEN).strong());
    ui.separator();
}

fn top_bar(
    ui: &mut egui::Ui,
    g: &Game,
    notice: &Option<(Level, String)>,
    can_save: bool,
    confirm_new: &mut bool,
    acts: &mut Vec<Act>,
) {
    let w = g.world();
    ui.horizontal(|ui| {
        ui.label(RichText::new("GHOSTLINE").color(GREEN).strong().size(18.0));
        ui.separator();
        ui.label(RichText::new(format!("₡ {}", w.player.credits)).color(AMBER).strong());
        ui.label(format!("rating {}", w.player.rating));
        let heat: String = (0..gl_sim::tuning::MAX_HEAT).map(|i| if i < w.player.heat { '●' } else { '○' }).collect();
        ui.label(RichText::new(format!("heat {heat}")).color(if w.player.heat > 0 { RED } else { DIM }));
        ui.label(RichText::new(format!("t+{}", g.clock())).color(DIM));
        ui.separator();
        if ui.button(if g.paused { "▶ resume" } else { "⏸ pause" }).clicked() {
            acts.push(Act::TogglePause);
        }
        if can_save && ui.button("save").clicked() {
            acts.push(Act::SaveNow);
        }
        let label = if *confirm_new { "really? click again" } else { "new game" };
        if ui.button(label).clicked() {
            if *confirm_new {
                acts.push(Act::NewGame);
            }
            *confirm_new = !*confirm_new;
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (text, color) = if g.verified { ("VERIFIED", GREEN) } else { ("OFFLINE · UNVERIFIED", DIM) };
            ui.label(RichText::new(text).color(color).small()).on_hover_text("Offline runs are fully playable but cannot count on online leaderboards. Sync arrives with login.");
            if g.run.unsynced() > 0 {
                ui.label(RichText::new(format!("{} unsynced", g.run.unsynced())).color(DIM).small());
                ui.separator();
            }
        });
    });
    if let Some((lvl, msg)) = notice {
        ui.label(RichText::new(msg).color(level_color(*lvl)));
    }
}

fn console(ui: &mut egui::Ui, g: &Game) {
    heading(ui, "CONSOLE");
    egui::ScrollArea::vertical()
        .stick_to_bottom(true)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for l in &g.log {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{:>5}",
                            l.tick / gl_sim::tuning::TICKS_PER_SECOND as u64
                        ))
                        .color(LINE),
                    );
                    ui.label(RichText::new(&l.text).color(level_color(l.level)));
                });
            }
        });
}

fn network_list(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    let locked = g.connected();
    heading(ui, "RELAYS  (click to chain, in order)");
    for id in g.relays() {
        let pos = g.chain.iter().position(|r| *r == id);
        let name = g.name_of(id);
        let ip = g.world().servers[id].ip.clone();
        let text = match pos {
            Some(i) => format!("[{}] {name}  {ip}", i + 1),
            None => format!("[ ] {name}  {ip}"),
        };
        if ui
            .add_enabled(!locked, egui::Button::selectable(pos.is_some(), text))
            .clicked()
        {
            acts.push(Act::ToggleRelay(id));
        }
    }
    ui.add_space(8.0);
    heading(ui, "TARGETS");
    for id in g.targets() {
        let s = &g.world().servers[id];
        let n = s.layers.len();
        let text = format!(
            "{}  {}  ({n} layer{})",
            s.name,
            s.ip,
            if n == 1 { "" } else { "s" }
        );
        if ui
            .add_enabled(
                !locked,
                egui::Button::selectable(g.target == Some(id), text),
            )
            .clicked()
        {
            acts.push(Act::SelectTarget(id));
        }
    }
    ui.add_space(8.0);
    if g.connected() {
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(RichText::new("DISCONNECT").color(AMBER)),
            )
            .clicked()
        {
            acts.push(Act::Disconnect);
        }
    } else {
        let hops = g.chain.len();
        let label = format!(
            "CONNECT  ({hops} relay{})",
            if hops == 1 { "" } else { "s" }
        );
        if ui
            .add_enabled(
                g.target.is_some() && !g.world().game_over,
                egui::Button::new(RichText::new(label).color(GREEN))
                    .min_size(vec2(ui.available_width(), 28.0)),
            )
            .clicked()
        {
            acts.push(Act::Connect);
        }
        if g.chain.is_empty() && g.target.is_some() {
            ui.label(
                RichText::new("No relays: a trace reaches you after one hop.")
                    .color(AMBER)
                    .small(),
            );
        }
    }
}

fn missions(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    heading(ui, "CONTRACTS");
    for m in &g.world().missions {
        ui.horizontal_wrapped(|ui| {
            let (mark, color) = if m.done {
                ("+", GREEN)
            } else if m.accepted {
                ("▸", AMBER)
            } else {
                ("·", DIM)
            };
            ui.label(RichText::new(mark).color(color));
            ui.label(
                RichText::new(format!("{}  (+{})", m.title, m.reward)).color(if m.done {
                    DIM
                } else {
                    TEXT
                }),
            );
            if !m.accepted && !m.done && ui.small_button("accept").clicked() {
                acts.push(Act::Cmd(Command::AcceptMission(m.id)));
            }
        });
    }
}

fn shop(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    heading(ui, "TOOLS");
    let credits = g.world().player.credits;
    for t in ALL_TOOLS {
        let level = g.world().player.tools.get(&t).copied().unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{:<17} L{level}", tool_label(t))).color(if level > 0 {
                    TEXT
                } else {
                    DIM
                }),
            )
            .on_hover_text(tool_blurb(t));
            if level >= gl_sim::tuning::MAX_TOOL_LEVEL {
                ui.label(RichText::new("max").color(DIM));
            } else {
                let price = t.upgrade_price(level);
                let verb = if level == 0 { "buy" } else { "up" };
                if ui
                    .add_enabled(
                        credits >= price,
                        egui::Button::new(format!("{verb} ₡{price}")).small(),
                    )
                    .clicked()
                {
                    acts.push(Act::Cmd(Command::BuyTool(t)));
                }
            }
        });
    }
}

fn trace_panel(ui: &mut egui::Ui, g: &Game) {
    heading(ui, "TRACE");
    let Some(hops) = g.trace_view() else {
        ui.label(RichText::new("No connection.").color(DIM));
        return;
    };
    let tracing = g.world().trace.is_some();
    ui.label(if tracing {
        RichText::new("They are tracing you.").color(RED)
    } else {
        RichText::new("Quiet. Nothing has tripped an alarm.").color(GREEN)
    });
    ui.label(
        RichText::new(
            "The tracer walks back from the target. It can't cross a hop whose log you erased.",
        )
        .color(DIM)
        .small(),
    );
    ui.add_space(4.0);
    ui.label(RichText::new("YOU (gateway)").color(DIM));
    for h in &hops {
        let (color, tag) = match h.state {
            HopState::Traced => (RED, "traced"),
            HopState::Active => (AMBER, "tracing"),
            HopState::Pending => (DIM, "waiting"),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(&h.name).color(color));
            if h.log_erased {
                ui.label(RichText::new("log erased").color(GREEN).small());
            }
        });
        ui.add(ProgressBar::new(h.progress).fill(color).desired_height(8.0));
        ui.label(RichText::new(tag).color(color).small());
    }
}

fn task_manager(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    heading(ui, "TASKS");
    let w = g.world();
    if w.tasks.is_empty() {
        ui.label(RichText::new("Idle.").color(DIM));
        return;
    }
    let n = w.tasks.len();
    if n > 1 {
        ui.label(
            RichText::new(format!(
                "{n} tasks share your CPU: each runs at 1/{n} speed."
            ))
            .color(AMBER)
            .small(),
        );
    }
    for (id, t) in w.tasks.iter() {
        ui.horizontal(|ui| {
            ui.label(format!("{} @ {}", tool_label(t.tool), g.name_of(t.server)));
            if ui.small_button("cancel").clicked() {
                acts.push(Act::Cmd(Command::Cancel(id)));
            }
        });
        ui.add(
            ProgressBar::new(t.progress as f32 / t.required.max(1) as f32)
                .fill(GREEN)
                .desired_height(10.0),
        );
    }
}

/// Graph of the gateway, relays and targets. Draws the planned or live connection and the tracer.
fn network_map(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    let size = vec2(ui.available_width(), 230.0);
    let (resp, painter) = ui.allocate_painter(size, Sense::hover());
    let r = resp.rect;
    painter.rect_filled(r, 4.0, BG);
    painter.rect_stroke(r, 4.0, Stroke::new(1.0, LINE), StrokeKind::Inside);

    let gw = Pos2::new(r.left() + 56.0, r.center().y);
    let spread = |n: usize, x: f32| -> Vec<Pos2> {
        (0..n)
            .map(|i| {
                Pos2::new(
                    x,
                    r.top() + r.height() * (i as f32 + 1.0) / (n as f32 + 1.0),
                )
            })
            .collect()
    };
    let relays = g.relays();
    let targets = g.targets();
    let mut nodes: Vec<(ServerId, Pos2, bool)> = Vec::new();
    for (id, p) in relays
        .iter()
        .zip(spread(relays.len(), r.left() + r.width() * 0.42))
    {
        nodes.push((*id, p, false));
    }
    for (id, p) in targets.iter().zip(spread(targets.len(), r.right() - 150.0)) {
        nodes.push((*id, p, true));
    }
    let pos_of = |id: ServerId| nodes.iter().find(|n| n.0 == id).map(|n| n.1);

    // The connection: live if connected, otherwise what you have planned.
    let w = g.world();
    let (chain, target, live) = match &w.session {
        Some(s) => (s.chain.clone(), Some(s.target), true),
        None => (g.chain.clone(), g.target, false),
    };
    let mut points = vec![gw];
    for id in &chain {
        if let Some(p) = pos_of(*id) {
            points.push(p);
        }
    }
    if let Some(p) = target.and_then(pos_of) {
        points.push(p);
    }
    let edge = if live {
        Stroke::new(2.0, GREEN)
    } else {
        Stroke::new(1.5, Color32::from_rgb(40, 110, 76))
    };
    if points.len() > 1 {
        for pair in points.windows(2) {
            painter.line_segment([pair[0], pair[1]], edge);
        }
    }

    // The tracer: a red pulse crawling back along the live connection toward you.
    if let (Some(hops), true) = (g.trace_view(), live) {
        if let Some(pos) = hops.iter().position(|h| h.state == HopState::Active) {
            if points.len() == hops.len() + 1 {
                let f = hops[pos].progress;
                let (a, b) = (points[pos + 1], points[pos]);
                let p = a + (b - a) * f;
                painter.circle_filled(p, 7.0, RED.gamma_multiply(0.35));
                painter.circle_filled(p, 4.0, RED);
            }
        }
    }

    painter.circle_filled(gw, 14.0, Color32::from_rgb(20, 60, 42));
    painter.circle_stroke(gw, 14.0, Stroke::new(2.0, GREEN));
    painter.text(
        gw + vec2(0.0, 24.0),
        Align2::CENTER_CENTER,
        "YOU",
        FontId::monospace(12.0),
        GREEN,
    );

    for (i, (id, p, is_target)) in nodes.iter().enumerate() {
        let srv = &w.servers[*id];
        let hit = Rect::from_center_size(*p, vec2(34.0, 34.0));
        let resp = ui.interact(hit, ui.id().with(("node", i)), Sense::click());
        let in_chain = chain.iter().position(|c| c == id);
        let is_sel_target = target == Some(*id);
        let hot = in_chain.is_some() || is_sel_target;
        let (fill, ring) = if *is_target {
            let unlocked = live && is_sel_target && !srv.locked();
            (
                Color32::from_rgb(40, 14, 18),
                if unlocked {
                    GREEN
                } else if is_sel_target {
                    AMBER
                } else {
                    RED.gamma_multiply(0.7)
                },
            )
        } else {
            (Color32::from_rgb(14, 30, 24), if hot { GREEN } else { DIM })
        };
        let radius = if *is_target { 11.0 } else { 9.0 };
        let radius = if resp.hovered() { radius + 2.0 } else { radius };
        painter.circle_filled(*p, radius, fill);
        painter.circle_stroke(*p, radius, Stroke::new(if hot { 2.5 } else { 1.5 }, ring));
        if let Some(i) = in_chain {
            painter.text(
                *p,
                Align2::CENTER_CENTER,
                (i + 1).to_string(),
                FontId::monospace(11.0),
                GREEN,
            );
        }
        let (anchor, off) = if *is_target {
            (Align2::LEFT_CENTER, vec2(18.0, 0.0))
        } else {
            (Align2::CENTER_TOP, vec2(0.0, 14.0))
        };
        painter.text(
            *p + off,
            anchor,
            &srv.name,
            FontId::monospace(11.0),
            if hot { TEXT } else { DIM },
        );
        if resp.clicked() && !live {
            acts.push(if *is_target {
                Act::SelectTarget(*id)
            } else {
                Act::ToggleRelay(*id)
            });
        }
        resp.on_hover_text(format!(
            "{}  {}\n{}",
            srv.name,
            srv.ip,
            if *is_target {
                format!(
                    "{} defence layers, {} files",
                    srv.layers.len(),
                    srv.files.len()
                )
            } else {
                "Anonymous relay".into()
            }
        ));
    }
}

fn target_panel(ui: &mut egui::Ui, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();
    let Some(sess) = &w.session else {
        heading(ui, "TARGET");
        match g.target {
            Some(t) => {
                let s = &w.servers[t];
                ui.label(format!("{} ({}) selected.", s.name, s.ip));
                let kinds: Vec<_> = s.layers.iter().map(|l| layer_label(l.kind)).collect();
                ui.label(
                    RichText::new(format!("Defences, outermost first: {}", kinds.join(" → ")))
                        .color(DIM),
                );
                ui.label(
                    RichText::new(
                        "Choose relays, then connect. Your first attack starts the trace clock.",
                    )
                    .color(DIM),
                );
            }
            None => {
                ui.label(RichText::new("Pick a target on the map or in the list. Accept a contract first if you want to be paid.").color(DIM));
            }
        }
        return;
    };
    let t = sess.target;
    let srv = &w.servers[t];
    heading(ui, &format!("{}  {}", srv.name.to_uppercase(), srv.ip));

    ui.label(RichText::new("DEFENCES").color(DIM).small());
    let next = srv.next_layer();
    for (i, l) in srv.layers.iter().enumerate() {
        ui.horizontal(|ui| {
            let (mark, color) = if l.defeated {
                ("+", GREEN)
            } else if Some(i) == next {
                ("▸", AMBER)
            } else {
                ("·", DIM)
            };
            ui.label(RichText::new(mark).color(color));
            ui.label(
                RichText::new(format!(
                    "{:<9} strength {}",
                    layer_label(l.kind),
                    l.strength
                ))
                .color(if l.defeated { DIM } else { TEXT }),
            );
            if Some(i) == next {
                let tool = match l.kind {
                    LayerKind::Password => Tool::PasswordBreaker,
                    LayerKind::Firewall => Tool::FirewallBypass,
                    LayerKind::Proxy => Tool::ProxyBypass,
                    LayerKind::Cipher => Tool::Decrypter,
                };
                if !g.owns(tool) {
                    let price = tool.upgrade_price(0);
                    let can = w.player.credits >= price;
                    if ui
                        .add_enabled(
                            can,
                            egui::Button::new(format!("buy {} ₡{price}", tool_label(tool))),
                        )
                        .clicked()
                    {
                        acts.push(Act::Cmd(Command::BuyTool(tool)));
                    }
                } else if g.task_running(t, TaskKind::Defeat { layer: i }) {
                    ui.label(RichText::new("working…").color(AMBER));
                } else if ui.button(format!("run {}", tool_label(tool))).clicked() {
                    acts.push(Act::Cmd(Command::Run {
                        tool,
                        server: t,
                        file: None,
                    }));
                }
            }
        });
    }
    if srv.locked() {
        ui.label(
            RichText::new("Locked: files and log erasure need every layer down.")
                .color(DIM)
                .small(),
        );
    }

    ui.add_space(8.0);
    ui.label(RichText::new("FILES").color(DIM).small());
    for (i, f) in srv.files.iter().enumerate() {
        ui.horizontal(|ui| {
            let have = w
                .player
                .gateway
                .files
                .iter()
                .any(|(s, gf)| *s == t && gf.name == f.name);
            ui.label(
                RichText::new(format!(
                    "{:<18} size {:<4} value {}",
                    f.name, f.size, f.value
                ))
                .color(if have { DIM } else { TEXT }),
            );
            if have {
                ui.label(RichText::new("copied").color(GREEN));
            } else if g.task_running(t, TaskKind::Copy { file: i }) {
                ui.label(RichText::new("copying…").color(AMBER));
            } else if ui
                .add_enabled(!srv.locked(), egui::Button::new("copy"))
                .clicked()
            {
                acts.push(Act::Cmd(Command::Run {
                    tool: Tool::FileCopier,
                    server: t,
                    file: Some(i),
                }));
            }
        });
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new("YOUR LOGS  (erase before the tracer reaches that hop)")
            .color(DIM)
            .small(),
    );
    for id in sess.chain.iter().copied().chain(std::iter::once(t)) {
        let s = &w.servers[id];
        let Some(entry) = s.logs.iter().find(|l| l.session == sess.id) else {
            continue;
        };
        ui.horizontal(|ui| {
            ui.label(format!("{:<12}", s.name));
            if entry.erased {
                ui.label(RichText::new("erased").color(GREEN));
            } else if g.task_running(id, TaskKind::DeleteLog) {
                ui.label(RichText::new("erasing…").color(AMBER));
            } else {
                let blocked = id == t && srv.locked();
                if ui
                    .add_enabled(
                        !blocked && g.owns(Tool::LogDeleter),
                        egui::Button::new("erase log"),
                    )
                    .clicked()
                {
                    acts.push(Act::Cmd(Command::Run {
                        tool: Tool::LogDeleter,
                        server: id,
                        file: None,
                    }));
                }
                ui.label(RichText::new("present").color(RED).small());
            }
        });
    }
}
