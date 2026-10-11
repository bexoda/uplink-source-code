//! The main menu screens from `mainmenu/`: the login screen (`login_interface.cpp`), the new-user
//! start sequence (`firsttimeloading_interface.cpp` with `ScriptLibrary::Script30/31`) and the
//! loading screen (`loading_interface.cpp`). Coordinates are the original's; `SX`/`SY` scale the
//! 640x480 design to the 1024x768 screen.

use eframe::egui::{self, Color32, Key};

use super::canvas::{Canvas, Size, H, W};
use super::widgets::{image_button, picture, picture_scaled};
use super::{Res, State};
use crate::account::{provider_label, Status};
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
    /// New User while signed in: create the agent on the account now, so its seed is ready
    /// when the start sequence ends.
    Starting(String),
    SignIn(String),
    CancelSignIn,
    SignOut,
    SetHandle(String),
}

/// One name in the "Valid User Names" list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    pub name: String,
    /// Saved on this device.
    pub local: bool,
    /// Held by the signed-in Uplink account.
    pub online: bool,
}

/// What the login screen shows about the Uplink account.
#[derive(Debug, Clone)]
pub struct AccountView {
    pub status: Status,
    /// Sign-in providers the server offers (empty when it can't be reached).
    pub providers: Vec<String>,
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
        /// `Enter` was sent; the picture stays up until the game is ready.
        sent: bool,
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
    /// Proceed with a name nobody has: agent waiting for the "create it?" yes/no box.
    confirm_create: Option<String>,
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
            confirm_create: None,
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

    /// Starts the new-user sequence for `agent`; signed in, the account creates it meanwhile.
    fn begin_new(&mut self, agent: String, signed_in: bool, out: &mut Vec<MenuAct>) {
        if signed_in {
            out.push(MenuAct::Starting(agent.clone()));
        }
        self.last_t = -1.0;
        self.screen = Screen::FirstLoad { agent, t0: self.t };
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
            sent: false,
        };
    }
}

