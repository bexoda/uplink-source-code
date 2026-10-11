//! World map: small panel, large map, location dots, connection and trace. From
//! `worldmap_interface.cpp` / `worldmap_layout.cpp`.

use eframe::egui::Color32;
use gl_sim::{ServerId, ServerKind};

use super::canvas::{Canvas, Size, H, W};
use super::widgets::*;
use super::{Res, State};
use crate::act::Act;
use crate::game::Game;

pub const PANELSIZE: f32 = 0.29;

/// (x, y, w, h) of the small map, as `CreateWorldMapInterface_Small` computes it.
pub fn small_rect() -> (f32, f32, f32, f32) {
    let w = (W * PANELSIZE).floor();
    let h = (100.0 * (w / 188.0)).floor();
    (W - w - 3.0, 3.0, w, h)
}

/// (x, y, w, h) of the large map (`GetLargeMapRect`).
pub fn large_rect() -> (f32, f32, f32, f32) {
    let w = W - 23.0 * 2.0;
    (23.0, 50.0, w, (316.0 * (w / 595.0)).floor())
}

/// The connection as normalised map points: gateway, relays in order, then the target.
fn route_points(st: &State, g: &Game) -> Vec<(f32, f32)> {
    let w = g.world();
    let (chain, target) = match &w.session {
        Some(s) => (s.chain.clone(), Some(s.target)),
        None => (g.chain.clone(), g.target),
    };
    let mut pts = vec![st.gw];
    pts.extend(chain.iter().filter_map(|id| st.pos.get(id).copied()));
    if let Some(p) = target.and_then(|t| st.pos.get(&t).copied()) {
        pts.push(p);
    }
    pts
}

/// Red where the tracer has been: whole traced hops, plus the fraction of the active one.
/// Returns, per segment of `route_points`, how much of it is traced (0..=1), player side first.
fn traced_fraction(g: &Game, segments: usize) -> Vec<f32> {
    let mut out = vec![0.0; segments];
    let Some(hops) = g.trace_view() else {
        return out;
    };
    // `trace_view` lists hops player-side first; each hop's segment joins it to the one before it.
    for (i, h) in hops.iter().enumerate() {
        if i < segments {
            out[i] = match h.state {
                crate::game::HopState::Traced => 1.0,
                crate::game::HopState::Active => h.progress,
                crate::game::HopState::Pending => 0.0,
            };
        }
    }
    out
}

fn draw_route(
    c: &mut Canvas,
    res: &Res,
    st: &State,
    g: &Game,
    to_px: &dyn Fn((f32, f32)) -> (f32, f32),
    width: f32,
    live_trace: bool,
) {
    let pts: Vec<(f32, f32)> = route_points(st, g).into_iter().map(to_px).collect();
    if pts.len() < 2 {
        return;
    }
    let white = res.theme.c("DefaultText");
    c.dashed(&pts, white, width, 6.0, 3.0, -st.anim * 20.0);
    if live_trace && g.world().trace.is_some() {
        let frac = traced_fraction(g, pts.len() - 1);
        for (i, f) in frac.iter().enumerate() {
            if *f > 0.0 {
                // Hop i (player side first) sits at point i+1 and the tracer crawls from it toward point i.
                let (a, b) = (pts[i + 1], pts[i]);
                let end = (a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f);
                c.line(a, end, Color32::from_rgb(255, 0, 0), width);
            }
        }
    }
}

/// Everything about a server shown on the map.
struct Dot {
    id: ServerId,
    name: String,
    ip: String,
    target: bool,
}

fn dots(g: &Game) -> Vec<Dot> {
    g.relays()
        .into_iter()
        .chain(g.targets())
        .chain(g.services())
        .map(|id| {
            let s = &g.world().servers[id];
            Dot {
                id,
                name: s.name.clone(),
                ip: s.ip.clone(),
                target: s.kind != ServerKind::Relay,
            }
        })
        .collect()
}

/// The small panel at the top right.
pub fn small(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, _acts: &mut Vec<Act>) {
    let (x, y, w, h) = small_rect();
    let hit = c.hit("worldmap_smallmap", 0, x, y, w, h, "Global Communications");
    picture_scaled(c, res, x, y, w, h, "worldmapsmall.tif");
    let to_px = |p: (f32, f32)| (x + p.0 * w, y + p.1 * h);

    if g.connected() || !g.chain.is_empty() || g.target.is_some() {
        draw_route(c, res, st, g, &to_px, 2.0, true);
        for p in route_points(st, g) {
            let (px, py) = to_px(p);
            c.fill(px - 1.0, py - 1.0, 3.0, 3.0, Color32::WHITE);
        }
    } else {
        let (px, py) = to_px(st.gw);
        c.fill(px - 1.0, py - 1.0, 3.0, 3.0, Color32::WHITE);
    }
    let border = if hit.hover || hit.down {
        Color32::from_rgb(102, 102, 204)
    } else {
        Color32::from_rgb(81, 138, 215)
    };
    c.border(x, y, w, h, border);
    if hit.clicked {
        st.map_large = true;
    }
    let corner = c.hit(
        "worldmap_toprightclick",
        0,
        W - 3.0,
        0.0,
        3.0,
        3.0,
        "Global Communications",
    );
    if corner.clicked {
        st.map_large = true;
    }

    // Connect / Disconnect bar under the map (only when there is something to do).
    if g.connected() {
        let bar = c.hit(
            "worldmap_connect",
            0,
            x,
            y + h + 4.0,
            w,
            15.0,
            "Break the connection to your current remote host",
        );
        button_box(c, res, x, y + h + 4.0, w, 15.0, "Disconnect", bar);
        if bar.clicked {
            _acts.push(Act::Disconnect);
        }
    }
}

