//! Game controller: wraps a recorded `Run` with the state a UI needs (selection, pacing, log).
//! No egui here, so it is unit-testable and the rules stay in `gl-sim`.

use std::cell::RefCell;
use std::collections::VecDeque;

use gl_sim::replay::Run;
use gl_sim::tuning::TICKS_PER_SECOND;
use gl_sim::*;

pub const TICK_SECS: f32 = 1.0 / TICKS_PER_SECOND as f32;
const MAX_LOG: usize = 300;
/// Never simulate more than this much real time per frame (tab was backgrounded, debugger, etc.).
const MAX_FRAME_SECS: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Good,
    Warn,
    Bad,
}

#[derive(Clone, Debug)]
pub struct LogLine {
    pub tick: u64,
    pub level: Level,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HopState {
    /// The tracer has already passed this hop.
    Traced,
    /// The tracer is working on this hop right now.
    Active,
    Pending,
}

#[derive(Clone, Debug)]
pub struct HopView {
    pub name: String,
    pub state: HopState,
    /// 0..=1 progress of the tracer on this hop.
    pub progress: f32,
    pub log_erased: bool,
}

pub struct Game {
    pub run: Run,
    pub log: VecDeque<LogLine>,
    pub paused: bool,
    /// Relays chosen for the next connection, in order from the player's side.
    pub chain: Vec<ServerId>,
    pub target: Option<ServerId>,
    /// Seeds not issued by the server can't be verified, so such runs never count online.
    pub verified: bool,
    pub dirty: bool,
    accum: f32,
    scratch: Vec<Event>,
    /// Why the last command was refused, for a UI that shows message boxes. Taken by the UI.
    refusal: RefCell<Option<String>>,
    /// Big moments (traced, contract done, game over) for a UI that shows message boxes.
    notices: RefCell<VecDeque<String>>,
}

impl Game {
    pub fn new(seed: u64) -> Game {
        let mut g = Game::from_run(Run::new(seed), false);
        g.say(
            Level::Info,
            "Gateway online. Pick a target, build a relay chain, then connect.",
        );
        g
    }

    pub fn from_run(run: Run, verified: bool) -> Game {
        Game {
            run,
            log: VecDeque::new(),
            paused: false,
            chain: Vec::new(),
            target: None,
            verified,
            dirty: false,
            accum: 0.0,
            scratch: Vec::new(),
            refusal: RefCell::new(None),
            notices: RefCell::new(VecDeque::new()),
        }
    }

    pub fn take_refusal(&self) -> Option<String> {
        self.refusal.borrow_mut().take()
    }

    pub fn take_notice(&self) -> Option<String> {
        self.notices.borrow_mut().pop_front()
    }

    fn notify(&self, text: &str) {
        let mut n = self.notices.borrow_mut();
        n.push_back(text.to_string());
        while n.len() > 20 {
            n.pop_front();
        }
    }

    pub fn world(&self) -> &World {
        &self.run.world
    }

    fn say(&mut self, level: Level, text: impl Into<String>) {
        let tick = self.run.world.tick_no;
        self.log.push_back(LogLine {
            tick,
            level,
            text: text.into(),
        });
        while self.log.len() > MAX_LOG {
            self.log.pop_front();
        }
    }

    pub fn name_of(&self, id: ServerId) -> String {
        self.world()
            .servers
            .get(id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "?".into())
    }

    /// Advance by real elapsed time. Fixed 100 ms ticks; leftovers carry to the next frame.
    /// Returns how many ticks ran.
    pub fn advance(&mut self, dt: f32) -> u32 {
        if self.paused || self.world().game_over {
            self.accum = 0.0;
            return 0;
        }
        self.accum += dt.clamp(0.0, MAX_FRAME_SECS);
        let mut ticks = 0;
        while self.accum >= TICK_SECS {
            self.accum -= TICK_SECS;
            let mut ev = std::mem::take(&mut self.scratch);
            ev.clear();
            self.run.tick(&mut ev);
            for e in &ev {
                self.narrate(e);
            }
            self.scratch = ev;
            ticks += 1;
        }
        if ticks > 0 {
            self.dirty = true;
        }
        ticks
    }