pub fn draw(
    c: &mut Canvas,
    ui: &egui::Ui,
    res: &Res,
    st: &mut State,
    m: &mut Menu,
    agents: &[RosterEntry],
    account: &AccountView,
    dt: f32,
    out: &mut Vec<MenuAct>,
) {
    m.t += dt;
    match m.screen.clone() {
        Screen::Login => login(c, ui, res, st, m, agents, account, out),
        Screen::FirstLoad { agent, t0 } => first_load(c, res, st, m, agent, m.t - t0, out),
        Screen::Loading {
            agent,
            t0,
            image,
            sent,
        } => {
            loading(c, res, &image);
            if !sent && m.t - t0 >= LOADING_SECS {
                out.push(MenuAct::Enter(agent.clone()));
                m.screen = Screen::Loading {
                    agent,
                    t0,
                    image,
                    sent: true,
                };
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
    agents: &[RosterEntry],
    account: &AccountView,
    out: &mut Vec<MenuAct>,
) {
    let has = |a: &str| agents.iter().any(|e| e.name == a);
    let signed_in = matches!(account.status, Status::SignedIn { .. });
    // A new account first picks the handle other players see; the User ID box asks for it.
    let choosing_handle = matches!(account.status, Status::SignedIn { handle: None });
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
    let box_open = st.msg.is_some() || m.confirm_retire.is_some() || m.confirm_create.is_some();
    if !box_open {
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
                    Some(a) if has(&a) => st.say(
                        "New User",
                        &format!("An agent called '{a}' already exists. Choose another name."),
                    ),
                    Some(_) if choosing_handle => st.say(
                        "New User",
                        "Choose your handle first: type it in the Name box and click Proceed.",
                    ),
                    Some(a) => m.begin_new(a, signed_in, out),
                },
                1 => match crate::persist::agent_name(&m.name) {
                    Some(a) if has(&a) => m.confirm_retire = Some(a),
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
    if (p.clicked || proceed) && choosing_handle {
        let h = m.name.trim().to_string();
        if h.is_empty() {
            st.say("Handle", "Type the handle you want in the Name box first.");
        } else {
            out.push(MenuAct::SetHandle(h));
            m.name.clear();
            m.code.clear();
        }
    } else if p.clicked || proceed {
        match crate::persist::agent_name(&m.name) {
            Some(a) if has(&a) => {
                st.cues.push(Cue::Login);
                m.start_loading(a);
            }
            // A name nobody has yet: offer to create it instead of sending the player to New User.
            Some(a) => m.confirm_create = Some(a),
            None => st.say(
                "Log in",
                "Type your agent's name in the Name box, or pick one from the list.",
            ),
        }
    }

    // Help text above the box.
    let help: &[&str] = if choosing_handle {
        &[
            "Welcome to Uplink. Choose the handle other",
            "agents will know you by: type it in the Name",
            "box and click Proceed. It can't be changed.",
        ]
    } else {
        &[
            "If you are registered as an Uplink Agent,",
            "enter your username and password below.",
            "Otherwise, click on the New User button.",
        ]
    };
    // A message box sits over this text and is narrower than it.
    if !box_open {
        text_lines(c, res, ux, sy(200.0), help);
    }

    account_panel(c, res, ux, uy + 150.0, account, out);

    // The roster.
    large_text(c, res, sx(35.0), sy(270.0), sy(20.0), "Valid User Names");
    for (i, e) in agents.iter().enumerate() {
        let a = &e.name;
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
        // Signed in, each agent says where it lives: on the account, or on this device only.
        if signed_in {
            let tag = if e.online { "online" } else { "this device" };
            let tw = c.text_width(tag, Size::Small);
            c.text(
                sx(35.0) + sx(170.0) - tw - 6.0,
                y + 10.0,
                tag,
                Size::Small,
                res.theme.rgba("DefaultText", 0.55),
            );
        }
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
            &if agents.iter().any(|e| e.name == a && e.online) {
                format!(
                    "Remove '{a}' from the roster? It is also deleted from your Uplink account."
                )
            } else {
                format!("Are you sure you want to remove '{a}' from the roster?")
            },
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
    if let Some(a) = m.confirm_create.clone() {
        match yes_no(
            c,
            res,
            "New User",
            &format!("There is no agent called '{a}'. Create a new agent with that name?"),
        ) {
            Some(true) => {
                m.confirm_create = None;
                m.begin_new(a, signed_in, out);
            }
            Some(false) => m.confirm_create = None,
            None => {}
        }
    }
}

/// The Uplink account under the User ID box: a heading like "Valid User Names", then
/// full-width buttons in the original button style.
fn account_panel(
    c: &mut Canvas,
    res: &Res,
    x: f32,
    y: f32,
    account: &AccountView,
    out: &mut Vec<MenuAct>,
) {
    if account.status == Status::Off {
        return;
    }
    large_text(c, res, x, y, 20.0, "Uplink Account");
    let note = |c: &mut Canvas, line: usize, s: &str| {
        c.text(
            x,
            y + 34.0 + 14.0 * line as f32,
            s,
            Size::Small,
            res.theme.c("DefaultText"),
        );
    };
    let button = |c: &mut Canvas, i: u32, row: f32, caption: &str, tip: &str| -> bool {
        let by = y + 52.0 + row * 20.0;
        let hit = c.hit("account_button", i, x, by, 220.0, 15.0, tip);
        super::widgets::button_box(c, res, x, by, 220.0, 15.0, caption, hit);
        hit.clicked
    };
    match &account.status {
        Status::Off => {}
        Status::SignedOut => {
            if account.providers.is_empty() {
                note(c, 0, "The Uplink account server can't be reached.");
                note(c, 1, "Your agents are saved on this device.");
                return;
            }
            note(c, 0, "Sign in to keep your agents on every device.");
            for (i, p) in account.providers.iter().enumerate() {
                let caption = format!("Sign in with {}", provider_label(p));
                if button(
                    c,
                    i as u32,
                    i as f32,
                    &caption,
                    "Opens the sign-in page in your web browser",
                ) {
                    out.push(MenuAct::SignIn(p.clone()));
                }
            }
        }
        Status::Waiting { provider } => {
            note(
                c,
                0,
                &format!(
                    "Finish signing in with {} in your browser...",
                    provider_label(provider)
                ),
            );
            if button(c, 100, 0.0, "Cancel", "Stop waiting for the sign-in") {
                out.push(MenuAct::CancelSignIn);
            }
        }
        Status::Connecting => note(c, 0, "Connecting to your Uplink account..."),
        Status::SignedIn { handle } => {
            let who = handle.as_deref().unwrap_or("a new agent");
            note(c, 0, &format!("Signed in as {who}. Agents marked online"));
            note(c, 1, "follow you to every device.");
            if button(
                c,
                101,
                0.5,
                "Sign out",
                "Sign out of your Uplink account on this device",
            ) {
                out.push(MenuAct::SignOut);
            }
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
    if t >= START_AT && m.last_t < START_AT {
        st.cues.push(Cue::PhonePickup);
        out.push(MenuAct::NewAgent(agent));
    }
    if t >= START_AT + 1.0 {
        // Still here: the account server is slow to hand out the agent's seed.
        text_lines(
            c,
            res,
            W - 370.0,
            H - 50.0,
            &["Registering with Uplink Corporation..."],
        );
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
