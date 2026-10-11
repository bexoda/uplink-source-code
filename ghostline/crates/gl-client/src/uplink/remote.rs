//! Screens of the machine you are connected to, and the small windows of running programs.
//! Positions and looks come from the original `remoteinterface/*` and `taskmanager/*`.
//!
//! The sim's layered defences become Uplink's "high security" screen: one row per system, each with
//! the game's own Access Denied / Access Granted image, and each opening the matching screen.

use eframe::egui::Color32;
use gl_sim::{Command, LayerKind, ServerId, ServerKind, Task, TaskKind, Tool};

use super::canvas::{Canvas, Size, H};
use super::swmenu::tool_name;
use super::widgets::*;
use super::{Res, State};
use crate::act::Act;
use crate::game::Game;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// The list of security systems (Uplink's high-security screen).
    Systems,
    Password,
    Cypher,
    /// Firewall / proxy: a message screen explaining what to run.
    Notice(LayerKind),
    Menu,
    Files,
    Logs,
    /// Uplink's software shop.
    Sales,
    /// Uplink's contract board.
    Bbs,
}

pub struct Remote {
    pub screen: Screen,
    session: Option<u64>,
    /// `State::anim` when the screen was entered, for the menu's slide-in.
    entered: f32,
    /// Whose logs the log screen shows (defaults to the target).
    pub log_view: Option<ServerId>,
    /// Go back to the systems list at this animation time (after a system falls).
    back_at: Option<f32>,
    /// Selected row on the sales / board screens.
    pub sel: Option<usize>,
    /// Software version shown on the sales screen.
    pub ver: u32,
}

impl Default for Remote {
    fn default() -> Self {
        Remote {
            screen: Screen::Systems,
            session: None,
            entered: 0.0,
            log_view: None,
            back_at: None,
            sel: None,
            ver: 1,
        }
    }
}

impl Remote {
    fn go(&mut self, screen: Screen, now: f32) {
        self.screen = screen;
        self.entered = now;
        self.back_at = None;
    }
}

pub fn layer_caption(k: LayerKind) -> &'static str {
    match k {
        LayerKind::Password => "Password Authentication",
        LayerKind::Cipher => "Data Encryption",
        LayerKind::Firewall => "Firewall",
        LayerKind::Proxy => "Proxy Server",
    }
}

fn main_title(c: &mut Canvas, res: &Res, x: f32, y: f32, text: &str) {
    c.text(
        x,
        y + 12.5 + 5.0,
        text,
        Size::Large,
        res.theme.c("MenuText"),
    );
}

fn sub_title(c: &mut Canvas, res: &Res, x: f32, y: f32, text: &str) {
    c.text(
        x,
        y + 10.0 + 5.0,
        text,
        Size::Medium,
        res.theme.c("DefaultText"),
    );
}

/// A stable eight-letter "password" for display only; the sim has no real password strings.
fn fake_password(seed: &str) -> Vec<char> {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in seed.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    (0..6)
        .map(|i| {
            let v = (h >> (i * 8)) as u8 % 36;
            if v < 26 {
                (b'a' + v) as char
            } else {
                (b'0' + v - 26) as char
            }
        })
        .collect()
}

