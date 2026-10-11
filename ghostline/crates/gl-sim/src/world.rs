use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use slotmap::SlotMap;

use crate::model::*;
use crate::rng::Rng;
use crate::tuning::*;

/// Bump when the serialised shape of `World` changes; `gl-save` uses it to migrate.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    Connect {
        chain: Vec<ServerId>,
        target: ServerId,
    },
    Disconnect,
    Run {
        tool: Tool,
        server: ServerId,
        file: Option<usize>,
    },
    Cancel(TaskId),
    AcceptMission(u32),
    BuyTool(Tool),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Connected { session: u64 },
    Disconnected,
    TaskStarted(TaskId),
    TaskCancelled(TaskId),
    LayerDefeated { server: ServerId, layer: usize },
    LogErased { server: ServerId },
    FileCopied { server: ServerId, name: String },
    TraceStarted,
    TraceAdvanced { hop: usize },
    TraceLost,
    Caught { fine: i64 },
    MissionAccepted(u32),
    MissionComplete { id: u32, reward: i64 },
    ToolBought { tool: Tool, level: u32 },
    GameOver,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CmdError {
    GameOver,
    AlreadyConnected,
    NotConnected,
    BadChain,
    UnknownServer,
    NotInSession,
    ToolNotOwned,
    WrongTool,
    NothingToAttack,
    AlreadyRunning,
    Locked,
    NoSuchFile,
    NoSuchLog,
    NoSuchTask,
    NoSuchMission,
    MaxLevel,
    NotEnoughCredits,
}

impl fmt::Display for CmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CmdError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub schema: u32,
    pub tick_no: u64,
    pub rng: Rng,
    pub next_session: u64,
    pub servers: SlotMap<ServerId, Server>,
    pub tasks: SlotMap<TaskId, Task>,
    pub missions: Vec<Mission>,
    pub player: Player,
    pub session: Option<Session>,
    pub trace: Option<Trace>,
    pub game_over: bool,
    #[serde(skip)]
    scratch: Vec<TaskId>,
}

impl World {
    /// A world with a player and nothing else. Used by `new` and by tests.
    pub fn empty(seed: u64) -> Self {
        let tools = [Tool::PasswordBreaker, Tool::FileCopier, Tool::LogDeleter]
            .into_iter()
            .map(|t| (t, 1))
            .collect::<BTreeMap<_, _>>();
        World {
            schema: SCHEMA_VERSION,
            tick_no: 0,
            rng: Rng::new(seed),
            next_session: 1,
            servers: SlotMap::with_key(),
            tasks: SlotMap::with_key(),
            missions: Vec::new(),
            player: Player {
                credits: START_CREDITS,
                heat: 0,
                rating: 0,
                tools,
                gateway: Gateway {
                    cpu: START_CPU,
                    bandwidth: START_BANDWIDTH,
                    files: Vec::new(),
                },
            },
            session: None,
            trace: None,
            game_over: false,
            scratch: Vec::new(),
        }
    }

    pub fn new(seed: u64) -> Self {
        let mut w = World::empty(seed);
        for i in 0..4 {
            let ip = w.random_ip();
            let trace_ticks = w.rng.range(RELAY_TRACE_MIN, RELAY_TRACE_MAX);
            w.servers.insert(Server {
                name: format!("relay-{}", i + 1),
                ip,
                kind: ServerKind::Relay,
                layers: Vec::new(),
                files: Vec::new(),
                logs: Vec::new(),
                trace_ticks,
            });
        }
        const KINDS: [LayerKind; 4] = [
            LayerKind::Password,
            LayerKind::Firewall,
            LayerKind::Proxy,
            LayerKind::Cipher,
        ];
        let mut targets = Vec::new();
        for i in 0..6 {
            let n_layers = w.rng.range(1, 4) as usize;
            let layers = (0..n_layers)
                .map(|_| Layer {
                    kind: KINDS[w.rng.index(KINDS.len())],
                    strength: w.rng.range(300, 600) * n_layers as u32,
                    defeated: false,
                })
                .collect();
            let n_files = w.rng.range(2, 4);
            let files = (0..n_files)
                .map(|f| FileEntry {
                    name: format!("record_{}_{}.dat", i + 1, f + 1),
                    size: w.rng.range(50, 400),
                    value: w.rng.range(100, 900),
                })
                .collect();
            let ip = w.random_ip();
            let id = w.servers.insert(Server {
                name: format!("corp-{}", i + 1),
                ip,
                kind: ServerKind::Corp,
                layers,
                files,
                logs: Vec::new(),
                trace_ticks: TARGET_TRACE_TICKS,
            });
            targets.push(id);
        }
        for m in 0..3u32 {
            // Never offer the same file twice. There are always at least 12 distinct (server, file) pairs.
            let (sid, file, value, name) = loop {
                let sid = targets[w.rng.index(targets.len())];
                let s = &w.servers[sid];
                let f = &s.files[w.rng.index(s.files.len())];
                let dup = w.missions.iter().any(|x| matches!(&x.kind, MissionKind::StealFile { server, file } if *server == sid && *file == f.name));
                if !dup {
                    break (sid, f.name.clone(), f.value, s.name.clone());
                }
            };
            w.missions.push(Mission {
                id: m + 1,
                title: format!("Retrieve {file} from {name}"),
                kind: MissionKind::StealFile { server: sid, file },
                reward: value as i64 * 2,
                accepted: false,
                done: false,
            });
        }
        w
    }

