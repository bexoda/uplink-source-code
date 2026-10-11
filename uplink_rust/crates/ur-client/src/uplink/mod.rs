//! The Uplink interface, rebuilt from the original source: a 1024x768 screen of absolutely
//! positioned buttons in the "Uplink Blue" theme, using the game's own sprites and font.
//! Built from the player's own Uplink data; there is no other interface.
//!
//! Drawing helpers take `(x, y, w, h, ...)` like the original's button registration, hence the long signatures.
#![allow(clippy::too_many_arguments)]

mod canvas;
mod hud;
mod local;
mod map;
mod menu;
mod remote;
mod sprites;
mod swmenu;
mod theme;
mod widgets;

pub use menu::{AccountView, MenuAct, RosterEntry};

use std::collections::HashMap;
use std::sync::Arc;

use eframe::egui::{self, Color32, FontData, FontDefinitions, Rect};
use ur_assets::Store;
use ur_sim::{ServerId, Tool};

use self::canvas::{uplink_family, Ecl, Size, H, W};
use self::sprites::Sprites;
use self::theme::Theme;
use crate::act::Act;
use crate::assets::{place_on_land, LandMask};
use crate::audio::Cue;
use crate::game::Game;

/// Which panel the toolbar has open on the right-hand side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Local {
    None,
    Hardware,
    Memory,
    Status,
    Finance,
    Mail,
    Analyser,
    Mission(u32),
}

pub struct Res {
    pub theme: Theme,
    pub sprites: Sprites,
    land: LandMask,
}

pub struct State {
    pub map_large: bool,
    pub local: Local,
    pub sw_open: bool,
    pub sw_cat: Option<u32>,
    pub anim: f32,
    /// Normalised map positions, stable per server name.
    pub pos: HashMap<ServerId, (f32, f32)>,
    pub gw: (f32, f32),
    names: Vec<String>,
    pub remote: remote::Remote,
    /// A program waiting for the player to click its target.
    pub armed: Option<Tool>,
    pub help: String,
    pub msg: Option<(String, String)>,
    /// Messages waiting for the box to close.
    pub queued: Vec<String>,
    /// Whether the Trace Tracker window is open.
    pub tracker: bool,
    /// Sounds to play this frame, in the order their originals would have fired.
    pub cues: Vec<Cue>,
    /// `st.anim` time of the next Trace Tracker bleep.
    pub next_beep: f32,
}

impl State {
    pub fn say(&mut self, title: &str, text: &str) {
        self.msg = Some((title.to_string(), text.to_string()));
    }

    /// Start a program from the software menu. Programs that act on the whole connection run at
    /// once; the rest wait for the player to click what they should work on.
    pub fn run_software(&mut self, tool: Tool, g: &Game, acts: &mut Vec<Act>) {
        let Some(sess) = g.world().session.as_ref() else {
            self.say(
                "Message",
                "You are not connected to anything. Build a connection on the world map first.",
            );
            return;
        };
        // sw_interface: runsoftware.wav when a program starts.
        self.cues.push(Cue::RunSoftware);
        match tool {
            Tool::FirewallBypass | Tool::ProxyBypass => acts.push(Act::Cmd(ur_sim::Command::Run {
                tool,
                server: sess.target,
                file: None,
            })),
            _ => self.armed = Some(tool),
        }
    }
}

pub struct UplinkUi {
    ecl: Ecl,
    res: Res,
    st: State,
    menu: menu::Menu,
}