/// A tiny deterministic scrambler for flickering characters.
fn scramble(n: u64) -> u64 {
    let mut z = n.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn running(g: &Game, server: ServerId, kind: TaskKind) -> Option<(&Task, f32)> {
    g.world()
        .tasks
        .values()
        .find(|t| t.server == server && t.kind == kind)
        .map(|t| (t, t.progress as f32 / t.required.max(1) as f32))
}

pub fn draw(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();
    let Some(sess) = &w.session else {
        st.remote.session = None;
        st.armed = None;
        return;
    };
    let now = st.anim;
    if st.remote.session != Some(sess.id) {
        let first = if w.servers[sess.target].kind == ServerKind::Service {
            Screen::Menu
        } else {
            Screen::Systems
        };
        st.remote = Remote {
            session: Some(sess.id),
            entered: now,
            screen: first,
            ..Remote::default()
        };
        st.armed = None;
        st.tracker = true;
    }
    let target = sess.target;
    let srv = &w.servers[target];

    // A system that just fell sends you back to the list after a moment, like the original's "next page".
    if let Some(at) = st.remote.back_at {
        if now >= at {
            st.remote.go(Screen::Systems, now);
        }
    }
    // If everything is down while you are on a system's own screen, head back to the list.
    match st.remote.screen {
        Screen::Password | Screen::Cypher | Screen::Notice(_) => {
            let kind = match st.remote.screen {
                Screen::Password => LayerKind::Password,
                Screen::Cypher => LayerKind::Cipher,
                Screen::Notice(k) => k,
                _ => unreachable!(),
            };
            let done = srv
                .layers
                .iter()
                .filter(|l| l.kind == kind)
                .all(|l| l.defeated);
            if done && st.remote.back_at.is_none() {
                st.remote.back_at = Some(now + 1.2);
            }
        }
        _ => {}
    }

    match st.remote.screen {
        Screen::Systems => systems(c, res, st, g, acts, srv.name.as_str(), target),
        Screen::Password => password(c, res, st, g, acts, target),
        Screen::Cypher => cypher(c, res, st, g, acts, target),
        Screen::Notice(kind) => notice(c, res, st, kind, srv.name.as_str()),
        Screen::Menu => menu(c, res, st, srv.name.as_str(), srv.kind),
        Screen::Files => files(c, res, st, g, acts, target),
        Screen::Logs => logs(c, res, st, g, acts, target),
        Screen::Sales => sales(c, res, st, g, acts),
        Screen::Bbs => bbs(c, res, st, g, acts),
    }
}

fn systems(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    g: &Game,
    _acts: &mut Vec<Act>,
    name: &str,
    target: ServerId,
) {
    let srv = &g.world().servers[target];
    main_title(c, res, 80.0, 60.0, &name.to_uppercase());
    sub_title(
        c,
        res,
        80.0,
        80.0,
        "High security access. Every system must be cleared.",
    );
    let next = srv.next_layer();
    for (i, l) in srv.layers.iter().enumerate() {
        let y = 140.0 + i as f32 * 40.0;
        let tip = "Click here to access this system";
        let hit = c.hit(
            "highsecurity_systemtitle",
            i as u32,
            30.0,
            y,
            300.0,
            25.0,
            tip,
        );
        let hit2 = c.hit(
            "highsecurity_systemstatus",
            i as u32,
            335.0,
            y,
            100.0,
            20.0,
            tip,
        );
        // `SystemTitleDraw`: two background lines, then the name.
        let denied = !l.defeated;
        let line = if denied {
            eframe::egui::Color32::from_rgb(51, 51, 179)
        } else {
            eframe::egui::Color32::from_rgb(0, 0, 77)
        };
        c.fill(30.0, y + 10.0, 300.0, 2.0, line);
        c.fill(30.0, y + 15.0, 300.0, 2.0, line);
        let tc = if denied {
            Color32::WHITE
        } else {
            Color32::from_gray(128)
        };
        c.text(40.0, y + 18.0, layer_caption(l.kind), Size::Large, tc);
        picture(
            c,
            res,
            335.0,
            y,
            if denied {
                "accessdenied.tif"
            } else {
                "accessgranted.tif"
            },
        );
        if hit.clicked || hit2.clicked {
            if l.defeated || Some(i) == next {
                let to = match l.kind {
                    LayerKind::Password => Screen::Password,
                    LayerKind::Cipher => Screen::Cypher,
                    k => Screen::Notice(k),
                };
                st.remote.go(to, st.anim);
            } else {
                st.say("Message", "That system cannot be reached until the systems in front of it have been cleared.");
            }
        }
    }

    // Proceed (only meaningful once everything is down).
    let hit = c.hit(
        "highsecurity_proceed",
        0,
        335.0,
        350.0,
        80.0,
        15.0,
        "Click here when done",
    );
    if res.sprites.get("proceed.tif").is_some() {
        image_button(c, res, 335.0, 350.0, "proceed.tif", hit, false);
    } else {
        button_box(c, res, 335.0, 350.0, 80.0, 15.0, "Proceed", hit);
    }
    if hit.clicked {
        if srv.locked() {
            st.say("Message", "You have not cleared every system yet.");
        } else {
            // passwordscreen/useridscreen: login.wav on the way in.
            st.cues.push(crate::audio::Cue::Login);
            st.remote.go(Screen::Menu, st.anim);
        }
    }
}

fn password(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    g: &Game,
    acts: &mut Vec<Act>,
    target: ServerId,
) {
    let srv = &g.world().servers[target];
    main_title(c, res, 150.0, 120.0, &srv.name.to_uppercase());
    sub_title(c, res, 150.0, 140.0, "Enter password");
    picture_scaled(c, res, 168.0, 185.0, 220.0, 110.0, "passwordscreen.tif");
    back_button(c, res, st);

    // The password box (CodeButtonDraw): coloured field, one star per character, outline on hover.
    let hit = c.hit(
        "passwordscreen_password",
        0,
        206.0,
        251.0,
        145.0,
        14.0,
        "Target this password box",
    );
    c.fill(
        206.0,
        251.0,
        144.0,
        14.0,
        res.theme.c("PasswordBoxBackground"),
    );
    let stars: String = "*".repeat(6);
    c.text(
        216.0,
        261.0 + 4.0,
        &stars,
        Size::Large,
        res.theme.c("DefaultText"),
    );
    if hit.hover || hit.down {
        c.border(206.0, 251.0, 145.0, 14.0, Color32::WHITE);
    }
    if hit.clicked {
        target_click(st, g, acts, target, None);
    }
}

fn cypher(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    g: &Game,
    acts: &mut Vec<Act>,
    target: ServerId,
) {
    let srv = &g.world().servers[target];
    main_title(c, res, 80.0, 60.0, &srv.name.to_uppercase());
    sub_title(c, res, 80.0, 80.0, "Encrypted. Decrypt to continue.");
    back_button(c, res, st);

    // `DrawCypher`: a 30x14 field of cycling digits; decrypted cells lock to a shaded block counting down.
    let (cols, rows) = (30usize, 14usize);
    let (bx, by, bw, bh) = (50.0, 130.0, 380.0, 250.0);
    let hit = c.hit(
        "cypherscreen_cypher",
        0,
        bx,
        by,
        bw,
        bh,
        "Target this cypher",
    );
    let (done, frac) = layer_progress(g, target, LayerKind::Cipher);
    let locked_cells = if done {
        cols * rows
    } else {
        (frac * (cols * rows) as f32) as usize
    };
    // Cells lock in a fixed pseudo-random order: sort them by a hash of their index.
    let mut idx: Vec<usize> = (0..cols * rows).collect();
    idx.sort_by_key(|k| scramble(*k as u64 ^ 0xC1F4));
    let mut locked = vec![false; cols * rows];
    for k in idx.iter().take(locked_cells) {
        locked[*k] = true;
    }
    let tick = (st.anim * 8.0) as u64;
    c.clipped(bx, by, bw, bh, |c| {
        c.fill(bx, by, bw, bh, res.theme.c("Background"));
        for i in 0..cols {
            for j in 0..rows {
                let xpos = bx + (bw * i as f32 / cols as f32).floor();
                let ypos = by + 12.0 + (bh * j as f32 / rows as f32).floor();
                let k = i * rows + j;
                let digit = (scramble(k as u64 * 31 + tick) % 10) as u8;
                if locked[k] {
                    let shade = (0.2 + (digit.min(9) as f32) / 10.0).min(1.0);
                    let g8 = (shade * 255.0) as u8;
                    let (cw, ch) = (
                        (bw / cols as f32).floor() + 1.0,
                        (bh / rows as f32).floor() + 1.0,
                    );
                    c.fill(xpos, ypos - 10.0, cw, ch, Color32::from_gray(g8));
                    c.text(xpos, ypos, "0", Size::Medium, Color32::WHITE);
                } else {
                    c.text(
                        xpos,
                        ypos,
                        &digit.to_string(),
                        Size::Medium,
                        Color32::from_gray(153),
                    );
                }
            }
        }
    });
    if hit.hover || hit.down {
        c.border(bx, by, bw, bh, Color32::WHITE);
    }
    if hit.clicked {
        target_click(st, g, acts, target, None);
    }

    let p = c.hit(
        "cypherscreen_proceed",
        0,
        350.0,
        380.0,
        80.0,
        15.0,
        "Click here when done",
    );
    button_box(c, res, 350.0, 380.0, 80.0, 15.0, "Proceed", p);
    if p.clicked {
        if done {
            st.remote.go(Screen::Systems, st.anim);
        } else {
            st.say("Message", "The data is still encrypted.");
        }
    }
}

fn notice(c: &mut Canvas, res: &Res, st: &mut State, kind: LayerKind, name: &str) {
    main_title(c, res, 80.0, 60.0, &name.to_uppercase());
    sub_title(c, res, 80.0, 80.0, layer_caption(kind));
    let text = match kind {
        LayerKind::Firewall => "A firewall is protecting this system.\n\nRun a Firewall Bypass from your software menu (Bypassers) to get through. Your first move against a protected system starts the trace.",
        LayerKind::Proxy => "A proxy server is protecting this system.\n\nRun a Proxy Bypass from your software menu (Bypassers) to get through. Your first move against a protected system starts the trace.",
        _ => "",
    };
    let mut y = 135.0;
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let trial = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if c.text_width(&trial, Size::Medium) > 380.0 && !line.is_empty() {
                c.text(50.0, y, &line, Size::Medium, res.theme.c("DefaultText"));
                y += 15.0;
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        c.text(50.0, y, &line, Size::Medium, res.theme.c("DefaultText"));
        y += 15.0;
    }
    let hit = c.hit(
        "messagescreen_click",
        0,
        270.0,
        370.0,
        100.0,
        20.0,
        "Back to the security systems",
    );
    button_box(c, res, 270.0, 370.0, 100.0, 20.0, "Back", hit);
    if hit.clicked {
        st.remote.go(Screen::Systems, st.anim);
    }
}

fn back_button(c: &mut Canvas, res: &Res, st: &mut State) {
    let hit = c.hit(
        "remote_back",
        0,
        3.0,
        22.0,
        50.0,
        15.0,
        "Back to the list of security systems",
    );
    button_box(c, res, 3.0, 22.0, 50.0, 15.0, "Back", hit);
    if hit.clicked {
        st.remote.go(Screen::Systems, st.anim);
    }
}

fn menu(c: &mut Canvas, res: &Res, st: &mut State, name: &str, kind: ServerKind) {
    main_title(c, res, 80.0, 60.0, &name.to_uppercase());
    let options: Vec<(&str, &str, Screen)> = if kind == ServerKind::Service {
        sub_title(
            c,
            res,
            80.0,
            80.0,
            "Welcome, agent. What can we do for you?",
        );
        vec![
            ("Software Sales", "Buy and upgrade software", Screen::Sales),
            (
                "Bulletin Board System",
                "Contracts offered by Uplink's clients",
                Screen::Bbs,
            ),
        ]
    } else {
        sub_title(c, res, 80.0, 80.0, "Internal Services System");
        vec![
            (
                "File Server",
                "Browse the files stored on this system",
                Screen::Files,
            ),
            ("Log Screen", "View the access logs", Screen::Logs),
            (
                "Security Systems",
                "Return to the security systems",
                Screen::Systems,
            ),
        ]
    };
    let interval = 1000.0 / options.len() as f32;
    for (i, (caption, tip, to)) in options.iter().enumerate() {
        // Options slide in from the left, one after another (`EclRegisterMovement`).
        let t = (((st.anim - st.remote.entered) * 1000.0) / ((i + 1) as f32 * interval))
            .clamp(0.0, 1.0);
        let ease = 1.0 - (1.0 - t) * (1.0 - t);
        let ix = -350.0 + (100.0 + 350.0) * ease;
        let tx = -300.0 + (140.0 + 300.0) * ease;
        let y = 120.0 + i as f32 * 30.0;
        let h1 = c.hit("menuscreenimage", i as u32, ix, y + 10.0, 16.0, 16.0, tip);
        let h2 = c.hit("menuscreentext", i as u32, tx, y, 300.0, 32.0, tip);
        let both = super::canvas::Hit {
            hover: h1.hover || h2.hover,
            down: h1.down || h2.down,
            clicked: false,
        };
        image_button(c, res, ix, y + 10.0, "menuscreenoption.tif", both, false);
        c.text(
            tx,
            y + 20.0 + 5.0,
            caption,
            Size::Large,
            res.theme.c("MenuText"),
        );
        if h1.clicked || h2.clicked {
            st.remote.go(*to, st.anim);
        }
    }
}

fn stripe(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, even: bool) {
    let (a, b) = (Color32::from_rgb(8, 20, 80), Color32::from_rgb(8, 20, 0));
    // Alternate rows swap the gradient direction, as in `FileDraw` / `LogDraw`.
    if even {
        c.gradient(x, y, w, h, [b, a, b, a]);
    } else {
        c.gradient(x, y, w, h, [a, b, a, b]);
    }
}

fn files(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    g: &Game,
    acts: &mut Vec<Act>,
    target: ServerId,
) {
    let srv = &g.world().servers[target];
    main_title(c, res, 80.0, 60.0, &srv.name.to_uppercase());
    sub_title(c, res, 80.0, 80.0, "File Server");
    for (x, w, caption) in [
        (15.0, 144.0, "Filename"),
        (162.0, 96.0, "Size"),
        (261.0, 76.0, "Encryption"),
        (340.0, 75.0, "Compression"),
    ] {
        button_box_state(c, res, x, 120.0, w, 15.0, caption, false, false);
    }
    for (i, f) in srv.files.iter().enumerate() {
        let y = 140.0 + i as f32 * 15.0;
        let hit = c.hit(
            "fileserverscreen_file",
            i as u32,
            15.0,
            y,
            400.0,
            14.0,
            "Select this file",
        );
        stripe(c, 15.0, y, 400.0, 14.0, i % 2 == 0);
        c.text(15.0, y + 10.0, &f.name, Size::Small, Color32::WHITE);
        c.text(
            165.0,
            y + 10.0,
            &format!("{} GigaQuads", f.size),
            Size::Small,
            Color32::WHITE,
        );
        let have = g
            .world()
            .player
            .gateway
            .files
            .iter()
            .any(|(s, gf)| *s == target && gf.name == f.name);
        if have {
            c.text(
                265.0,
                y + 10.0,
                "Copied",
                Size::Small,
                Color32::from_rgb(0, 255, 0),
            );
        }
        if hit.hover {
            c.border(15.0, y, 400.0, 14.0, Color32::WHITE);
        }
        if hit.clicked {
            target_click(st, g, acts, target, Some(i));
        }
    }
    let close = c.hit(
        "fileserverscreen_close",
        0,
        421.0,
        121.0,
        13.0,
        13.0,
        "Close the File Server Screen",
    );
    image_button(c, res, 421.0, 121.0, "close.tif", close, false);
    if close.clicked {
        st.remote.go(Screen::Menu, st.anim);
    }
}

fn logs(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    g: &Game,
    acts: &mut Vec<Act>,
    target: ServerId,
) {
    let w = g.world();
    let sess = w.session.as_ref().expect("connected");
    let view = st
        .remote
        .log_view
        .filter(|v| sess.contains(*v))
        .unwrap_or(target);
    let srv = &w.servers[target];
    main_title(c, res, 80.0, 60.0, &srv.name.to_uppercase());
    sub_title(c, res, 80.0, 80.0, "Log Screen");

    // Which machine's logs to show. Your bounce chain's machines keep logs too, and a trace needs them.
    let mut x = 15.0;
    for (i, id) in sess
        .chain
        .iter()
        .copied()
        .chain(std::iter::once(target))
        .enumerate()
    {
        let name = &w.servers[id].name;
        let bw = 20.0 + name.len() as f32 * 6.0;
        let hit = c.hit(
            "logscreen_view",
            i as u32,
            x,
            100.0,
            bw,
            15.0,
            "Show the logs kept on this machine",
        );
        button_box_state(
            c,
            res,
            x,
            100.0,
            bw,
            15.0,
            name,
            hit.hover || view == id,
            hit.down,
        );
        if hit.clicked {
            st.remote.log_view = Some(id);
        }
        x += bw + 3.0;
    }

    for (x, w_, caption) in [(15.0, 96.0, "Date"), (115.0, 300.0, "Action")] {
        button_box_state(c, res, x, 120.0, w_, 15.0, caption, false, false);
    }
    let vs = &w.servers[view];
    let mut row = 0;
    for entry in vs.logs.iter().rev().filter(|l| !l.erased) {
        let y = 140.0 + row as f32 * 15.0;
        if row >= 13 {
            break;
        }
        let hit = c.hit(
            "logscreen_log",
            row as u32,
            15.0,
            y,
            400.0,
            14.0,
            "Select this log",
        );
        stripe(c, 15.0, y, 400.0, 14.0, row % 2 == 0);
        let date = super::hud::game_date(entry.tick, gl_sim::tuning::TICKS_PER_SECOND as u64);
        let short: String = date.split(',').next().unwrap_or("").to_string();
        c.text(25.0, y + 10.0, &short, Size::Small, Color32::WHITE);
        let from = if entry.session == sess.id {
            // The machine this connection came from.
            let order: Vec<ServerId> = sess
                .chain
                .iter()
                .copied()
                .chain(std::iter::once(target))
                .collect();
            match order.iter().position(|s| *s == view) {
                Some(0) => "your gateway".to_string(),
                Some(p) => w.servers[order[p - 1]].ip.clone(),
                None => "unknown".to_string(),
            }
        } else {
            "an earlier connection".to_string()
        };
        c.text(
            125.0,
            y + 10.0,
            &format!("Connection from {from}"),
            Size::Small,
            Color32::WHITE,
        );
        if hit.hover {
            c.border(15.0, y, 400.0, 14.0, Color32::WHITE);
        }
        if hit.clicked && entry.session == sess.id {
            // Only your current connection's entry can be erased from here.
            target_click_log(st, g, acts, view);
        } else if hit.clicked {
            st.say("Message", "That entry belongs to an earlier connection. Only the log of your current connection matters to a trace.");
        }
        row += 1;
    }
    if row == 0 {
        c.text(
            25.0,
            150.0,
            "No log entries.",
            Size::Small,
            res.theme.c("DimmedText"),
        );
    }
    let close = c.hit(
        "logscreen_close",
        0,
        421.0,
        121.0,
        13.0,
        13.0,
        "Close the Log Screen",
    );
    image_button(c, res, 421.0, 121.0, "close.tif", close, false);
    if close.clicked {
        st.remote.go(Screen::Menu, st.anim);
    }
}

/// `SetProgramTarget`: the armed program works on whatever was clicked. If nothing is armed, say so.
fn target_click(
    st: &mut State,
    g: &Game,
    acts: &mut Vec<Act>,
    server: ServerId,
    file: Option<usize>,
) {
    match st.armed.take() {
        Some(tool) => acts.push(Act::Cmd(Command::Run { tool, server, file })),
        None => {
            let _ = g;
            st.say(
                "Message",
                "Run a program from the software menu first, then click what it should work on.",
            );
        }
    }
}

fn target_click_log(st: &mut State, g: &Game, acts: &mut Vec<Act>, server: ServerId) {
    match st.armed {
        Some(Tool::LogDeleter) => {
            st.armed = None;
            acts.push(Act::Cmd(Command::Run {
                tool: Tool::LogDeleter,
                server,
                file: None,
            }));
        }
        Some(_) => st.say("Message", "That program can't be used on a log."),
        None => {
            let _ = g;
            st.say("Message", "Run the Log Deleter from the software menu (Security), then click the log to erase.");
        }
    }
}

/// (finished, progress 0..1) for the first system of `kind` on `server`.
fn layer_progress(g: &Game, server: ServerId, kind: LayerKind) -> (bool, f32) {
    let srv = &g.world().servers[server];
    let Some((i, l)) = srv.layers.iter().enumerate().find(|(_, l)| l.kind == kind) else {
        return (true, 1.0);
    };
    if l.defeated {
        return (true, 1.0);
    }
    (
        false,
        running(g, server, TaskKind::Defeat { layer: i })
            .map(|(_, f)| f)
            .unwrap_or(0.0),
    )
}

/// The small windows of running programs, and of an armed program waiting for a target.
pub fn draw_programs(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();
    let mut used_rows: Vec<f32> = Vec::new();
    let mut slot = |want: f32| {
        let mut y = want;
        while used_rows.iter().any(|u| (u - y).abs() < 19.0) {
            y += 20.0;
        }
        used_rows.push(y);
        y
    };

    trace_tracker(c, res, st, g);

    // Armed program first (it has no task yet).
    if let (Some(tool), true) = (st.armed, g.connected()) {
        if program_window(c, res, st, tool, "Select target", 0.0, None, &mut slot, 0) {
            st.armed = None;
        }
    }

    let tasks: Vec<(gl_sim::TaskId, &Task)> = w.tasks.iter().collect();
    for (n, (id, t)) in tasks.into_iter().enumerate() {
        let frac = t.progress as f32 / t.required.max(1) as f32;
        let caption = match t.kind {
            TaskKind::Copy { file } => w.servers[t.server]
                .files
                .get(file)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| tool_name(t.tool).into()),
            _ => tool_name(t.tool).to_string(),
        };
        if program_window(
            c,
            res,
            st,
            t.tool,
            &caption,
            frac,
            Some(t),
            &mut slot,
            n as u32 + 1,
        ) {
            acts.push(Act::Cmd(Command::Cancel(id)));
        }
    }
}

