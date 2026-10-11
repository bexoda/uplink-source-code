//! Panels the toolbar opens on the right-hand side, from `interface/localinterface/*`: the panel
//! background and title bar are shared; each screen fills in its own lines.

use eframe::egui::Color32;
use ur_sim::LayerKind;

use super::canvas::{Canvas, Size, H, W};
use super::remote::layer_caption;
use super::widgets::*;
use super::{Local, Res, State};
use crate::act::Act;
use crate::game::{Game, HopState};

const PANEL_W: f32 = 296.0; // floor(1024 * PANELSIZE)
const TOP: f32 = 187.0; // floor(100 * (PANEL_W / 188)) + 30

/// Uplink's rating names and the score each needs. Our rating is the number of contracts completed.
const RATINGS: [(&str, u32); 17] = [
    ("Unregistered", 0),
    ("Registered", 1),
    ("Beginner", 2),
    ("Novice", 6),
    ("Confident", 15),
    ("Intermediate", 35),
    ("Skilled", 60),
    ("Experienced", 90),
    ("Knowledgeable", 125),
    ("Uber-Skilled", 160),
    ("Professional", 220),
    ("Elite", 300),
    ("Mage", 400),
    ("Expert", 600),
    ("Veteren", 1000),
    ("Techno-mage", 1500),
    ("TERMINAL", 2500),
];

fn rating_index(score: u32) -> usize {
    RATINGS
        .iter()
        .rposition(|(_, need)| score >= *need)
        .unwrap_or(0)
}