impl UplinkUi {
    /// Loads fonts, the theme and the map mask from the player's archives. Any failure returns an
    /// error and the caller keeps the plain UI.
    pub fn new(ctx: &egui::Context, mut store: Store) -> Result<UplinkUi, String> {
        let font = store
            .font("dungeon.TTF")
            .map_err(|e| format!("font: {e}"))?;
        let theme_text = store
            .read("patch.dat", "graphics/theme.txt")
            .map_err(|e| format!("theme: {e}"))?;
        let theme = Theme::parse(&String::from_utf8_lossy(&theme_text));
        if theme.len() < 10 {
            return Err("theme file has too few colours".into());
        }
        let mask_img = store
            .image("worldmaplarge_mask.tif")
            .map_err(|e| format!("map mask: {e}"))?;
        let land = LandMask::from_rgba(
            mask_img.width as usize,
            mask_img.height as usize,
            &mask_img.pixels,
        );

        // The game uses only dungeon.ttf. Keep egui's built-in faces behind it for glyphs it lacks (the credit sign).
        let mut fonts = FontDefinitions::default();
        fonts
            .font_data
            .insert("dungeon".into(), Arc::new(FontData::from_owned(font)));
        let mut chain = vec!["dungeon".to_string()];
        chain.extend(
            fonts
                .families
                .get(&egui::FontFamily::Monospace)
                .cloned()
                .unwrap_or_default(),
        );
        fonts.families.insert(uplink_family(), chain);
        ctx.set_fonts(fonts);

        let bg = theme.c("Background").to_array();
        let loading_list = store
            .read("loading.dat", "graphics/loading/filenames.txt")
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned());
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(1);
        let menu = menu::Menu::new(loading_list.as_deref(), seed);
        let sprites = Sprites::new(ctx.clone(), store, [bg[0], bg[1], bg[2]]);
        Ok(UplinkUi {
            menu,
            ecl: Ecl::default(),
            res: Res {
                theme,
                sprites,
                land,
            },
            st: State {
                map_large: false,
                local: Local::None,
                sw_open: false,
                sw_cat: None,
                anim: 0.0,
                pos: HashMap::new(),
                gw: (0.5, 0.5),
                names: Vec::new(),
                remote: remote::Remote::default(),
                armed: None,
                help: String::new(),
                msg: None,
                queued: Vec::new(),
                tracker: true,
                cues: Vec::new(),
                next_beep: 0.0,
            },
        })
    }

    /// Server positions on the map: land only, stable per name. Recomputed only when the set of servers changes.
    fn layout(&mut self, g: &Game) {
        let ids: Vec<ServerId> = g
            .relays()
            .into_iter()
            .chain(g.targets())
            .chain(g.services())
            .collect();
        let mut names = vec!["gateway".to_string()];
        names.extend(ids.iter().map(|id| g.name_of(*id)));
        if names == self.st.names {
            return;
        }
        let pts = place_on_land(&names, &self.res.land, 0.075);
        self.st.gw = pts[0];
        self.st.pos = ids.iter().copied().zip(pts.into_iter().skip(1)).collect();
        self.st.names = names;
    }

    /// Shows `text` in the original message box (queued behind any box already open).
    pub fn message(&mut self, text: &str) {
        if self.st.msg.is_none() {
            self.st.say("Message", text);
        } else {
            self.st.queued.push(text.to_string());
        }
    }

    /// Sounds the interface asked for since the last call.
    pub fn take_cues(&mut self) -> Vec<Cue> {
        std::mem::take(&mut self.st.cues)
    }

    /// The main menu (login, new-user start sequence, loading screen).
    pub fn show_menu(
        &mut self,
        ui: &mut egui::Ui,
        agents: &[RosterEntry],
        account: &AccountView,
        dt: f32,
        out: &mut Vec<MenuAct>,
    ) {
        let area: Rect = ui.max_rect();
        ui.allocate_rect(area, egui::Sense::hover());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::BLACK);
        let (res, st, ecl, m) = (&self.res, &mut self.st, &mut self.ecl, &mut self.menu);
        if st.msg.is_none() && !st.queued.is_empty() {
            let next = st.queued.remove(0);
            st.say("Message", &next);
        }
        let mut c = ecl.begin(ui, painter, area);
        if c.just_pressed() {
            st.cues.push(Cue::MouseClick);
        }
        c.fill(0.0, 0.0, W, H, res.theme.c("Background"));
        menu::draw(&mut c, ui, res, st, m, agents, account, dt, out);
        msgbox(&mut c, res, st);
        if matches!(m.screen, menu::Screen::Login) {
            let tip = c.tooltip_text();
            c.text(
                10.0,
                H - 15.0 + 10.0,
                &tip,
                Size::Small,
                res.theme.c("DefaultText"),
            );
        }
    }

    /// Back to the login screen (log off, game over, or a save that would not load), with the
    /// agent's name filled in. The in-game screens start fresh next time.
    pub fn show_login(&mut self, agent: Option<&str>) {
        self.menu.show_login(agent);
        let st = &mut self.st;
        st.map_large = false;
        st.local = Local::None;
        st.sw_open = false;
        st.sw_cat = None;
        st.armed = None;
        st.remote = remote::Remote::default();
        st.names.clear();
        st.help.clear();
        st.msg = None;
        st.tracker = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, g: &Game, dt: f32, acts: &mut Vec<Act>) {
        self.layout(g);
        self.st.anim += dt;
        let area: Rect = ui.max_rect();
        ui.allocate_rect(area, egui::Sense::hover());
        let painter = ui.painter_at(area);
        painter.rect_filled(area, 0.0, Color32::BLACK);
        let (res, st, ecl) = (&self.res, &mut self.st, &mut self.ecl);
        // Refusals and big moments become message boxes, as in the original.
        if st.msg.is_none() {
            let next = if st.queued.is_empty() {
                None
            } else {
                Some(st.queued.remove(0))
            };
            if let Some(m) = next
                .or_else(|| g.take_refusal())
                .or_else(|| g.take_notice())
            {
                st.say("Message", &m);
            }
        }
        let mut c = ecl.begin(ui, painter, area);
        if c.just_pressed() {
            st.cues.push(Cue::MouseClick);
        }

        c.fill(0.0, 0.0, W, H, res.theme.c("Background"));

        // The player's own screens, then the HUD over them, then the map.
        remote::draw(&mut c, res, st, g, acts);
        local::draw(&mut c, res, st, g, acts);
        hud::draw(&mut c, res, st, g, acts);
        if st.map_large {
            map::large(&mut c, res, st, g, acts);
        } else {
            map::small(&mut c, res, st, g, acts);
        }
        swmenu::draw(&mut c, res, st, g, acts);
        remote::draw_programs(&mut c, res, st, g, acts);
        msgbox(&mut c, res, st);
        game_over_box(&mut c, res, g, acts);

        // The tooltip bar along the bottom edge (text from whatever the mouse was over last frame).
        let tip = c.tooltip_text();
        c.text(
            10.0,
            H - 15.0 + 10.0,
            &tip,
            Size::Small,
            res.theme.c("DefaultText"),
        );
    }
}