/// Draws one program window; returns true if its close box was clicked.
#[allow(clippy::too_many_arguments)]
fn program_window(
    c: &mut Canvas,
    res: &Res,
    st: &mut State,
    tool: Tool,
    caption: &str,
    frac: f32,
    task: Option<&Task>,
    slot: &mut dyn FnMut(f32) -> f32,
    n: u32,
) -> bool {
    if tool == Tool::PasswordBreaker {
        // A title bar over a body that shows the characters as they are cracked.
        let (x, y) = (245.0, 360.0);
        button_box_state(c, res, x, y, 116.0, 15.0, "Password Breaker", false, false);
        let body = c.hit(
            "passwordbreaker",
            n,
            x,
            y + 15.0,
            130.0,
            30.0,
            "Password Breaker",
        );
        let (a, b) = (Color32::from_rgb(8, 20, 0), Color32::from_rgb(8, 20, 124));
        c.gradient(x, y + 15.0, 130.0, 30.0, [a, b, a, b]);
        c.border(x, y + 15.0, 130.0, 30.0, Color32::from_rgb(81, 138, 215));
        let pw = fake_password("pw");
        if task.is_none() {
            c.text(
                x + 10.0,
                y + 15.0 + 18.0,
                "Select target",
                Size::Medium,
                Color32::from_rgba_unmultiplied(255, 255, 255, 204),
            );
        } else {
            let known = (frac * pw.len() as f32) as usize;
            let tick = (st.anim * 12.0) as u64;
            for (i, ch) in pw.iter().enumerate() {
                let shown = if i < known {
                    *ch
                } else {
                    (b'a' + (scramble(i as u64 * 7 + tick) % 26) as u8) as char
                };
                let col = if i < known {
                    Color32::WHITE
                } else {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 110)
                };
                c.text(
                    x + 10.0 + i as f32 * 20.0,
                    y + 15.0 + 18.0,
                    &shown.to_string(),
                    Size::Large,
                    col,
                );
            }
        }
        let _ = body;
        let close = c.hit(
            "passwordbreaker_close",
            n,
            362.0,
            361.0,
            13.0,
            13.0,
            "Close the password breaker",
        );
        image_button(c, res, 362.0, 361.0, "close.tif", close, false);
        return close.clicked;
    }

    let want = match tool {
        Tool::ProxyBypass => 422.0,
        Tool::FileCopier => 432.0,
        Tool::FirewallBypass => 442.0,
        _ => 450.0,
    };
    let y = slot(want);
    let icon = match tool {
        Tool::Decrypter => "software/dec.tif",
        Tool::LogDeleter => "software/log.tif",
        Tool::FileCopier => "software/cpy.tif",
        _ => "software/go.tif",
    };
    let tip = tool_name(tool);
    c.hit("program_icon", n, 265.0, y, 20.0, 15.0, tip);
    picture(c, res, 265.0, y, icon);
    // `BorderDraw`
    let hov = c.hit(
        "program_border",
        n,
        285.0,
        y,
        120.0,
        15.0,
        "Shows the progress of this program",
    );
    let al = 0.85;
    let (c1, c2) = if hov.hover {
        ((0.2, 0.2, 0.5), (0.5, 0.5, 0.6))
    } else {
        ((0.2, 0.2, 0.4), (0.3, 0.3, 0.5))
    };
    let col = |v: (f32, f32, f32)| {
        Color32::from_rgba_unmultiplied(
            (v.0 * 255.0) as u8,
            (v.1 * 255.0) as u8,
            (v.2 * 255.0) as u8,
            (al * 255.0) as u8,
        )
    };
    c.gradient(285.0, y, 120.0, 15.0, [col(c2), col(c1), col(c2), col(c1)]);
    if hov.hover {
        c.border(285.0, y, 120.0, 15.0, Color32::from_rgb(77, 77, 179));
    }
    // `ProgressDraw`: a bar that shifts from green toward blue as it fills.
    let width = if task.is_none() {
        120.0
    } else {
        (frac * 120.0).max(1.0)
    };
    let scale = width / 100.0;
    let (gc, bc) = ((1.5 - scale).clamp(0.0, 1.0), scale.clamp(0.0, 1.0));
    c.fill(
        285.0,
        y + 1.0,
        width,
        13.0,
        Color32::from_rgba_unmultiplied(0, (gc * 255.0) as u8, (bc * 255.0) as u8, 153),
    );
    c.text(
        290.0,
        y + 1.0 + 6.5 + 3.0,
        caption,
        Size::Small,
        Color32::WHITE,
    );
    let close = c.hit(
        "program_close",
        n,
        405.0,
        y + 1.0,
        13.0,
        13.0,
        "Close this program",
    );
    image_button(c, res, 405.0, y + 1.0, "close.tif", close, false);
    let _ = H;
    close.clicked
}

