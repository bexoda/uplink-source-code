//! The permanent HUD, positions from `hud_interface.cpp`.

use eframe::egui::Color32;

use super::canvas::{Canvas, Size, H, W};
use super::remote::tool_label_for_task;
use super::widgets::*;
use super::{Local, Res, State};
use crate::act::Act;
use crate::game::Game;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn leap(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

fn month_len(m: usize, y: u32) -> u32 {
    [
        31,
        if leap(y) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][m]
}

/// `Date::GetLongString`: "HH:MM.SS, D Month YYYY". The world starts on 24 February 2010 at 00:00.
pub fn game_date(tick: u64, ticks_per_second: u64) -> String {
    let mut secs = tick / ticks_per_second;
    let (mut day, mut month, mut year) = (24u32, 1usize, 2010u32);
    let days = secs / 86_400;
    secs %= 86_400;
    for _ in 0..days {
        day += 1;
        if day > month_len(month, year) {
            day = 1;
            month += 1;
            if month == 12 {
                month = 0;
                year += 1;
            }
        }
    }
    format!(
        "{:02}:{:02}.{:02}, {} {} {}",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60,
        day,
        MONTHS[month],
        year
    )
}

pub fn draw(c: &mut Canvas, res: &Res, st: &mut State, g: &Game, acts: &mut Vec<Act>) {
    let w = g.world();

    // Status bar: close, date, location, speed buttons.
    let hit = c.hit(
        "hud_mainmenu",
        0,
        3.0,
        4.0,
        13.0,
        13.0,
        "Close any connections and log off",
    );
    image_button(c, res, 3.0, 4.0, "close.tif", hit, false);
    if hit.clicked {
        if g.connected() {
            acts.push(Act::Disconnect);
        }
        acts.push(Act::SaveNow);
        st.say("Message", "Progress saved.");
    }

    button_box_state(
        c,
        res,
        20.0,
        3.0,
        147.0,
        15.0,
        &game_date(w.tick_no, gl_sim::tuning::TICKS_PER_SECOND as u64),
        false,
        false,
    );
    c.hit(
        "hud_date",
        0,
        20.0,
        3.0,
        147.0,
        15.0,
        "Shows the current Date and Time",
    );
    let loc = match &w.session {
        Some(s) => w.servers[s.target].ip.clone(),
        None => "127.0.0.1".to_string(),
    };
    button_box_state(c, res, 170.0, 3.0, 90.0, 15.0, &loc, false, false);
    c.hit(
        "hud_location",
        0,
        170.0,
        3.0,
        90.0,
        15.0,
        "Shows your current Uplink location",
    );

    // Speed. Faster-than-real-time play would let a run claim more game time than has passed, which the
    // server's wall-clock check rejects, so only PAUSED and NORMAL are live.
    let speeds = [
        ("Run at PAUSED speed", true),
        ("Run at NORMAL speed", true),
        (
            "Faster speeds are disabled: online runs are checked against real time",
            false,
        ),
        (
            "Faster speeds are disabled: online runs are checked against real time",
            false,
        ),
    ];
    let current = if g.paused { 0 } else { 1 };
    for (i, (tip, live)) in speeds.iter().enumerate() {
        let x = 263.0 + 17.0 * i as f32;
        let hit = c.hit("hud_speed", i as u32, x, 3.0, 15.0, 15.0, tip);
        image_button(
            c,
            res,
            x,
            3.0,
            &format!("hud/speed{i}.tif"),
            if *live { hit } else { Default::default() },
            i == current,
        );
        if !live {
            c.fill(x, 3.0, 15.0, 15.0, Color32::from_black_alpha(140));
        }
        if hit.clicked && *live && (i == 0) != g.paused {
            acts.push(Act::TogglePause);
        }
    }

    cpu_panel(c, res, g);

    // Software button.
    let hit = c.hit(
        "hud_software",
        0,
        3.0,
        H - 70.0,
        53.0,
        53.0,
        "Run a software application",
    );
    image_button(c, res, 3.0, H - 70.0, "hud/software.tif", hit, st.sw_open);
    if hit.clicked {
        st.sw_open = !st.sw_open;
        st.sw_cat = None;
        if st.sw_open {
            st.cues.push(crate::audio::Cue::SoftwareMenu);
        }
    }

    // Toolbar.
    let bar = [
        (
            "hud_hardware",
            "hud/hardware.tif",
            "Hardware",
            "Show the hardware installed in your gateway",
            60.0,
            Local::Hardware,
        ),
        (
            "hud_memory",
            "hud/memory.tif",
            "Memory",
            "Show your memory banks",
            87.0,
            Local::Memory,
        ),
        (
            "hud_status",
            "hud/status.tif",
            "Status",
            "Show your status in the World",
            114.0,
            Local::Status,
        ),
        (
            "hud_finance",
            "hud/finance.tif",
            "Finances",
            "Show your financial situation",
            141.0,
            Local::Finance,
        ),
        (
            "hud_sendmail",
            "hud/newmail.tif",
            "New Mail",
            "Send an email",
            168.0,
            Local::Mail,
        ),
        (
            "hud_analyser",
            "hud/analyser.tif",
            "Analyser",
            "Show the connection analyser",
            195.0,
            Local::Analyser,
        ),
    ];
    for (name, img, _label, tip, x, screen) in bar {
        let hit = c.hit(name, 0, x, H - 42.0, 24.0, 24.0, tip);
        image_button(c, res, x, H - 42.0, img, hit, st.local == screen);
        if hit.clicked {
            st.local = if st.local == screen {
                Local::None
            } else {
                screen
            };
        }
    }

    // One icon per accepted contract, right to left.
    let accepted: Vec<&gl_sim::Mission> = w
        .missions
        .iter()
        .filter(|m| m.accepted && !m.done)
        .collect();
    for (i, m) in accepted.iter().enumerate() {
        let x = W - 30.0 * (i as f32 + 1.0);
        let hit = c.hit("hud_mission", i as u32, x, H - 41.0, 24.0, 24.0, &m.title);
        image_button(
            c,
            res,
            x,
            H - 41.0,
            "hud/mission.tif",
            hit,
            st.local == Local::Mission(m.id),
        );
        if hit.clicked {
            st.local = if st.local == Local::Mission(m.id) {
                Local::None
            } else {
                Local::Mission(m.id)
            };
        }
    }
}

/// The "CPU usage" task manager from `vanbakel/interface.cpp`, at (335, 15): a 0..100 scale bar and one
/// bar per running program. Programs share the CPU equally here, so each bar is 100/n wide.
fn cpu_panel(c: &mut Canvas, res: &Res, g: &Game) {
    let (x, y) = (335.0, 15.0);
    let w = g.world();
    let white = Color32::from_rgba_unmultiplied(255, 255, 255, 204);
    c.fill(x, y - 15.0, 100.0, 12.0, Color32::BLACK);
    c.text(
        x + 2.0,
        y - 15.0 + 6.0 + 1.0,
        "CPU usage",
        Size::Small,
        white,
    );
    c.hit("svb_titlebar", 0, x, y - 15.0, 100.0, 12.0, "");

    let a = |r: u8, g_: u8, b: u8| Color32::from_rgba_unmultiplied(r, g_, b, 153);
    c.gradient(
        x,
        y,
        100.0,
        5.0,
        [a(0, 0, 102), a(0, 0, 179), a(0, 0, 102), a(0, 0, 179)],
    );
    c.text(
        x + 2.0,
        y + 2.5 + 1.0,
        "0...........50...........100",
        Size::Small,
        white,
    );

    let n = w.tasks.len();
    if n == 0 {
        return;
    }
    let width = (100.0 / n as f32).floor().max(1.0);
    for (i, (_, t)) in w.tasks.iter().enumerate() {
        let ry = y + 8.0 + i as f32 * 15.0;
        let hit = c.hit(
            "svb_priority",
            i as u32,
            x,
            ry,
            width,
            10.0,
            "The share of your CPU this program is using",
        );
        let scale = width / 50.0 * if hit.hover { 2.0 } else { 1.0 };
        let col = Color32::from_rgba_unmultiplied(
            0,
            ((2.0 - scale).clamp(0.0, 1.0) * 255.0) as u8,
            (scale.clamp(0.0, 1.0) * 255.0) as u8,
            153,
        );
        c.fill(x, ry, width, 10.0, col);
        c.text(
            x + 5.0,
            ry + 5.0 + 2.0,
            &tool_label_for_task(t),
            Size::Small,
            Color32::WHITE,
        );
    }
    let _ = (res, H, W);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_follow_the_original_format_and_roll_over() {
        assert_eq!(game_date(0, 10), "00:00.00, 24 February 2010");
        assert_eq!(game_date(10 * 3661, 10), "01:01.01, 24 February 2010");
        assert_eq!(game_date(10 * 86_400 * 5, 10), "00:00.00, 1 March 2010");
        assert_eq!(
            game_date(
                10 * 86_400 * (5 + 31 + 30 + 31 + 30 + 31 + 31 + 30 + 31 + 30 + 31 + 31),
                10
            )
            .split(", ")
            .nth(1)
            .unwrap(),
            "1 February 2011"
        );
    }
}
