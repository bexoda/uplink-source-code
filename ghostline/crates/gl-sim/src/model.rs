use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use slotmap::new_key_type;

new_key_type! {
    pub struct ServerId;
    pub struct TaskId;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LayerKind {
    Password,
    Firewall,
    Proxy,
    Cipher,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tool {
    PasswordBreaker,
    FirewallBypass,
    ProxyBypass,
    Decrypter,
    LogDeleter,
    FileCopier,
}

impl Tool {
    /// The security layer this tool defeats, if it is an intrusion tool.
    pub fn defeats(self) -> Option<LayerKind> {
        match self {
            Tool::PasswordBreaker => Some(LayerKind::Password),
            Tool::FirewallBypass => Some(LayerKind::Firewall),
            Tool::ProxyBypass => Some(LayerKind::Proxy),
            Tool::Decrypter => Some(LayerKind::Cipher),
            Tool::LogDeleter | Tool::FileCopier => None,
        }
    }

    /// Cost to go from `current_level` (0 = not owned) to the next level.
    pub fn upgrade_price(self, current_level: u32) -> i64 {
        self.base_price() * (current_level as i64 + 1)
    }

    pub fn base_price(self) -> i64 {
        match self {
            Tool::PasswordBreaker => 300,
            Tool::FirewallBypass => 600,
            Tool::ProxyBypass => 600,
            Tool::Decrypter => 900,
            Tool::LogDeleter => 400,
            Tool::FileCopier => 200,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerKind {
    /// Anonymous relay the player can bounce through. No security.
    Relay,
    /// Hardened target.
    Corp,
    /// Uplink's own services machine: sells software and posts contracts. No defences.
    Service,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Layer {
    pub kind: LayerKind,
    /// Work units required to defeat at tool level 1.
    pub strength: u32,
    pub defeated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    /// Also the work required to copy it.
    pub size: u32,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogEntry {
    pub session: u64,
    pub erased: bool,
    /// World tick at which the connection was made, for display.
    pub tick: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Server {
    pub name: String,
    pub ip: String,
    pub kind: ServerKind,
    pub layers: Vec<Layer>,
    pub files: Vec<FileEntry>,
    pub logs: Vec<LogEntry>,
    /// Ticks a trace spends on this hop before moving to the next.
    pub trace_ticks: u32,
}

impl Server {
    pub fn locked(&self) -> bool {
        self.layers.iter().any(|l| !l.defeated)
    }

    /// Layers fall in order; only the first undefeated one is attackable.
    pub fn next_layer(&self) -> Option<usize> {
        self.layers.iter().position(|l| !l.defeated)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gateway {
    /// Work per tick for intrusion tools and log deletion.
    pub cpu: u32,
    /// Work per tick for file copying.
    pub bandwidth: u32,
    pub files: Vec<(ServerId, FileEntry)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Player {
    pub credits: i64,
    pub heat: u8,
    pub rating: u32,
    /// Owned tools and their level (1..=MAX_TOOL_LEVEL).
    pub tools: BTreeMap<Tool, u32>,
    pub gateway: Gateway,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: u64,
    /// Relays in connection order, player-side first. Excludes the target.
    pub chain: Vec<ServerId>,
    pub target: ServerId,
}

impl Session {
    pub fn contains(&self, id: ServerId) -> bool {
        self.target == id || self.chain.contains(&id)
    }

    /// Server `hop` steps back from the target (0 = target).
    pub fn hop_server(&self, hop: usize) -> Option<ServerId> {
        if hop == 0 {
            Some(self.target)
        } else {
            self.chain.len().checked_sub(hop).map(|i| self.chain[i])
        }
    }

    pub fn path_len(&self) -> usize {
        self.chain.len() + 1
    }
}

/// A trace walking back from the target toward the player's gateway.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub hop: usize,
    pub progress: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Defeat { layer: usize },
    DeleteLog,
    Copy { file: usize },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub tool: Tool,
    pub server: ServerId,
    pub kind: TaskKind,
    pub progress: u32,
    pub required: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionKind {
    StealFile { server: ServerId, file: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mission {
    pub id: u32,
    pub title: String,
    pub kind: MissionKind,
    pub reward: i64,
    pub accepted: bool,
    pub done: bool,
}