/// Short name for the CPU panel.
pub fn tool_label_for_task(t: &Task) -> String {
    tool_name(t.tool).to_string()
}

/// The tools in the order the shop lists them.
const SHOP: [Tool; 6] = [
    Tool::PasswordBreaker,
    Tool::Decrypter,
    Tool::FirewallBypass,
    Tool::ProxyBypass,
    Tool::LogDeleter,
    Tool::FileCopier,
];

fn blurb(t: Tool) -> &'static str {
    match t {
        Tool::PasswordBreaker => "Cracks the passwords that guard a system. Higher versions work faster.",
        Tool::Decrypter => "Breaks data encryption. Higher versions work faster.",
        Tool::FirewallBypass => "Gets you through a firewall. Higher versions work faster.",
        Tool::ProxyBypass => "Disables a proxy server. Higher versions work faster.",
        Tool::LogDeleter => "Erases the record of your connection from a machine's logs. A trace can't cross a machine whose log you erased.",
        Tool::FileCopier => "Copies a file from the system you are connected to onto your gateway.",
    }
}

/// A row in a table: `clear_draw`, then the highlight gradient if selected, then a border if hovered.
fn row(c: &mut Canvas, res: &Res, x: f32, y: f32, w: f32, h: f32, selected: bool, hover: bool) {
    c.fill(x, y, w, h, res.theme.c("Background"));
    if selected {
        let t = &res.theme;
        let (a, b) = (t.c("PanelHighlightA"), t.c("PanelHighlightB"));
        c.gradient(x, y, w, h, [a, a, b, b]);
    }
    if hover || selected {
        c.border(x, y, w, h, res.theme.c("PanelBorder"));
    }
}