    fn random_ip(&mut self) -> String {
        let r = &mut self.rng;
        format!(
            "{}.{}.{}.{}",
            r.range(11, 223),
            r.range(0, 255),
            r.range(0, 255),
            r.range(1, 254)
        )
    }

    pub fn apply(&mut self, cmd: Command, out: &mut Vec<Event>) -> Result<(), CmdError> {
        if self.game_over {
            return Err(CmdError::GameOver);
        }
        match cmd {
            Command::Connect { chain, target } => self.connect(chain, target, out),
            Command::Disconnect => {
                if self.session.is_none() {
                    return Err(CmdError::NotConnected);
                }
                self.end_session();
                out.push(Event::Disconnected);
                Ok(())
            }
            Command::Run { tool, server, file } => self.run(tool, server, file, out),
            Command::Cancel(id) => {
                self.tasks.remove(id).ok_or(CmdError::NoSuchTask)?;
                out.push(Event::TaskCancelled(id));
                Ok(())
            }
            Command::AcceptMission(id) => {
                let m = self.missions.iter_mut().find(|m| m.id == id && !m.done);
                m.ok_or(CmdError::NoSuchMission)?.accepted = true;
                out.push(Event::MissionAccepted(id));
                Ok(())
            }
            Command::BuyTool(tool) => self.buy(tool, out),
        }
    }

    fn connect(
        &mut self,
        chain: Vec<ServerId>,
        target: ServerId,
        out: &mut Vec<Event>,
    ) -> Result<(), CmdError> {
        if self.session.is_some() {
            return Err(CmdError::AlreadyConnected);
        }
        let ok_chain = chain.len() <= MAX_CHAIN
            && chain.iter().enumerate().all(|(i, id)| {
                !chain[..i].contains(id)
                    && self
                        .servers
                        .get(*id)
                        .is_some_and(|s| s.kind == ServerKind::Relay)
            });
        if !ok_chain {
            return Err(CmdError::BadChain);
        }
        if !self
            .servers
            .get(target)
            .ok_or(CmdError::UnknownServer)?
            .kind
            .eq(&ServerKind::Corp)
        {
            return Err(CmdError::BadChain);
        }
        let id = self.next_session;
        self.next_session += 1;
        for sid in chain.iter().copied().chain(std::iter::once(target)) {
            self.servers[sid].logs.push(LogEntry {
                session: id,
                erased: false,
            });
        }
        self.session = Some(Session { id, chain, target });
        out.push(Event::Connected { session: id });
        Ok(())
    }

    fn run(
        &mut self,
        tool: Tool,
        server: ServerId,
        file: Option<usize>,
        out: &mut Vec<Event>,
    ) -> Result<(), CmdError> {
        let sess = self.session.as_ref().ok_or(CmdError::NotConnected)?;
        if !sess.contains(server) {
            return Err(CmdError::NotInSession);
        }
        let is_target = sess.target == server;
        let session_id = sess.id;
        if !self.player.tools.contains_key(&tool) {
            return Err(CmdError::ToolNotOwned);
        }
        let srv = &self.servers[server];
        let (kind, required) = if let Some(want) = tool.defeats() {
            if !is_target {
                return Err(CmdError::WrongTool);
            }
            let layer = srv.next_layer().ok_or(CmdError::NothingToAttack)?;
            if srv.layers[layer].kind != want {
                return Err(CmdError::WrongTool);
            }
            (TaskKind::Defeat { layer }, srv.layers[layer].strength)
        } else if tool == Tool::LogDeleter {
            if is_target && srv.locked() {
                return Err(CmdError::Locked);
            }
            if !srv
                .logs
                .iter()
                .any(|l| l.session == session_id && !l.erased)
            {
                return Err(CmdError::NoSuchLog);
            }
            (TaskKind::DeleteLog, LOG_DELETE_WORK)
        } else {
            if !is_target {
                return Err(CmdError::WrongTool);
            }
            if srv.locked() {
                return Err(CmdError::Locked);
            }
            let idx = file.ok_or(CmdError::NoSuchFile)?;
            let f = srv.files.get(idx).ok_or(CmdError::NoSuchFile)?;
            (TaskKind::Copy { file: idx }, f.size)
        };
        if self
            .tasks
            .values()
            .any(|t| t.server == server && t.kind == kind)
        {
            return Err(CmdError::AlreadyRunning);
        }
        let id = self.tasks.insert(Task {
            tool,
            server,
            kind,
            progress: 0,
            required,
        });
        out.push(Event::TaskStarted(id));
        // Hostile action on the target starts the clock.
        if is_target && self.trace.is_none() {
            self.trace = Some(Trace {
                hop: 0,
                progress: 0,
            });
            out.push(Event::TraceStarted);
        }
        Ok(())
    }