    pub fn exec(&mut self, cmd: Command) -> bool {
        let mut ev = Vec::new();
        match self.run.apply(cmd, &mut ev) {
            Ok(()) => {
                for e in &ev {
                    self.narrate(e);
                }
                self.dirty = true;
                true
            }
            Err(e) => {
                *self.refusal.borrow_mut() = Some(format!("Refused: {}.", explain(e)));
                self.say(Level::Warn, format!("Refused: {}", explain(e)));
                false
            }
        }
    }

    fn narrate(&mut self, e: &Event) {
        let (level, text) = match e {
            Event::Connected { .. } => {
                let t = self.session_summary();
                (Level::Info, format!("Connected: {t}"))
            }
            Event::Disconnected => (
                Level::Info,
                "Disconnected. Defeated defences re-arm.".into(),
            ),
            Event::TaskStarted(_) => return,
            Event::TaskCancelled(_) => (Level::Info, "Task cancelled.".into()),
            Event::LayerDefeated { server, layer } => {
                let kind = self
                    .world()
                    .servers
                    .get(*server)
                    .and_then(|s| s.layers.get(*layer))
                    .map(|l| l.kind);
                (
                    Level::Good,
                    format!(
                        "{}: {:?} layer defeated.",
                        self.name_of(*server),
                        kind.unwrap_or(LayerKind::Password)
                    ),
                )
            }
            Event::LogErased { server } => (
                Level::Good,
                format!("{}: your log entry is gone.", self.name_of(*server)),
            ),
            Event::FileCopied { server, name } => (
                Level::Good,
                format!("{}: copied {name}.", self.name_of(*server)),
            ),
            Event::TraceStarted => (
                Level::Warn,
                "TRACE STARTED. They know something is wrong.".into(),
            ),
            Event::TraceAdvanced { hop } => {
                let who = self
                    .world()
                    .session
                    .as_ref()
                    .and_then(|s| s.hop_server(*hop - 1))
                    .map(|id| self.name_of(id))
                    .unwrap_or_default();
                (
                    Level::Warn,
                    format!("Trace cleared {who}. It is closing in."),
                )
            }
            Event::TraceLost => (Level::Good, "Trace lost: it hit a hop with no log.".into()),
            Event::Caught { fine } => {
                self.notify(&format!(
                    "You were traced. Your connection was cut and you were fined {fine} credits."
                ));
                (Level::Bad, format!("TRACED. Fined {fine}. Connection cut."))
            }
            Event::MissionAccepted(id) => (Level::Info, format!("Mission {id} accepted.")),
            Event::MissionComplete { id, reward } => {
                self.notify(&format!(
                    "Contract {id} complete. {reward} credits have been paid into your account."
                ));
                (Level::Good, format!("Mission {id} complete: +{reward}."))
            }
            Event::ToolBought { tool, level } => {
                (Level::Info, format!("{tool:?} is now level {level}."))
            }
            Event::GameOver => {
                self.notify("Three strikes. Your career is over.");
                (Level::Bad, "Three strikes. Your career is over.".into())
            }
        };
        self.say(level, text);
    }

    fn session_summary(&self) -> String {
        let Some(s) = &self.world().session else {
            return String::new();
        };
        let mut names: Vec<String> = s.chain.iter().map(|id| self.name_of(*id)).collect();
        names.push(self.name_of(s.target));
        format!("gateway > {}", names.join(" > "))
    }

    // ----- selection -----

    pub fn connected(&self) -> bool {
        self.world().session.is_some()
    }

    pub fn toggle_relay(&mut self, id: ServerId) {
        if self.connected() {
            return;
        }
        if let Some(i) = self.chain.iter().position(|r| *r == id) {
            self.chain.remove(i);
        } else if self.chain.len() < gl_sim::tuning::MAX_CHAIN {
            self.chain.push(id);
        }
    }

