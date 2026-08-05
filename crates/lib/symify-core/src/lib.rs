//! symify core: config loading, planning, and filesystem execution.
//!
//! See `knowledge/architecture.md` for the design. The crate is layered:
//! [`config`] loads and merges TOML into a resolved model, [`mod@plan`] turns
//! that plus current filesystem state into a pure list of actions, and the
//! executor applies them. The planner never mutates the filesystem.
//!
//! # Example
//!
//! Load a config, plan a `sync`, and execute it as a dry run (no writes):
//!
//! ```
//! use std::fs;
//! use symify_core::{config, plan, RunOptions, Verb};
//! use symify_core::clock::SystemClock;
//!
//! let dir = tempfile::tempdir()?;
//! let live = dir.path().join("live");
//! let store = dir.path().join("store");
//! fs::create_dir_all(&live)?;
//! fs::create_dir_all(&store)?;
//! fs::write(live.join(".bashrc"), b"export EDITOR=vim\n")?;
//!
//! let cfg = dir.path().join("symify.toml");
//! fs::write(&cfg, format!(
//!     "[settings]\n\
//!      live = \"{}\"\n\
//!      store = \"{}\"\n\n\
//!      [mappings.dotfiles.links]\n\
//!      \".bashrc\" = true\n",
//!     live.display(),
//!     store.display(),
//! ))?;
//!
//! let machine = config::MachineContext::with_host("wrk-01");
//! let resolved = config::load_config(&[cfg], &machine)?;
//! let planned = plan::plan(&resolved, Verb::Sync, RunOptions::default())?;
//! let outcomes = plan::execute(&planned, &SystemClock, /* dry_run = */ true);
//! assert_eq!(outcomes.len(), planned.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![warn(missing_docs)]

pub mod clock;
pub mod config;
pub mod edit;
pub mod error;
pub mod fs;
pub mod model;
pub mod plan;
pub mod status;

pub use error::{Error, Result};
pub use plan::{
    Action, ActionKind, DiffPair, DiffState, FsOp, Outcome, Planned, RunOptions, SharedSide,
    SharedTarget, Verb, diff_pairs, entry_paths, execute, plan, shared_targets,
};
pub use status::{StatusEntry, StatusLabel, status};