/// `create_msgbox`: a small panel in the upper middle with a title bar, text and a Close button.
fn msgbox(c: &mut canvas::Canvas, res: &Res, st: &mut State) {
    let Some((title, text)) = st.msg.clone() else {
        return;
    };
    let (x, y) = (W / 2.0 - 100.0, (H / 3.0).floor());
    // Swallow clicks on everything underneath.
    c.hit("msgbox_background", 0, 0.0, 0.0, W, H, "");
    widgets::panel(c, res, x, y, 200.0, 140.0);
    let t = c.hit(
        "msgbox_title",
        0,
        x + 2.0,
        y + 2.0,
        195.0,
        15.0,
        "Close the message box",
    );
    widgets::button_box(c, res, x + 2.0, y + 2.0, 195.0, 15.0, &title, t);
    // Word-wrapped body at (x+12, y+30).
    let mut line = String::new();
    let mut row = 0.0;
    for word in text.split_whitespace() {
        let trial = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if c.text_width(&trial, Size::Small) > 175.0 && !line.is_empty() {
            c.text(
                x + 12.0,
                y + 30.0 + row * 15.0,
                &line,
                Size::Small,
                res.theme.c("DefaultText"),
            );
            row += 1.0;
            line = word.to_string();
        } else {
            line = trial;
        }
    }
    c.text(
        x + 12.0,
        y + 30.0 + row * 15.0,
        &line,
        Size::Small,
        res.theme.c("DefaultText"),
    );
    let close = c.hit(
        "msgbox_close",
        0,
        W / 2.0 - 50.0,
        y + 110.0,
        100.0,
        15.0,
        "Close the message box",
    );
    widgets::button_box(
        c,
        res,
        W / 2.0 - 50.0,
        y + 110.0,
        100.0,
        15.0,
        "Close",
        close,
    );
    if close.clicked || t.clicked {
        st.msg = None;
    }
}

/// Shown when the third trace ends the run.
fn game_over_box(c: &mut canvas::Canvas, res: &Res, g: &Game, acts: &mut Vec<Act>) {
    if !g.world().game_over {
        return;
    }
    let (x, y) = (W / 2.0 - 150.0, (H / 3.0).floor());
    c.hit("gameover_background", 0, 0.0, 0.0, W, H, "");
    widgets::panel(c, res, x, y, 300.0, 140.0);
    widgets::button_box_state(
        c,
        res,
        x + 2.0,
        y + 2.0,
        295.0,
        15.0,
        "Your career is over",
        false,
        false,
    );
    let w = g.world();
    let lines = [
        "You were traced three times.".to_string(),
        format!("Uplink rating: {}", w.player.rating),
        format!("Credits: {}", w.player.credits),
        format!("You survived {}", g.clock()),
    ];
    for (i, l) in lines.iter().enumerate() {
        c.text(
            x + 12.0,
            y + 35.0 + i as f32 * 15.0,
            l,
            Size::Small,
            res.theme.c("DefaultText"),
        );
    }
    let hit = c.hit(
        "gameover_new",
        0,
        x + 100.0,
        y + 110.0,
        100.0,
        15.0,
        "Start a new career",
    );
    widgets::button_box(c, res, x + 100.0, y + 110.0, 100.0, 15.0, "New career", hit);
    if hit.clicked {
        acts.push(Act::NewGame);
    }
}