    pub fn select_target(&mut self, id: ServerId) {
        if !self.connected() {
            self.target = Some(id);
        }
    }

    pub fn toggle_target(&mut self, id: ServerId) {
        if !self.connected() {
            self.target = if self.target == Some(id) {
                None
            } else {
                Some(id)
            };
        }
    }

    pub fn clear_route(&mut self) {
        if !self.connected() {
            self.chain.clear();
            self.target = None;
        }
    }

    pub fn connect(&mut self) {
        match self.target {
            Some(target) => {
                let chain = self.chain.clone();
                self.exec(Command::Connect { chain, target });
            }
            None => self.say(Level::Warn, "Pick a target first."),
        }
    }

    pub fn disconnect(&mut self) {
        self.exec(Command::Disconnect);
    }

    // ----- views -----

    pub fn relays(&self) -> Vec<ServerId> {
        self.ids_of(ServerKind::Relay)
    }

    pub fn targets(&self) -> Vec<ServerId> {
        self.ids_of(ServerKind::Corp)
    }

    /// Uplink's own services machine(s).
    pub fn services(&self) -> Vec<ServerId> {
        self.ids_of(ServerKind::Service)
    }

    fn ids_of(&self, kind: ServerKind) -> Vec<ServerId> {
        let mut v: Vec<_> = self
            .world()
            .servers
            .iter()
            .filter(|(_, s)| s.kind == kind)
            .map(|(i, s)| (s.name.clone(), i))
            .collect();
        v.sort();
        v.into_iter().map(|(_, i)| i).collect()
    }

    pub fn task_running(&self, server: ServerId, kind: TaskKind) -> bool {
        self.world()
            .tasks
            .values()
            .any(|t| t.server == server && t.kind == kind)
    }

    pub fn owns(&self, tool: Tool) -> bool {
        self.world().player.tools.contains_key(&tool)
    }

    /// The tracer's position along the connection, in display order (player side first).
    pub fn trace_view(&self) -> Option<Vec<HopView>> {
        let w = self.world();
        let (s, tr) = (w.session.as_ref()?, w.trace.as_ref());
        let n = s.path_len();
        let mut out = Vec::with_capacity(n);
        for pos in 0..n {
            let hop = n - 1 - pos; // hop 0 is the target, the tracer's starting point
            let id = s.hop_server(hop)?;
            let srv = w.servers.get(id)?;
            let (state, progress) = match tr {
                Some(t) if t.hop > hop => (HopState::Traced, 1.0),
                Some(t) if t.hop == hop => (
                    HopState::Active,
                    t.progress as f32 / srv.trace_ticks.max(1) as f32,
                ),
                _ => (HopState::Pending, 0.0),
            };
            out.push(HopView {
                name: srv.name.clone(),
                state,
                progress: progress.min(1.0),
                log_erased: srv.logs.iter().any(|l| l.session == s.id && l.erased),
            });
        }
        Some(out)
    }

