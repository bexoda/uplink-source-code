//! Drives the Uplink interface headlessly with synthesised mouse clicks, against the real game data.
//! Covers hit-testing, screen flow and the commands the UI emits. Skipped if no Uplink data is found
//! (set UPLINK_DIR, or run from this repository where the developer CD sits at `uplink/bin`).
//!
//! Click coordinates are the machines' fixed spots on the 1024x768 world map (land placement is a
//! pure function of the machine names), so a change to node placement means updating them.

use std::path::PathBuf;

use eframe::egui::{self, vec2, Event, Modifiers, PointerButton, Pos2, RawInput, Rect};
use gl_assets::Store;
use gl_client::audio::Cue;
use gl_client::game::Game;
use gl_client::{Act, UplinkUi};
use gl_sim::{Command, Tool};

fn data_dir() -> Option<PathBuf> {
    let d = std::env::var_os("UPLINK_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../uplink/bin"));
    if d.join("graphics.dat").is_file() {
        Some(d)
    } else {
        eprintln!("SKIPPED: no Uplink data found (set UPLINK_DIR)");
        None
    }
}

struct Rig {
    ctx: egui::Context,
    ui: UplinkUi,
    game: Game,
    t: f64,
    /// Every sound the interface and the game asked for, in order.
    cues: Vec<Cue>,
}

impl Rig {
    fn new() -> Option<Rig> {
        let dir = data_dir()?;
        let ctx = egui::Context::default();
        let ui = UplinkUi::new(&ctx, Store::from_dir(dir).unwrap())
            .expect("the UI builds from the real data");
        let mut rig = Rig {
            ctx,
            ui,
            game: Game::new(42),
            t: 0.0,
            cues: Vec::new(),
        };
        for _ in 0..3 {
            rig.frame(vec![]);
        }
        Some(rig)
    }

