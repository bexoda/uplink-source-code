//! Balance knobs. Everything is integer "work units"; one tick = 100 ms of game time.

pub const TICKS_PER_SECOND: u32 = 10;
pub const MAX_CHAIN: usize = 8;
pub const MAX_TOOL_LEVEL: u32 = 5;
pub const MAX_HEAT: u8 = 3;

pub const START_CREDITS: i64 = 1_000;
pub const START_CPU: u32 = 8;
pub const START_BANDWIDTH: u32 = 10;

pub const LOG_DELETE_WORK: u32 = 300;
pub const MIN_FINE: i64 = 100;

pub const TARGET_TRACE_TICKS: u32 = 150;
pub const RELAY_TRACE_MIN: u32 = 30;
pub const RELAY_TRACE_MAX: u32 = 60;
