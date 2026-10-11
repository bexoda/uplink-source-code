//! The main menu screens from `mainmenu/`: the login screen (`login_interface.cpp`), the new-user
//! start sequence (`firsttimeloading_interface.cpp` with `ScriptLibrary::Script30/31`) and the
//! loading screen (`loading_interface.cpp`). Coordinates are the original's; `SX`/`SY` scale the
//! 640x480 design to the 1024x768 screen.

use eframe::egui::{self, Color32, Key};

use super::canvas::{Canvas, Size, H, W};
use super::widgets::{image_button, picture, picture_scaled};
use super::{Res, State};
use crate::audio::Cue;

fn sx(x: f32) -> f32 {
    (x * W / 640.0).floor()
}
fn sy(y: f32) -> f32 {
    (y * H / 480.0).floor()
}

/// What the player asked the menu for. The app owns the saves and the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuAct {
    /// The start sequence finished: create this agent and start playing.
    NewAgent(String),
    /// The loading screen finished: load this agent's save.
    Enter(String),
    Retire(String),
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Login,
    /// The new-user start sequence, started at `t0` (menu time).
    FirstLoad {
        agent: String,
        t0: f32,
    },
    Loading {
        agent: String,
        t0: f32,
        image: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Name,
    Code,
}

pub struct Menu {
    pub screen: Screen,
    name: String,
    code: String,
    focus: Field,
    /// Agent waiting for the "Retire" yes/no box.
    confirm_retire: Option<String>,
    t: f32,
    /// Images listed in `loading/filenames.txt`, minus the one the original skips.
    loading_images: Vec<String>,
    pick: u32,
    /// Start-sequence time at the previous frame, so one-shot sounds fire once.
    last_t: f32,
}

/// How long the loading screen shows before the game (the original shows it while loading).
const LOADING_SECS: f32 = 1.5;
/// Script30 runs 2 s after New User and types its caption over 1.5 s; Script31 adds the local
/// machine, then 0.8 s later the public access server. The phone dialler that follows in the
/// original is not built, so the game starts after a short pause instead.
const CAPTION_AT: f32 = 2.0;
const CAPTION_SECS: f32 = 1.5;
const LOCAL_AT: f32 = CAPTION_AT + CAPTION_SECS;
const SERVER_AT: f32 = LOCAL_AT + 0.8;
const START_AT: f32 = SERVER_AT + 2.0;
const CAPTION: &str = "Establishing connection to UPLINK public access server...";

impl Menu {
    pub fn new(loading_list: Option<&str>, seed: u32) -> Menu {
        let loading_images = loading_list
            .unwrap_or("")
            .split_whitespace()
            .filter(|f| *f != "loading/kris.tif")
            .map(str::to_string)
            .collect();
        Menu {
            screen: Screen::Login,
            name: String::new(),
            code: String::new(),
            focus: Field::Name,
            confirm_retire: None,
            t: 0.0,
            loading_images,
            pick: seed,
            last_t: -1.0,
        }
    }

    pub fn show_login(&mut self, agent: Option<&str>) {
        self.screen = Screen::Login;
        if let Some(a) = agent {
            self.name = a.to_string();
            self.code = "PASSWORD".into();
        }
    }

    fn start_loading(&mut self, agent: String) {
        self.pick = self.pick.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let image = if self.loading_images.is_empty() {
            String::new()
        } else {
            self.loading_images[(self.pick >> 8) as usize % self.loading_images.len()].clone()
        };
        self.screen = Screen::Loading {
            agent,
            t0: self.t,
            image,
        };
    }
}

pub fn draw(
    c: &mut Canvas,
    ui: &egui::Ui,
    res: &Res,
    st: &mut State,
    m: &mut Menu,
    agents: &[String],
    dt: f32,
    out: &mut Vec<MenuAct>,
) {
    m.t += dt;
    match m.screen.clone() {
        Screen::Login => login(c, ui, res, st, m, agents, out),
        Screen::FirstLoad { agent, t0 } => first_load(c, res, st, m, agent, m.t - t0, out),
        Screen::Loading { agent, t0, image } => {
            loading(c, res, &image);
            if m.t - t0 >= LOADING_SECS {
                out.push(MenuAct::Enter(agent));
            }
        }
    }
}