fn wrap(c: &Canvas, text: &str, width: f32, size: Size) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let trial = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if c.text_width(&trial, size) > width && !line.is_empty() {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = trial;
            }
        }
        out.push(line);
    }
    out
}

/// A details panel (`DrawDetails`): panel gradient and border with wrapped text.
fn details(c: &mut Canvas, res: &Res, x: f32, y: f32, w: f32, h: f32, text: &str) {
    super::widgets::panel(c, res, x, y, w, h);
    for (i, l) in wrap(c, text, w - 20.0, Size::Small).iter().enumerate() {
        if i as f32 * 15.0 + 20.0 > h {
            break;
        }
        c.text(
            x + 10.0,
            y + 10.0 + 10.0 + i as f32 * 15.0,
            l,
            Size::Small,
            res.theme.c("DefaultText"),
        );
    }
}

fn sales(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();
    let t = &res.theme;
    for (x, wd, caption) in [
        (20.0, 237.0, "Name"),
        (260.0, 22.0, "Ver"),
        (285.0, 60.0, "Cost"),
        (350.0, 57.0, "Size"),
    ] {
        button_box_state(c, res, x, 30.0, wd, 15.0, caption, false, false);
    }
    let owned = |tool: Tool| w.player.tools.get(&tool).copied().unwrap_or(0);
    for (i, tool) in SHOP.iter().enumerate() {
        let y = 50.0 + i as f32 * 20.0;
        let hit = c.hit(
            "SWsale",
            i as u32,
            20.0,
            y,
            624.0,
            17.0,
            "View details on this item",
        );
        let selected = st.remote.sel == Some(i);
        row(c, res, 20.0, y, 624.0, 17.0, selected, hit.hover);
        let ver = if selected {
            st.remote.ver
        } else {
            (owned(*tool) + 1).min(5)
        };
        let cost = tool.base_price() * ver as i64;
        let col = if w.player.credits >= cost {
            t.c("DefaultText")
        } else {
            t.c("DimmedText")
        };
        c.text(25.0, y + 10.0, tool_name(*tool), Size::Small, col);
        c.text(260.0, y + 10.0, &format!("v{ver}.0"), Size::Small, col);
        c.text(290.0, y + 10.0, &format!("{cost}c"), Size::Small, col);
        c.text(
            355.0,
            y + 10.0,
            if owned(*tool) >= ver { "owned" } else { "-" },
            Size::Small,
            col,
        );
        if hit.clicked {
            st.remote.sel = Some(i);
            st.remote.ver = (owned(*tool) + 1).min(5);
        }
    }
    let base_y = 50.0 + SHOP.len() as f32 * 20.0 + 10.0;
    let text = match st.remote.sel {
        Some(i) => {
            let tool = SHOP[i];
            let ver = st.remote.ver;
            format!(
                "{} version {}.0\nCost: {} credits   (you have {})\nYou own: {}\n\n{}",
                tool_name(tool),
                ver,
                tool.base_price() * ver as i64,
                w.player.credits,
                if owned(tool) == 0 {
                    "nothing yet".to_string()
                } else {
                    format!("version {}.0", owned(tool))
                },
                blurb(tool)
            )
        }
        None => "Select an item to see its details.".to_string(),
    };
    details(c, res, 20.0, base_y, 565.0, 110.0, &text);
    let bx = 588.0;
    let buy = c.hit(
        "swsales_accept",
        0,
        bx,
        base_y,
        72.0,
        15.0,
        "Purchase this item",
    );
    button_box(c, res, bx, base_y, 72.0, 15.0, "Purchase", buy);
    if buy.clicked {
        match st.remote.sel {
            None => st.say("Message", "Select an item first."),
            Some(i) => {
                let tool = SHOP[i];
                let (ver, have) = (st.remote.ver, owned(tool));
                if have >= ver {
                    st.say("Message", "You already have that version.");
                } else if ver > have + 1 {
                    st.say("Message", "You need the earlier versions first.");
                } else {
                    acts.push(Act::Cmd(Command::BuyTool(tool)));
                    st.remote.ver = (ver + 1).min(5);
                }
            }
        }
    }
    let next = c.hit(
        "swsales_nextver",
        0,
        bx,
        base_y + 20.0,
        72.0,
        15.0,
        "View the next version",
    );
    button_box(c, res, bx, base_y + 20.0, 72.0, 15.0, "Next Version", next);
    if next.clicked && st.remote.sel.is_some() {
        st.remote.ver = (st.remote.ver + 1).min(5);
    }
    let prev = c.hit(
        "swsales_prevver",
        0,
        bx,
        base_y + 40.0,
        72.0,
        15.0,
        "View the previous version",
    );
    button_box(c, res, bx, base_y + 40.0, 72.0, 15.0, "Prev Version", prev);
    if prev.clicked && st.remote.sel.is_some() {
        st.remote.ver = st.remote.ver.saturating_sub(1).max(1);
    }
    exit_button(
        c,
        res,
        st,
        bx,
        base_y + 110.0 - 15.0,
        "Exit Market",
        "Close the screen and return to the main menu",
    );
}