    fn buy(&mut self, tool: Tool, out: &mut Vec<Event>) -> Result<(), CmdError> {
        let level = self.player.tools.get(&tool).copied().unwrap_or(0);
        if level >= MAX_TOOL_LEVEL {
            return Err(CmdError::MaxLevel);
        }
        let price = tool.upgrade_price(level);
        if self.player.credits < price {
            return Err(CmdError::NotEnoughCredits);
        }
        self.player.credits -= price;
        self.player.tools.insert(tool, level + 1);
        out.push(Event::ToolBought {
            tool,
            level: level + 1,
        });
        Ok(())
    }

    /// Advance the simulation by one tick (100 ms of game time).
    ///
    /// Order matters: tasks resolve before the trace advances, so a log
    /// deletion finishing on the same tick as a trace hop wins for the player.
    pub fn tick(&mut self, out: &mut Vec<Event>) {
        if self.game_over {
            return;
        }
        self.tick_no += 1;
        self.advance_tasks(out);
        self.advance_trace(out);
    }

    fn advance_tasks(&mut self, out: &mut Vec<Event>) {
        let n = self.tasks.len() as u32;
        if n == 0 {
            return;
        }
        let (cpu, bw) = (self.player.gateway.cpu, self.player.gateway.bandwidth);
        let mut done = std::mem::take(&mut self.scratch);
        for (id, t) in self.tasks.iter_mut() {
            let level = self.player.tools.get(&t.tool).copied().unwrap_or(1);
            let base = if matches!(t.kind, TaskKind::Copy { .. }) {
                bw
            } else {
                cpu
            };
            t.progress += (base * level / n).max(1);
            if t.progress >= t.required {
                done.push(id);
            }
        }
        for id in done.drain(..) {
            let Some(t) = self.tasks.remove(id) else {
                continue;
            };
            match t.kind {
                TaskKind::Defeat { layer } => {
                    self.servers[t.server].layers[layer].defeated = true;
                    out.push(Event::LayerDefeated {
                        server: t.server,
                        layer,
                    });
                }
                TaskKind::DeleteLog => {
                    if let Some(sid) = self.session.as_ref().map(|s| s.id) {
                        if let Some(l) = self.servers[t.server]
                            .logs
                            .iter_mut()
                            .find(|l| l.session == sid)
                        {
                            l.erased = true;
                        }
                    }
                    out.push(Event::LogErased { server: t.server });
                }
                TaskKind::Copy { file } => {
                    let f = self.servers[t.server].files[file].clone();
                    out.push(Event::FileCopied {
                        server: t.server,
                        name: f.name.clone(),
                    });
                    self.complete_missions(t.server, &f.name, out);
                    self.player.gateway.files.push((t.server, f));
                }
            }
        }
        self.scratch = done;
    }

    fn complete_missions(&mut self, server: ServerId, name: &str, out: &mut Vec<Event>) {
        for m in self.missions.iter_mut().filter(|m| m.accepted && !m.done) {
            let MissionKind::StealFile { server: s, file } = &m.kind;
            if *s == server && file == name {
                m.done = true;
                self.player.credits += m.reward;
                self.player.rating += 1;
                out.push(Event::MissionComplete {
                    id: m.id,
                    reward: m.reward,
                });
            }
        }
    }

    fn advance_trace(&mut self, out: &mut Vec<Event>) {
        let (Some(tr), Some(sess)) = (self.trace.as_mut(), self.session.as_ref()) else {
            return;
        };
        tr.progress += 1;
        let Some(sid) = sess.hop_server(tr.hop) else {
            return;
        };
        let srv = &self.servers[sid];
        if tr.progress < srv.trace_ticks {
            return;
        }
        tr.progress = 0;
        // The tracer needs this hop's log to learn where the connection came from.
        if srv.logs.iter().any(|l| l.session == sess.id && l.erased) {
            self.trace = None;
            out.push(Event::TraceLost);
            return;
        }
        tr.hop += 1;
        if tr.hop >= sess.path_len() {
            self.caught(out);
        } else {
            out.push(Event::TraceAdvanced { hop: tr.hop });
        }
    }

    fn caught(&mut self, out: &mut Vec<Event>) {
        let fine = (self.player.credits / 4).max(MIN_FINE);
        self.player.credits = (self.player.credits - fine).max(0);
        self.player.heat += 1;
        self.end_session();
        out.push(Event::Caught { fine });
        if self.player.heat >= MAX_HEAT {
            self.game_over = true;
            out.push(Event::GameOver);
        }
    }

    /// Drops the connection. Defeated layers re-arm: you have to break in again.
    fn end_session(&mut self) {
        if let Some(s) = self.session.take() {
            for l in &mut self.servers[s.target].layers {
                l.defeated = false;
            }
        }
        self.tasks.clear();
        self.trace = None;
    }
}
