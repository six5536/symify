//! Config data model.
//!
//! [`generated`] holds the types generated from `schema/symify.schema.json` by
//! `cargo typify` (regenerate with `npm run codegen`). This module re-exports
//! them and adds hand-written ergonomics (defaults, [`LinkValue`] interpretation)
//! that the generator can't express.

#[rustfmt::skip]
#[allow(clippy::all)]
pub mod generated;

pub use generated::{Config, Conflict, LinkValue, Mapping, Mode, Settings};

/// Default link mechanism when neither `[settings]` nor a mapping specifies one.
pub const DEFAULT_MODE: Mode = Mode::Symlink;

/// Default conflict policy: back up the overwritten file (safest).
pub const DEFAULT_CONFLICT: Conflict = Conflict::Backup;

/// How a [`LinkValue`] resolves the store-side path for an entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind<'a> {
    /// `false` — the entry is turned off.
    Disabled,
    /// `""` or `true` — mirror the key path under `store`.
    Mirror,
    /// `"<path>"` — explicit store path (relative to `store`, or absolute).
    Explicit(&'a str),
}

impl LinkValue {
    /// Interpret this value per the link-resolution rules.
    pub fn kind(&self) -> LinkKind<'_> {
        match self {
            LinkValue::Boolean(false) => LinkKind::Disabled,
            LinkValue::Boolean(true) => LinkKind::Mirror,
            LinkValue::String(s) if s.is_empty() => LinkKind::Mirror,
            LinkValue::String(s) => LinkKind::Explicit(s),
        }
    }

    /// `true` when the entry is disabled (`false`).
    pub fn is_disabled(&self) -> bool {
        matches!(self.kind(), LinkKind::Disabled)
    }
}
