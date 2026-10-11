//! The "start menu" above the software button, from `sw_interface.cpp`.

use gl_sim::Tool;

use super::canvas::{Canvas, H};
use super::widgets::*;
use super::{Res, State};
use crate::act::Act;
use crate::game::Game;

/// (category id, caption, tooltip) in the order the original lists them, bottom to top.
const CATEGORIES: [(u32, &str, &str); 7] = [
    (
        1,
        "File Utilities",
        "Eg file copiers, deleters, encrypters etc",
    ),
    (2, "Hardware Drivers", "Eg gateway cameras"),
    (3, "Security", "Eg log deleters, Trace trackers etc"),
    (4, "Crackers", "Eg password breakers, decrypters etc"),
    (5, "Bypassers", "Eg proxy bypass, firewall bypass etc"),
    (6, "LAN tools", "Eg LAN Scan, LAN probe etc"),
    (10, "Other", "Other software programs"),
];

pub fn tool_name(t: Tool) -> &'static str {
    match t {
        Tool::PasswordBreaker => "Password Breaker",
        Tool::FirewallBypass => "Firewall Bypass",
        Tool::ProxyBypass => "Proxy Bypass",
        Tool::Decrypter => "Decrypter",
        Tool::LogDeleter => "Log Deleter",
        Tool::FileCopier => "File Copier",
    }
}

fn category_of(t: Tool) -> u32 {
    match t {
        Tool::FileCopier => 1,
        Tool::LogDeleter => 3,
        Tool::PasswordBreaker | Tool::Decrypter => 4,
        Tool::FirewallBypass | Tool::ProxyBypass => 5,
    }
}

pub fn tools_in(g: &Game, cat: u32) -> Vec<(Tool, u32)> {
    let mut v: Vec<(Tool, u32)> = g
        .world()
        .player
        .tools
        .iter()
        .filter(|(t, _)| category_of(**t) == cat)
        .map(|(t, l)| (*t, *l))
        .collect();
    v.sort_by_key(|(t, _)| tool_name(*t));
    v
}

pub fn draw(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    if !st.sw_open {
        return;
    }
    let y_pos = H - 90.0;
    let mut hovered_cat = None;
    for (i, (id, caption, tip)) in CATEGORIES.iter().enumerate() {
        let y = y_pos - (6 - i) as f32 * 17.0;
        let has = !tools_in(g, *id).is_empty();
        let hit = c.hit("hud_swmenu", *id, 3.0, y, 100.0, 15.0, tip);
        let selected = st.sw_cat == Some(*id);
        button_box_state(
            c,
            res,
            3.0,
            y,
            100.0,
            15.0,
            caption,
            hit.hover || selected,
            hit.down,
        );
        if hit.hover && has {
            hovered_cat = Some((*id, y));
        } else if hit.hover {
            st.sw_cat = None;
        }
    }
    if let Some((id, _)) = hovered_cat {
        st.sw_cat = Some(id);
    }
    if let Some(cat) = st.sw_cat {
        let base_y = y_pos
            - (6 - CATEGORIES.iter().position(|(i, ..)| *i == cat).unwrap_or(0)) as f32 * 17.0;
        for (i, (tool, level)) in tools_in(g, cat).into_iter().enumerate() {
            let (x, y) = (3.0 + 100.0 + 2.0, base_y - i as f32 * 17.0);
            let hit = c.hit(
                "hud_software",
                10 + i as u32,
                x,
                y,
                140.0,
                15.0,
                "Runs this software application",
            );
            button_box_state(c, res, x, y, 140.0, 15.0, "", hit.hover, hit.down);
            let t = &res.theme;
            let (name, ver) = (tool_name(tool), format!("v{level}.0"));
            let col = if hit.hover {
                t.c("DefaultText")
            } else {
                t.rgba("DefaultText", 0.85)
            };
            c.text(
                x + 5.0,
                y + 7.5 + 3.0,
                name,
                super::canvas::Size::Small,
                col,
            );
            c.text(
                x + 140.0 - 30.0,
                y + 7.5 + 3.0,
                &ver,
                super::canvas::Size::Small,
                col,
            );
            if hit.clicked {
                st.run_software(tool, g, acts);
                st.sw_open = false;
                st.sw_cat = None;
            }
        }
    }
}