    /// One frame at 10 fps of UI time; returns the actions the UI produced and applies them to the game.
    fn frame(&mut self, events: Vec<Event>) -> Vec<String> {
        self.t += 0.1;
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1024.0, 768.0))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        raw.viewport_id = egui::ViewportId::ROOT;
        let mut acts: Vec<Act> = Vec::new();
        let (ui, game) = (&mut self.ui, &self.game);
        let out = self.ctx.run_ui(raw, |u| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(u, |u| ui.show(u, game, 0.1, &mut acts));
        });
        out.drop_without_applying_deltas();
        let names: Vec<String> = acts.iter().map(|a| format!("{a:?}")).collect();
        for a in acts {
            self.game.apply_act(a);
        }
        self.cues.extend(self.ui.take_cues());
        self.cues.extend(self.game.take_cues());
        names
    }

    fn hover(&mut self, x: f32, y: f32) {
        self.frame(vec![Event::PointerMoved(Pos2::new(x, y))]);
        self.frame(vec![]);
    }

    /// Move, press, release; returns the actions emitted by the release frame.
    fn click(&mut self, x: f32, y: f32) -> Vec<String> {
        let p = Pos2::new(x, y);
        self.hover(x, y);
        self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        let acts = self.frame(vec![Event::PointerButton {
            pos: p,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![]);
        acts
    }

    /// Let game time pass (the UI keeps animating).
    fn wait(&mut self, seconds: f32) {
        for _ in 0..(seconds * 10.0) as u32 {
            self.game.advance(0.1);
            self.frame(vec![]);
        }
    }
}

#[test]
fn a_whole_hack_through_the_original_interface() {
    let Some(mut r) = Rig::new() else { return };
    assert_eq!(r.game.world().player.credits, 1000);

    // Open the world map, pick Uplink's services machine and connect to it.
    r.click(870.0, 80.0);
    assert!(r
        .click(902.0, 401.0)
        .iter()
        .any(|a| a.contains("ToggleTarget")));
    assert!(r.click(565.0, 581.0).iter().any(|a| a.contains("Connect")));
    assert!(r.game.connected(), "the connect button connects");

    // Buy the Firewall Bypass in the software shop.
    r.click(200.0, 140.0); // "Software Sales"
    r.click(100.0, 98.0); // the Firewall Bypass row
    let bought = r.click(620.0, 187.0); // Purchase
    assert!(bought.iter().any(|a| a.contains("BuyTool")), "{bought:?}");
    assert!(r.game.owns(Tool::FirewallBypass));
    assert_eq!(r.game.world().player.credits, 400);
    // Buying the next level costs more than is left: refused, with failure.wav.
    r.cues.clear();
    r.click(620.0, 187.0);
    assert_eq!(r.game.world().player.credits, 400);
    assert!(r.cues.contains(&Cue::Failure), "{:?}", r.cues);
    r.click(512.0, 373.0); // the refusal's message box: Close

    // Back out, route through two relays to corp-1 and connect.
    r.click(873.0, 171.0);
    assert!(!r.game.connected());
    r.click(870.0, 80.0);
    r.cues.clear();
    r.click(329.0, 360.0); // relay-1
    r.click(336.0, 482.0); // relay-2
    r.click(363.0, 405.0); // corp-1
    assert_eq!(r.game.chain.len(), 2);
    // addlink.wav for each bounce added (not for the target), mouseclick.wav for each press.
    assert_eq!(
        r.cues.iter().filter(|c| **c == Cue::AddLink).count(),
        2,
        "{:?}",
        r.cues
    );
    assert_eq!(r.cues.iter().filter(|c| **c == Cue::MouseClick).count(), 3);
    r.click(565.0, 581.0);
    assert!(r.game.connected());
    let target = r.game.world().session.as_ref().unwrap().target;

    // Open the firewall system, run the bypass from the software menu.
    r.click(150.0, 152.0);
    r.cues.clear();
    r.click(30.0, 742.0); // software button
    r.hover(50.0, 650.0); // "Bypassers"
    let ran = r.click(170.0, 651.0);
    let menu = r.cues.iter().filter(|c| **c == Cue::SoftwareMenu).count();
    assert_eq!(menu, 2, "menu and submenu: {:?}", r.cues);
    assert!(r.cues.contains(&Cue::RunSoftware));
    assert!(ran.iter().any(|a| a.contains("FirewallBypass")), "{ran:?}");
    assert_eq!(r.game.world().tasks.len(), 1);

    r.wait(9.0);
    assert!(
        !r.game.world().servers[target].locked(),
        "the bypass finished"
    );

    // Proceed to the server menu, open the file server, arm the file copier, click a file.
    r.wait(2.0); // the screen returns to the systems list by itself
    r.cues.clear();
    r.click(375.0, 357.0); // Proceed
    assert!(r.cues.contains(&Cue::Login), "{:?}", r.cues);
    r.wait(1.5);
    r.click(200.0, 135.0); // File Server
    r.click(30.0, 742.0);
    r.hover(50.0, 583.0); // "File Utilities"
    r.click(170.0, 583.0); // File Copier
    let copy = r.click(100.0, 147.0); // first file
    assert!(copy.iter().any(|a| a.contains("FileCopier")), "{copy:?}");
    assert!(r
        .game
        .world()
        .tasks
        .values()
        .any(|t| matches!(t.kind, gl_sim::TaskKind::Copy { .. })));
    // The command was recorded for the server to verify.
    assert!(r.game.run.unsynced() >= 5);
    let _ = Command::Disconnect;
}

#[test]
fn clicking_the_tool_before_connecting_explains_instead_of_failing() {
    let Some(mut r) = Rig::new() else { return };
    r.click(30.0, 742.0);
    r.hover(50.0, 634.0); // Crackers
    let acts = r.click(170.0, 634.0); // Password Breaker
    assert!(
        acts.is_empty(),
        "nothing is sent to the sim without a connection: {acts:?}"
    );
    assert!(!r.game.connected());
}

#[test]
fn every_toolbar_panel_draws_without_a_connection() {
    let Some(mut r) = Rig::new() else { return };
    for x in [72.0, 99.0, 126.0, 153.0, 180.0, 207.0] {
        r.click(x, 738.0);
        r.frame(vec![]);
        r.click(x, 738.0); // toggles it closed again
    }
}