fn bbs(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();
    let t = &res.theme;
    button_box_state(c, res, 20.0, 30.0, 100.0, 15.0, "Date", false, false);
    button_box_state(c, res, 125.0, 30.0, 519.0, 15.0, "Subject", false, false);
    let date = super::hud::game_date(0, 10);
    let date = date.split(", ").nth(1).unwrap_or("").to_string();
    for (i, m) in w.missions.iter().enumerate() {
        let y = 50.0 + i as f32 * 20.0;
        let hit = c.hit(
            "BBmessage",
            i as u32,
            20.0,
            y,
            620.0,
            17.0,
            "Read this Bulletin Board message",
        );
        let selected = st.remote.sel == Some(i);
        row(c, res, 20.0, y, 620.0, 17.0, selected, hit.hover);
        let col = if m.done {
            t.c("DimmedText")
        } else {
            t.c("DefaultText")
        };
        c.text(25.0, y + 10.0, &date, Size::Small, col);
        let subject = if m.done {
            format!("{}  (completed)", m.title)
        } else if m.accepted {
            format!("{}  (accepted)", m.title)
        } else {
            m.title.clone()
        };
        c.text(130.0, y + 10.0, &subject, Size::Small, col);
        if hit.clicked {
            st.remote.sel = Some(i);
        }
    }
    let base_y = 50.0 + w.missions.len() as f32 * 20.0 + 10.0;
    let text = match st.remote.sel.and_then(|i| w.missions.get(i)) {
        Some(m) => {
            let target = match &m.kind {
                gl_sim::MissionKind::StealFile { server, file } => {
                    format!("Copy the file {file} from {}.", w.servers[*server].name)
                }
            };
            format!("{}\n\n{target}\nPayment on completion: {} credits.\n\nGet in through a chain of relays, clear every system, and copy the file. Erase your logs or the trace will find you.", m.title, m.reward)
        }
        None => "Select a contract to read it.".to_string(),
    };
    details(c, res, 20.0, base_y, 565.0, 110.0, &text);
    let bx = 588.0;
    let acc = c.hit(
        "bbs_accept",
        0,
        bx,
        base_y,
        70.0,
        15.0,
        "Accept this mission",
    );
    button_box(c, res, bx, base_y, 70.0, 15.0, "Accept", acc);
    if acc.clicked {
        match st.remote.sel.and_then(|i| w.missions.get(i)) {
            None => st.say("Message", "Select a contract first."),
            Some(m) if m.done => st.say("Message", "That contract is already complete."),
            Some(m) if m.accepted => st.say("Message", "You have already accepted that contract."),
            Some(m) => acts.push(Act::Cmd(Command::AcceptMission(m.id))),
        }
    }
    exit_button(
        c,
        res,
        st,
        bx,
        base_y + 110.0 - 15.0,
        "Exit BBS",
        "Close the BBS screen and return to the main menu",
    );
}