/// `LargeTextBoxDraw`: a caption in the large font, TitleText colour.
fn large_text(c: &mut Canvas, res: &Res, x: f32, y: f32, h: f32, s: &str) {
    c.text(
        x,
        y + h / 2.0 + 5.0,
        s,
        Size::Large,
        res.theme.c("TitleText"),
    );
}

/// `textbutton_draw` / `text_draw`: lines at (x+10, y+10), 15 apart.
fn text_lines(c: &mut Canvas, res: &Res, x: f32, y: f32, lines: &[&str]) {
    for (i, l) in lines.iter().enumerate() {
        c.text(
            x + 10.0,
            y + 10.0 + 15.0 * i as f32,
            l,
            Size::Small,
            res.theme.c("DefaultText"),
        );
    }
}

fn login(
    c: &mut Canvas,
    ui: &egui::Ui,
    res: &Res,
    st: &mut State,
    m: &mut Menu,
    agents: &[String],
    out: &mut Vec<MenuAct>,
) {
    // MainMenuScreen: the logo.
    picture_scaled(
        c,
        res,
        sx(320.0) - 170.0,
        75.0,
        425.0,
        60.0,
        "mainmenu/uplinklogo.tif",
    );

    // Typing goes to the focused box; Return is the Proceed button (ReturnKeyPressed).
    let mut proceed = false;
    if st.msg.is_none() && m.confirm_retire.is_none() {
        ui.input(|i| {
            for e in &i.events {
                match e {
                    egui::Event::Text(t) => {
                        let f = if m.focus == Field::Name {
                            &mut m.name
                        } else {
                            &mut m.code
                        };
                        for ch in t.chars().filter(|c| !c.is_control()) {
                            if f.chars().count() < 32 {
                                f.push(ch);
                            }
                        }
                    }
                    egui::Event::Key {
                        key: Key::Backspace,
                        pressed: true,
                        ..
                    } => {
                        if m.focus == Field::Name {
                            m.name.pop();
                        } else {
                            m.code.pop();
                        }
                    }
                    egui::Event::Key {
                        key: Key::Tab,
                        pressed: true,
                        ..
                    } => {
                        m.focus = if m.focus == Field::Name {
                            Field::Code
                        } else {
                            Field::Name
                        };
                    }
                    egui::Event::Key {
                        key: Key::Enter,
                        pressed: true,
                        ..
                    } => proceed = true,
                    _ => {}
                }
            }
        });
    }

    // The four menu buttons down the right, each an icon with a large caption to its left.
    let (bx, by) = (sx(600.0), sy(270.0));
    let buttons = [
        (
            0.0,
            95.0,
            100.0,
            "New User",
            "mainmenu/newgame.tif",
            "Create a new user's account",
        ),
        (
            40.0,
            68.0,
            50.0,
            "Retire",
            "mainmenu/loadgame.tif",
            "Remove an agent from the roster",
        ),
        (
            80.0,
            81.0,
            80.0,
            "Options",
            "mainmenu/options.tif",
            "Advanced options",
        ),
        (
            120.0,
            45.0,
            50.0,
            "Exit",
            "mainmenu/exitgame.tif",
            "Exit Uplink",
        ),
    ];
    for (i, (dy, back, tw, caption, icon, tip)) in buttons.into_iter().enumerate() {
        let icon_hit = c.hit("mainmenu_button", i as u32, bx, by + dy, 32.0, 32.0, tip);
        image_button(c, res, bx, by + dy, icon, icon_hit, false);
        let text_hit = c.hit(
            "mainmenu_text",
            i as u32,
            bx - back,
            by + dy + 5.0,
            tw,
            20.0,
            tip,
        );
        large_text(c, res, bx - back, by + dy + 5.0, 20.0, caption);
        if icon_hit.clicked || text_hit.clicked {
            match i {
                0 => match crate::persist::agent_name(&m.name) {
                    None => st.say(
                        "New User",
                        "Type the user name you want in the User ID box, then click New User.",
                    ),
                    Some(a) if agents.contains(&a) => st.say(
                        "New User",
                        &format!("An agent called '{a}' already exists. Choose another name."),
                    ),
                    Some(a) => {
                        m.last_t = -1.0;
                        m.screen = Screen::FirstLoad { agent: a, t0: m.t };
                    }
                },
                1 => match crate::persist::agent_name(&m.name) {
                    Some(a) if agents.contains(&a) => m.confirm_retire = Some(a),
                    _ => st.say(
                        "Retire",
                        &format!("Impossible to remove '{}' from the roster.", m.name),
                    ),
                },
                2 => st.say("Options", "The options screens are not built yet."),
                _ => out.push(MenuAct::Exit),
            }
        }
    }

    // The User ID box.
    let (ux, uy) = (sx(320.0) - 110.0, sy(265.0));
    picture(c, res, ux, uy, "userid.tif");
    for (field, fy, tip) in [
        (Field::Name, 55.0, "Enter your userID here"),
        (Field::Code, 81.0, "Enter your access code here"),
    ] {
        let (fx, fy) = (ux + 59.0, uy + fy);
        let hit = c.hit("userid_field", field as u32, fx, fy, 147.0, 15.0, tip);
        if hit.clicked {
            m.focus = field;
        }
        c.fill(fx, fy, 147.0, 15.0, res.theme.c("PasswordBoxBackground"));
        let shown = match field {
            Field::Name => m.name.clone(),
            Field::Code => "*".repeat(m.code.chars().count()),
        };
        // An editable box shows a cursor while it has the focus, blinking each second.
        let cursor = if m.focus == field && (m.t as u32).is_multiple_of(2) {
            "_"
        } else {
            ""
        };
        c.clipped(fx, fy, 147.0, 15.0, |c| {
            c.text(
                fx + 10.0,
                fy + 10.0,
                &format!("{shown}{cursor}"),
                Size::Small,
                res.theme.c("DefaultText"),
            );
        });
        if hit.hover || m.focus == field {
            c.border(fx, fy, 147.0, 15.0, res.theme.c("PanelBorder"));
        }
    }
    let p = c.hit(
        "userid_proceed",
        0,
        ux + 140.0,
        uy + 125.0,
        80.0,
        15.0,
        "Click here when done",
    );
    image_button(c, res, ux + 140.0, uy + 125.0, "proceed.tif", p, false);
    if p.clicked || proceed {
        match crate::persist::agent_name(&m.name) {
            Some(a) if agents.contains(&a) => {
                st.cues.push(Cue::Login);
                m.start_loading(a);
            }
            _ => st.say(
                "Log in",
                "No agent by that name. Pick one from the list, or create a New User.",
            ),
        }
    }

    // Help text above the box.
    text_lines(
        c,
        res,
        ux,
        sy(200.0),
        &[
            "If you are registered as an Uplink Agent,",
            "enter your username and password below.",
            "Otherwise, click on the New User button.",
        ],
    );

    // The roster.
    large_text(c, res, sx(35.0), sy(270.0), sy(20.0), "Valid User Names");
    for (i, a) in agents.iter().enumerate() {
        let y = sy(300.0) + i as f32 * 18.0;
        let hit = c.hit(
            "username",
            i as u32,
            sx(35.0),
            y,
            sx(170.0),
            15.0,
            "Log in as this Agent",
        );
        text_lines(c, res, sx(35.0), y, &[a.as_str()]);
        if hit.hover {
            c.border(sx(35.0), y, sx(170.0), 15.0, res.theme.c("PanelBorder"));
        }
        if hit.clicked {
            m.name = a.clone();
            m.code = "PASSWORD".into();
        }
    }

    // Version, bottom right.
    let version = concat!("uplink_rust ", env!("CARGO_PKG_VERSION"));
    let vw = c.text_width(version, Size::Small);
    c.text(
        W - vw - 8.0,
        H - 5.0,
        version,
        Size::Small,
        res.theme.c("DefaultText"),
    );

    if let Some(a) = m.confirm_retire.clone() {
        match yes_no(
            c,
            res,
            "Retire",
            &format!("Are you sure you want to remove '{a}' from the roster?"),
        ) {
            Some(true) => {
                out.push(MenuAct::Retire(a));
                m.confirm_retire = None;
                m.name.clear();
                m.code.clear();
            }
            Some(false) => m.confirm_retire = None,
            None => {}
        }
    }
}