    pub fn clock(&self) -> String {
        let secs = self.world().tick_no / TICKS_PER_SECOND as u64;
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

pub fn explain(e: CmdError) -> &'static str {
    match e {
        CmdError::GameOver => "the run is over",
        CmdError::AlreadyConnected => "already connected",
        CmdError::NotConnected => "not connected",
        CmdError::BadChain => "invalid relay chain",
        CmdError::UnknownServer => "unknown server",
        CmdError::NotInSession => "that server is not on your connection",
        CmdError::ToolNotOwned => "you do not own that tool",
        CmdError::WrongTool => "wrong tool for this defence",
        CmdError::NothingToAttack => "nothing left to attack",
        CmdError::AlreadyRunning => "that task is already running",
        CmdError::Locked => "defences are still up",
        CmdError::NoSuchFile => "no such file",
        CmdError::NoSuchLog => "no log entry to erase",
        CmdError::NoSuchTask => "no such task",
        CmdError::NoSuchMission => "no such mission",
        CmdError::MaxLevel => "already at the maximum level",
        CmdError::NotEnoughCredits => "not enough credits",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn weak_corp(g: &mut Game) -> ServerId {
        g.run.world.servers.insert(Server {
            name: "corp-test".into(),
            ip: "10.0.0.9".into(),
            kind: ServerKind::Corp,
            layers: vec![Layer {
                kind: LayerKind::Password,
                strength: 40,
                defeated: false,
            }],
            files: vec![FileEntry {
                name: "x.dat".into(),
                size: 50,
                value: 10,
            }],
            logs: vec![],
            trace_ticks: 500,
        })
    }

    #[test]
    fn pacing_is_fixed_step_and_pausable() {
        let mut g = Game::new(1);
        assert_eq!(g.advance(0.05), 0, "less than a tick carries over");
        assert_eq!(g.advance(0.05), 1);
        assert_eq!(
            g.advance(1.0),
            5,
            "one frame never runs more than half a second of ticks"
        );
        assert_eq!(g.world().tick_no, 6);
        g.paused = true;
        assert_eq!(g.advance(1.0), 0);
        g.paused = false;
        assert_eq!(g.advance(0.1), 1, "pausing doesn't bank time");
    }

    #[test]
    fn relay_chain_selection_rules() {
        let mut g = Game::new(1);
        let r = g.relays();
        g.toggle_relay(r[1]);
        g.toggle_relay(r[0]);
        assert_eq!(g.chain, vec![r[1], r[0]], "order is the order you clicked");
        g.toggle_relay(r[1]);
        assert_eq!(g.chain, vec![r[0]]);
        g.connect();
        assert!(!g.connected(), "no target yet");
        assert_eq!(g.log.back().unwrap().level, Level::Warn);
    }

    #[test]
    fn full_attack_through_the_controller_is_recorded() {
        let mut g = Game::new(3);
        let t = weak_corp(&mut g);
        let r = g.relays()[0];
        g.toggle_relay(r);
        g.select_target(t);
        g.connect();
        assert!(g.connected());
        // Selection is frozen while connected.
        g.select_target(g.targets()[0]);
        assert_eq!(g.target, Some(t));

        assert!(g.exec(Command::Run {
            tool: Tool::PasswordBreaker,
            server: t,
            file: None
        }));
        assert!(g
            .trace_view()
            .unwrap()
            .iter()
            .any(|h| h.state == HopState::Active));
        for _ in 0..200 {
            g.advance(0.1);
        }
        assert!(!g.world().servers[t].locked());
        assert!(g.exec(Command::Run {
            tool: Tool::FileCopier,
            server: t,
            file: Some(0)
        }));
        for _ in 0..200 {
            g.advance(0.1);
        }
        assert_eq!(g.world().player.gateway.files.len(), 1);
        assert!(
            g.run.unsynced() >= 3,
            "connect + attack + copy are all in the proof log"
        );
        assert!(g
            .log
            .iter()
            .any(|l| l.level == Level::Good && l.text.contains("copied")));
    }

    #[test]
    fn refused_commands_explain_themselves_and_are_not_logged_for_replay() {
        let mut g = Game::new(1);
        let before = g.run.unsynced();
        assert!(!g.exec(Command::Disconnect));
        assert!(g.log.back().unwrap().text.contains("not connected"));
        assert_eq!(g.run.unsynced(), before);
    }

    #[test]
    fn trace_view_orders_hops_player_side_first() {
        let mut g = Game::new(1);
        let t = weak_corp(&mut g);
        let (r1, r2) = (g.relays()[0], g.relays()[1]);
        g.toggle_relay(r1);
        g.toggle_relay(r2);
        g.select_target(t);
        g.connect();
        let names: Vec<_> = g
            .trace_view()
            .unwrap()
            .into_iter()
            .map(|h| h.name)
            .collect();
        assert_eq!(
            names,
            vec![g.name_of(r1), g.name_of(r2), "corp-test".to_string()]
        );
    }

    #[test]
    fn game_over_stops_the_clock() {
        let mut g = Game::new(1);
        g.run.world.game_over = true;
        assert_eq!(g.advance(1.0), 0);
    }
}
