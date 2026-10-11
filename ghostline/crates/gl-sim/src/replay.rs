//! Run recording and replay. This is the anti-cheat core.
//!
//! Clients never upload state. They upload the *commands* that produced it,
//! and the server replays them on its own copy of the world with this same
//! crate. Credits can only be earned through events the sim itself emits, so
//! an edited number in client memory simply never reaches the server.

use serde::{Deserialize, Serialize};

use crate::world::{CmdError, Command, Event, World};

/// Version of the *rules*. Bump on any change that alters simulation outcomes
/// (balance, mechanics), since old logs would no longer replay identically.
pub const SIM_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEntry {
    /// `World::tick_no` when the command was applied.
    pub tick: u64,
    pub cmd: Command,
}

/// A contiguous slice of a run, starting from a state the server already holds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub sim_version: u32,
    pub start_tick: u64,
    pub end_tick: u64,
    pub entries: Vec<CommandEntry>,
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_ticks: u64,
    pub max_entries: usize,
}

impl Default for Limits {
    fn default() -> Self {
        // ~14 hours of game time and 50k commands per segment.
        Limits {
            max_ticks: 500_000,
            max_entries: 50_000,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayError {
    SimVersion {
        found: u32,
    },
    StartTick {
        expected: u64,
        found: u64,
    },
    TooLong,
    TooManyEntries,
    Unordered {
        index: usize,
    },
    OutOfRange {
        index: usize,
    },
    /// A recorded command is not legal in the state it claims to follow.
    Rejected {
        index: usize,
        error: CmdError,
    },
}

/// Replays `seg` onto `world`, which must be exactly at `seg.start_tick`.
/// On error `world` is left in an unspecified state and must be discarded.
pub fn replay(world: &mut World, seg: &Segment, limits: Limits) -> Result<(), ReplayError> {
    if seg.sim_version != SIM_VERSION {
        return Err(ReplayError::SimVersion {
            found: seg.sim_version,
        });
    }
    if world.tick_no != seg.start_tick {
        return Err(ReplayError::StartTick {
            expected: world.tick_no,
            found: seg.start_tick,
        });
    }
    if seg.end_tick < seg.start_tick || seg.end_tick - seg.start_tick > limits.max_ticks {
        return Err(ReplayError::TooLong);
    }
    if seg.entries.len() > limits.max_entries {
        return Err(ReplayError::TooManyEntries);
    }
    let mut sink: Vec<Event> = Vec::new();
    let mut last = seg.start_tick;
    for (index, e) in seg.entries.iter().enumerate() {
        if e.tick < last {
            return Err(ReplayError::Unordered { index });
        }
        if e.tick > seg.end_tick {
            return Err(ReplayError::OutOfRange { index });
        }
        last = e.tick;
        while world.tick_no < e.tick && !world.game_over {
            sink.clear();
            world.tick(&mut sink);
        }
        sink.clear();
        world
            .apply(e.cmd.clone(), &mut sink)
            .map_err(|error| ReplayError::Rejected { index, error })?;
    }
    while world.tick_no < seg.end_tick && !world.game_over {
        sink.clear();
        world.tick(&mut sink);
    }
    Ok(())
}

/// A live game plus the log needed to prove it. Only successful commands are
/// recorded: failed ones never mutate state, so replay doesn't need them.
pub struct Run {
    pub world: World,
    start_tick: u64,
    log: Vec<CommandEntry>,
}

impl Run {
    pub fn new(seed: u64) -> Run {
        Run {
            world: World::new(seed),
            start_tick: 0,
            log: Vec::new(),
        }
    }

    /// Continue from a verified state fetched from the server.
    pub fn resume(world: World) -> Run {
        let start_tick = world.tick_no;
        Run {
            world,
            start_tick,
            log: Vec::new(),
        }
    }

    pub fn apply(&mut self, cmd: Command, out: &mut Vec<Event>) -> Result<(), CmdError> {
        let r = self.world.apply(cmd.clone(), out);
        if r.is_ok() {
            self.log.push(CommandEntry {
                tick: self.world.tick_no,
                cmd,
            });
        }
        r
    }

    pub fn tick(&mut self, out: &mut Vec<Event>) {
        self.world.tick(out);
    }

    /// Everything since the last acknowledged sync.
    pub fn segment(&self) -> Segment {
        Segment {
            sim_version: SIM_VERSION,
            start_tick: self.start_tick,
            end_tick: self.world.tick_no,
            entries: self.log.clone(),
        }
    }

    /// Call after the server accepted `seg`. Safe if play continued while uploading.
    pub fn acknowledge(&mut self, seg: &Segment) {
        let n = seg.entries.len().min(self.log.len());
        self.log.drain(..n);
        self.start_tick = seg.end_tick;
    }
}