/// `create_yesnomsgbox`: the message box with Yes and No.
fn yes_no(c: &mut Canvas, res: &Res, title: &str, text: &str) -> Option<bool> {
    let (x, y) = (W / 2.0 - 100.0, (H / 3.0).floor());
    c.hit("yesno_background", 0, 0.0, 0.0, W, H, "");
    super::widgets::panel(c, res, x, y, 200.0, 140.0);
    let t = c.hit("yesno_title", 0, x + 2.0, y + 2.0, 195.0, 15.0, "");
    super::widgets::button_box(c, res, x + 2.0, y + 2.0, 195.0, 15.0, title, t);
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
    let yes = c.hit("yesno_yes", 0, x + 20.0, y + 110.0, 70.0, 15.0, "");
    super::widgets::button_box(c, res, x + 20.0, y + 110.0, 70.0, 15.0, "Yes", yes);
    let no = c.hit("yesno_no", 0, x + 110.0, y + 110.0, 70.0, 15.0, "");
    super::widgets::button_box(c, res, x + 110.0, y + 110.0, 70.0, 15.0, "No", no);
    if yes.clicked {
        Some(true)
    } else if no.clicked {
        Some(false)
    } else {
        None
    }
}

/// `ScriptLibrary::DrawConnection`: a 2-pixel light-blue line.
fn link(c: &mut Canvas, a: (f32, f32), b: (f32, f32)) {
    c.line(a, b, Color32::from_rgb(77, 128, 255), 2.0);
}

