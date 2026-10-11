//! Headless, deterministic simulation core for Ghostline.
//!
//! No rendering, no I/O, no wall-clock, no floats, no `HashMap` iteration:
//! the same seed and command stream produce byte-identical state on native
//! and wasm, which is what makes saves portable and tests meaningful.

pub mod model;
pub mod rng;
pub mod tuning;
pub mod world;

pub use model::*;
pub use rng::Rng;
pub use world::{CmdError, Command, Event, World, SCHEMA_VERSION};