/// The full map with its controls, over the middle of the screen.
pub fn large(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let (x, y, w, h) = large_rect();
    picture_scaled(c, res, x, y, w, h, "worldmaplarge.tif");
    c.border(x, y, w, h, Color32::from_rgb(81, 138, 215));
    let to_px = |p: (f32, f32)| (x + p.0 * w, y + p.1 * h);
    let w_ = g.world();
    let connected = g.connected();

    c.clipped(x, y, w, h, |c| {
        draw_route(c, res, st, g, &to_px, 2.0, true);
    });

    // Gateway.
    let (gx, gy) = to_px(st.gw);
    c.fill(gx - 3.0, gy - 3.0, 7.0, 7.0, Color32::WHITE);
    c.text(
        gx + 10.0,
        gy + 7.0,
        "Localhost",
        Size::Small,
        Color32::WHITE,
    );
    c.hit(
        "worldmap_localhost",
        0,
        gx - 3.0,
        gy - 3.0,
        7.0,
        7.0,
        "Your physical location",
    );

    // Contract targets show green; places in the connection show orange.
    let mission_targets: Vec<ServerId> = w_
        .missions
        .iter()
        .filter(|m| m.accepted && !m.done)
        .map(|m| match &m.kind {
            gl_sim::MissionKind::StealFile { server, .. } => *server,
        })
        .collect();
    let in_route = |id: ServerId| match &w_.session {
        Some(s) => s.contains(id),
        None => g.chain.contains(&id) || g.target == Some(id),
    };

    let mut info: Option<(f32, f32, String, String)> = None;
    for (i, d) in dots(g).into_iter().enumerate() {
        let Some(&p) = st.pos.get(&d.id) else {
            continue;
        };
        let (px, py) = to_px(p);
        let colour = if mission_targets.contains(&d.id) {
            Color32::from_rgb(0, 255, 0)
        } else if in_route(d.id) {
            Color32::from_rgb(255, 128, 0)
        } else {
            Color32::WHITE
        };
        let tip = format!("Connect to IP address {}", d.ip);
        let hit = c.hit("worldmap_dot", i as u32, px - 3.0, py - 3.0, 7.0, 7.0, &tip);
        c.fill(px - 3.0, py - 3.0, 7.0, 7.0, colour);
        // Long names would run off the edge of the map; the info box has the full one.
        let label = if d.name.starts_with("Uplink") {
            "Uplink Services"
        } else {
            d.name.as_str()
        };
        c.text(px + 10.0, py + 7.0, label, Size::Small, Color32::WHITE);
        if hit.hover || hit.down {
            info = Some((px, py, d.ip.clone(), d.name.clone()));
        }
        if hit.clicked {
            if connected {
                st.help = "You cannot modify an existing connection. Disconnect first.".into();
            } else if d.target {
                acts.push(Act::ToggleTarget(d.id));
            } else {
                acts.push(Act::ToggleRelay(d.id));
            }
        }
    }

    // Info box for the dot under the mouse.
    if let Some((px, py, ip, owner)) = info {
        let (l1, l2) = (format!("IP: {ip}"), format!("Owner: {owner}"));
        let bw = c
            .text_width(&l1, Size::Small)
            .max(c.text_width(&l2, Size::Small))
            + 10.0;
        let (bh, mut bx, mut by) = (26.0, px - 15.0, py + 20.0);
        bx = bx.clamp(x, (x + w - bw - 2.0).max(x));
        by = by.clamp(y, (y + h - bh - 2.0).max(y));
        let (a, b) = (Color32::from_rgb(8, 20, 0), Color32::from_rgb(8, 20, 124));
        c.gradient(bx, by, bw, bh, [a, b, a, b]);
        c.border(bx, by, bw, bh, Color32::from_rgb(81, 138, 215));
        c.text(bx + 5.0, by + 10.0, &l1, Size::Small, Color32::WHITE);
        c.text(bx + 5.0, by + 20.0, &l2, Size::Small, Color32::WHITE);
    }

    // Controls under the map.
    let by = y + h + 5.0;
    c.text(
        235.0 + 10.0,
        y + h + 3.0 + 10.0,
        &st.help,
        Size::Small,
        res.theme.c("DefaultText"),
    );

    let hit = c.hit(
        "worldmap_cancel",
        0,
        455.0,
        by,
        70.0,
        15.0,
        "Cancels changes made to your connection",
    );
    button_box(c, res, 455.0, by, 70.0, 15.0, "Cancel", hit);
    if hit.clicked {
        if !connected {
            acts.push(Act::ClearRoute);
        }
        st.map_large = false;
        st.help.clear();
    }

    let (label, tip) = if connected {
        (
            "Disconnect",
            "Break the connection to your current remote host",
        )
    } else {
        (
            "Connect",
            "Establish the connection to your current remote host",
        )
    };
    let hit = c.hit("worldmap_connect", 1, 530.0, by, 70.0, 15.0, tip);
    button_box(c, res, 530.0, by, 70.0, 15.0, label, hit);
    if hit.clicked {
        if connected {
            acts.push(Act::Disconnect);
        } else if g.target.is_some() {
            acts.push(Act::Connect);
            st.map_large = false;
            st.help.clear();
        } else {
            st.help = "Pick a target to connect to first.".into();
        }
    }

    let hit = c.hit(
        "worldmap_close",
        0,
        605.0,
        by + 1.0,
        13.0,
        13.0,
        "Close the World Map screen",
    );
    image_button(c, res, 605.0, by + 1.0, "close.tif", hit, false);
    if hit.clicked {
        st.map_large = false;
        st.help.clear();
    }
    let _ = H;
}
