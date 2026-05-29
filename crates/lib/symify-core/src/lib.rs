//! symify core: config loading, planning, and filesystem execution.
//!
//! See `specs/ARCHITECTURE.md` for the design. The crate is layered:
//! [`config`] loads and merges TOML into a resolved model, [`plan`] turns that
//! plus current filesystem state into a pure list of actions, and the executor
//! applies them. The planner never mutates the filesystem.

pub mod clock;
pub mod config;
pub mod error;
pub mod fs;
pub mod model;
pub mod plan;
pub mod status;

pub use error::{Error, Result};
pub use plan::{Action, ActionKind, FsOp, Outcome, Planned, Verb, execute, plan};
pub use status::{StatusEntry, StatusLabel, status};