/// `text_draw`: lines at (x+10, y+10 + 15 i).
fn text_lines(c: &mut Canvas, res: &Res, x: f32, y: f32, lines: &[String]) {
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

/// `MiniTitleDraw`: a large title at (x+10, y+10).
fn mini_title(c: &mut Canvas, res: &Res, x: f32, y: f32, text: &str) {
    c.text(
        x + 10.0,
        y + 10.0,
        text,
        Size::Large,
        res.theme.c("TitleText"),
    );
}

pub fn draw(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    if st.local == Local::None {
        return;
    }
    let x0 = W - PANEL_W;
    panel(c, res, x0 - 3.0, TOP, PANEL_W, 480.0);
    let (title, tip) = match st.local {
        Local::Hardware => ("GATEWAY", "Remove the gateway screen"),
        Local::Memory => ("MEMORY BANKS", "Remove the memory screen"),
        Local::Status => ("STATUS", "Remove the status screen"),
        Local::Finance => ("FINANCIAL", "Remove the finance screen"),
        Local::Mail => ("NEW MAIL", "Remove the mail screen"),
        Local::Analyser => ("CONNECTION ANALYSER", "Remove this screen"),
        Local::Mission(_) => ("MISSION", "Remove the mission screen"),
        Local::None => return,
    };
    let hit = c.hit("local_title", 0, x0, TOP + 3.0, PANEL_W - 7.0, 15.0, tip);
    button_box(c, res, x0, TOP + 3.0, PANEL_W - 7.0, 15.0, title, hit);
    if hit.clicked {
        st.cues.push(crate::audio::Cue::Close);
        st.local = Local::None;
        return;
    }
    let w = g.world();
    match st.local {
        Local::Hardware => {
            mini_title(c, res, x0 + 40.0, TOP + 40.0, "Hardware");
            text_lines(
                c,
                res,
                x0,
                TOP + 60.0,
                &[
                    format!("Total CPU speed: {} Ghz", w.player.gateway.cpu),
                    format!("Bandwidth: {} Gq/s", w.player.gateway.bandwidth),
                    format!(
                        "Running tasks share your CPU equally ({} now)",
                        w.tasks.len()
                    ),
                ],
            );
            text_lines(c, res, x0, TOP + 130.0, &["DEVICES".into(), "None".into()]);
            mini_title(c, res, x0 + 20.0, TOP + 290.0 - 40.0, "HUD Upgrades");
            text_lines(c, res, x0, TOP + 290.0 + 20.0, &["None".into()]);
        }
        Local::Memory => {
            // One block per installed program and per copied file.
            let mut items: Vec<(String, bool)> = w
                .player
                .tools
                .iter()
                .map(|(t, l)| (format!("{} v{l}.0", super::swmenu::tool_name(*t)), true))
                .collect();
            items.extend(
                w.player
                    .gateway
                    .files
                    .iter()
                    .map(|(_, f)| (f.name.clone(), false)),
            );
            let cap = items.len().max(16);
            c.text(
                x0 + 10.0,
                TOP + 3.0 + 20.0 + 10.0,
                &format!("Memory Capacity : {cap} GigaQuads"),
                Size::Small,
                res.theme.c("DefaultText"),
            );
            for i in 0..cap.min(28) {
                let (bx, by, bw) = (x0 + 5.0, TOP + 50.0 + i as f32 * 10.0, PANEL_W - 7.0 - 25.0);
                let hit = c.hit("memory_block", i as u32, bx, by, bw, 10.0, "");
                c.text(
                    bx,
                    by + 10.0 - 1.0,
                    &format!("{i:03}"),
                    Size::Small,
                    Color32::WHITE,
                );
                let left = match items.get(i) {
                    Some((_, true)) => Color32::from_rgba_unmultiplied(204, 51, 51, 217),
                    Some((_, false)) => Color32::from_rgba_unmultiplied(51, 204, 51, 217),
                    None => Color32::from_rgba_unmultiplied(77, 77, 128, 217),
                };
                let right = if hit.hover {
                    Color32::from_rgba_unmultiplied(153, 153, 204, 217)
                } else {
                    Color32::from_rgba_unmultiplied(77, 77, 204, 217)
                };
                c.gradient(bx + 30.0, by, bw - 30.0, 10.0, [left, left, right, right]);
                if let Some((name, _)) = items.get(i) {
                    c.text(bx + 30.0, by + 8.0, name, Size::Small, Color32::WHITE);
                }
            }
        }
        Local::Status => {
            mini_title(c, res, x0 + 50.0, TOP + 40.0, "Ratings");
            let idx = rating_index(w.player.rating);
            text_lines(
                c,
                res,
                x0,
                TOP + 70.0,
                &[
                    format!("GRADE {} UPLINK AGENT", RATINGS.len() - 1 - idx),
                    format!("Rating: {}", RATINGS[idx].0),
                    format!("Contracts completed: {}", w.player.rating),
                ],
            );
            mini_title(c, res, x0 + 50.0, TOP + 190.0, "Criminal");
            let legal = if w.player.heat == 0 {
                vec!["None".to_string()]
            } else {
                vec![
                    format!(
                        "Traced {} time{}.",
                        w.player.heat,
                        if w.player.heat == 1 { "" } else { "s" }
                    ),
                    "Three traces end your career.".to_string(),
                ]
            };
            text_lines(c, res, x0 + 5.0, TOP + 220.0, &legal);
        }
        Local::Finance => {
            text_lines(
                c,
                res,
                x0,
                TOP + 50.0,
                &[format!("Balance : {} credits", w.player.credits)],
            );
            button_box_state(
                c,
                res,
                x0,
                TOP + 80.0,
                PANEL_W - 7.0,
                15.0,
                "AccNo     Bank            Balance",
                false,
                false,
            );
            let accno = format!("{:07}", g.run.seed % 10_000_000);
            let ip = w
                .servers
                .values()
                .find(|s| s.kind == ur_sim::ServerKind::Service)
                .map(|s| s.ip.clone())
                .unwrap_or_default();
            let hit = c.hit(
                "finance_account",
                0,
                x0,
                TOP + 100.0,
                PANEL_W - 7.0,
                15.0,
                "Your current account",
            );
            button_box(
                c,
                res,
                x0,
                TOP + 100.0,
                PANEL_W - 7.0,
                15.0,
                &format!("{accno}    {ip}    {}c", w.player.credits),
                hit,
            );
        }
        Local::Mail => {
            text_lines(
                c,
                res,
                x0,
                TOP + 40.0,
                &[
                    "E-mail is not part of this version.".into(),
                    "Contracts come from the Uplink".into(),
                    "Bulletin Board instead.".into(),
                ],
            );
        }
        Local::Analyser => analyser(c, res, st, g),
        Local::Mission(id) => {
            let Some(m) = w.missions.iter().find(|m| m.id == id) else {
                st.local = Local::None;
                return;
            };
            let target = match &m.kind {
                ur_sim::MissionKind::StealFile { server, file } => format!(
                    "Copy {file} from {} ({}).",
                    w.servers[*server].name, w.servers[*server].ip
                ),
            };
            text_lines(
                c,
                res,
                x0,
                TOP + 3.0 + 20.0,
                &[
                    "Employer: Uplink Contracts".into(),
                    format!("Payment: {} credits", m.reward),
                ],
            );
            c.text(
                x0 + 10.0,
                TOP + 3.0 + 60.0 + 10.0,
                &target,
                Size::Small,
                res.theme.c("DefaultText"),
            );
            c.text(
                x0 + 10.0,
                TOP + 3.0 + 75.0 + 10.0,
                "Erase your logs before the trace reaches them.",
                Size::Small,
                res.theme.c("DefaultText"),
            );
            let close = c.hit(
                "mission_close",
                0,
                x0,
                TOP + 120.0,
                80.0,
                15.0,
                "Close this screen",
            );
            button_box(c, res, x0, TOP + 120.0, 80.0, 15.0, "Close", close);
            if close.clicked {
                st.cues.push(crate::audio::Cue::Close);
                st.local = Local::None;
            }
        }
        Local::None => {}
    }
    let _ = (acts, H);
}

/// `AnalyserInterface`: your computer, the remote one, a dashed link between them, and an icon for every
/// system guarding it. The part of the link the tracer has crossed turns red.
fn analyser(c: &mut Canvas, res: &Res, st: &State, g: &Game) {
    let x0 = W - PANEL_W;
    let w = g.world();
    let local_hit = c.hit(
        "analyser_localhost",
        0,
        x0 + 20.0,
        TOP + 35.0,
        40.0,
        40.0,
        "Shows your home computer",
    );
    c.text(
        x0 + 6.0 + 15.0,
        TOP + 20.0 + 10.0,
        "Localhost",
        Size::Small,
        res.theme.c("DefaultText"),
    );
    image_button(
        c,
        res,
        x0 + 20.0,
        TOP + 35.0,
        "analyser/localhost.tif",
        local_hit,
        false,
    );

    let Some(sess) = &w.session else {
        c.text(
            x0 + 10.0,
            TOP + 200.0,
            "Not connected.",
            Size::Small,
            res.theme.c("DimmedText"),
        );
        return;
    };
    let srv = &w.servers[sess.target];

    // The link, stippled and moving.
    let path = [
        (x0 + 40.0, TOP + 75.0),
        (x0 + 40.0, TOP + 160.0),
        (W - 40.0, TOP + 160.0),
        (W - 40.0, TOP + 240.0),
    ];
    c.dashed(&path, Color32::WHITE, 2.0, 6.0, 3.0, -st.anim * 20.0);
    // Red from the remote end, by how far the tracer has got overall.
    if let Some(hops) = g.trace_view() {
        let done: f32 = hops
            .iter()
            .map(|h| match h.state {
                HopState::Traced => 1.0,
                HopState::Active => h.progress,
                HopState::Pending => 0.0,
            })
            .sum::<f32>()
            / hops.len().max(1) as f32;
        if done > 0.0 {
            let lens: Vec<f32> = path
                .windows(2)
                .map(|p| ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt())
                .collect();
            let mut left = done * lens.iter().sum::<f32>();
            for (i, seg) in path.windows(2).enumerate().rev() {
                let take = left.min(lens[i]);
                let f = take / lens[i];
                let (a, b) = (seg[1], seg[0]);
                c.line(
                    a,
                    (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f),
                    Color32::from_rgb(255, 0, 0),
                    2.0,
                );
                left -= take;
                if left <= 0.0 {
                    break;
                }
            }
        }
    }

    // One icon per system guarding the machine, spaced along the horizontal run.
    let n = srv.layers.len().max(1) as f32;
    for (i, l) in srv.layers.iter().enumerate() {
        let fx = x0 + 40.0 + (W - 80.0 - (x0 + 40.0)) * (i as f32 + 0.5) / n - 16.0;
        let icon = if l.defeated {
            "analyser/nonsecure.tif"
        } else {
            "analyser/secure.tif"
        };
        let hit = c.hit(
            "analyser_system",
            i as u32,
            fx,
            TOP + 145.0,
            32.0,
            32.0,
            "Shows a security system",
        );
        image_button_blend(c, res, fx, TOP + 145.0, icon, hit, false);
        let cap = match l.kind {
            LayerKind::Password => "Password",
            LayerKind::Cipher => "Cipher",
            LayerKind::Firewall => "Firewall",
            LayerKind::Proxy => "Proxy",
        };
        c.text(
            fx - 4.0,
            TOP + 190.0,
            cap,
            Size::Small,
            res.theme.c("DefaultText"),
        );
        let _ = layer_caption(l.kind);
    }

    let rh = c.hit(
        "analyser_remotehost",
        0,
        W - 60.0,
        TOP + 240.0,
        40.0,
        40.0,
        "Shows the remote computer target",
    );
    image_button(
        c,
        res,
        W - 60.0,
        TOP + 240.0,
        "analyser/remotehost.tif",
        rh,
        false,
    );
    c.text(
        W - 78.0 + 3.0,
        TOP + 281.0 + 10.0,
        "Remotehost",
        Size::Small,
        res.theme.c("DefaultText"),
    );
    c.text(
        x0 + 10.0,
        TOP + 240.0 + 10.0,
        &srv.name,
        Size::Medium,
        res.theme.c("DefaultText"),
    );
    c.text(
        x0 + 10.0,
        TOP + 240.0 + 25.0,
        &srv.ip,
        Size::Small,
        res.theme.c("DimmedText"),
    );
}