fn exit_button(c: &mut Canvas, res: &Res, st: &mut State, x: f32, y: f32, label: &str, tip: &str) {
    let hit = c.hit("remote_exit", 0, x, y, 72.0, 15.0, tip);
    button_box(c, res, x, y, 72.0, 15.0, label, hit);
    if hit.clicked {
        st.remote.go(Screen::Menu, st.anim);
        st.remote.sel = None;
    }
}

/// `TraceTracker`: a small window at (25, 300) that glows brighter and pulses while a trace is running.
fn trace_tracker(c: &mut Canvas, res: &Res, st: &mut State, g: &Game) {
    if !g.connected() || !st.tracker {
        return;
    }
    let tracing = g.world().trace.is_some();
    let progress = g
        .trace_view()
        .map(|h| {
            h.iter()
                .map(|x| match x.state {
                    crate::game::HopState::Traced => 1.0,
                    crate::game::HopState::Active => x.progress,
                    crate::game::HopState::Pending => 0.0,
                })
                .sum::<f32>()
                / h.len().max(1) as f32
        })
        .unwrap_or(0.0);
    // tracetracker.cpp bleeps with a gap of (time left)/20; ours shrinks with trace progress.
    if tracing {
        if st.anim >= st.next_beep {
            st.cues.push(crate::audio::Cue::TraceBleep);
            st.next_beep = st.anim + (3.0 * (1.0 - progress)).max(0.15);
        }
    } else {
        st.next_beep = 0.0;
    }
    // The nearer the tracer, the faster it pulses.
    let brightness = if tracing {
        0.35 + 0.45 * (0.5 + 0.5 * (st.anim * (3.0 + 9.0 * progress) * std::f32::consts::TAU).sin())
    } else {
        0.0
    };
    let (x, y, w, h) = (25.0, 300.0, 100.0, 13.0);
    let hit = c.hit(
        "tracetracker",
        0,
        x,
        y,
        w,
        h,
        "Trace Tracker: pulses faster the closer the trace gets",
    );
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    let a = Color32::from_rgba_unmultiplied(u(brightness), u(brightness), u(0.7), 128);
    let b = Color32::from_rgba_unmultiplied(u(brightness), u(brightness), u(0.4), 128);
    c.gradient(x, y, w, h, [a, b, a, b]);
    let label = "Trace Tracker";
    let tx = x + w / 2.0 - c.text_width(label, Size::Small) / 2.0;
    c.text(
        tx,
        y + h / 2.0 + 2.0,
        label,
        Size::Small,
        if hit.hover {
            Color32::WHITE
        } else {
            Color32::from_rgba_unmultiplied(255, 255, 255, 217)
        },
    );
    let close = c.hit(
        "tracetracker_close",
        0,
        x + w,
        y,
        13.0,
        13.0,
        "Close the TraceTracker",
    );
    image_button(c, res, x + w, y, "close.tif", close, false);
    if close.clicked {
        st.tracker = false;
    }
    let _ = res;
}