fn first_load(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    m: &mut Menu,
    agent: String,
    t: f32,
    out: &mut Vec<MenuAct>,
) {
    // The caption types itself out (EclRegisterCaptionChange over 1.5 s).
    if t >= CAPTION_AT {
        let n = (((t - CAPTION_AT) / CAPTION_SECS).min(1.0) * CAPTION.len() as f32) as usize;
        text_lines(c, res, W - 370.0, H - 30.0, &[&CAPTION[..n]]);
    }
    if t >= LOCAL_AT {
        let local = if cfg!(windows) {
            "start/localhost_win32.tif"
        } else {
            "start/localhost_linux.tif"
        };
        picture_scaled(c, res, 230.0, 200.0, 64.0, 78.0, local);
        picture_scaled(c, res, 330.0, 220.0, 32.0, 32.0, "analyser/nonsecure.tif");
        link(c, (330.0 - 43.0 + 1.0, 236.0), (330.0 - 2.0, 236.0));
    }
    if t >= SERVER_AT {
        // The server, its lock and their link slide in from the right over 0.5 s.
        let k = ((t - SERVER_AT) / 0.5).min(1.0);
        let dx = 200.0 * (1.0 - k);
        picture_scaled(
            c,
            res,
            490.0 + dx,
            320.0,
            100.0,
            100.0,
            "start/publicaccessserver.tif",
        );
        picture_scaled(c, res, 450.0 + dx, 340.0, 32.0, 32.0, "analyser/secure.tif");
        let (lx, ly) = (450.0 + 32.0 + dx, 340.0 + 16.0 - 1.0 + 1.0);
        link(c, (lx + 1.0, ly), (lx + 31.0, ly));
        if m.last_t < SERVER_AT {
            st.cues.push(Cue::NewMail);
        }
    }
    if t >= START_AT {
        st.cues.push(Cue::PhonePickup);
        out.push(MenuAct::NewAgent(agent));
    }
    m.last_t = t;
}

/// `LoadingInterface`: one of the loading pictures stretched over the screen.
fn loading(c: &mut Canvas, res: &Res, image: &str) {
    if !image.is_empty() {
        picture_scaled(c, res, 0.0, 0.0, W, H, image);
    }
    c.text(
        sx(440.0),
        sy(450.0) + 10.0,
        "Connecting to GATEWAY...",
        Size::Small,
        res.theme.c("DefaultText"),
    );
}
