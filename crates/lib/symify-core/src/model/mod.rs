//! Config data model.
//!
//! [`generated`] holds the types generated from `schema/symify.schema.json` by
//! `cargo typify` (regenerate with `npm run codegen`). This module re-exports
//! them and adds hand-written ergonomics (defaults, [`LinkValue`] interpretation)
//! that the generator can't express.

/// Types generated from `schema/symify.schema.json` by `cargo typify`
/// (regenerate with `npm run codegen`). Field and variant docs come from the
/// schema's `description`s. The `missing_docs` allow covers only what typify
/// cannot document — the two untagged [`LinkValue`] `oneOf` arms (`String` /
/// `Boolean`) — since per-item attributes can't be placed in generated code.
#[rustfmt::skip]
#[allow(clippy::all)]
#[allow(missing_docs)]
pub mod generated;

pub use generated::{Config, Conflict, LinkValue, Mapping, Mirror, Mode, Settings};

/// Default link mechanism when neither `[settings]` nor a mapping specifies one.
pub const DEFAULT_MODE: Mode = Mode::Symlink;

/// Default conflict policy: back up the overwritten file (safest).
pub const DEFAULT_CONFLICT: Conflict = Conflict::Backup;

/// Default mirror policy: off (additive — never prune destination-only files).
pub const DEFAULT_MIRROR: bool = false;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_value_kind_covers_every_spelling() {
        assert_eq!(LinkValue::Boolean(false).kind(), LinkKind::Disabled);
        assert_eq!(LinkValue::Boolean(true).kind(), LinkKind::Mirror);
        assert_eq!(LinkValue::String(String::new()).kind(), LinkKind::Mirror);
        assert_eq!(
            LinkValue::String("vim/vimrc".into()).kind(),
            LinkKind::Explicit("vim/vimrc")
        );
    }

    #[test]
    fn is_disabled_only_for_false() {
        assert!(LinkValue::Boolean(false).is_disabled());
        assert!(!LinkValue::Boolean(true).is_disabled());
        assert!(!LinkValue::String(String::new()).is_disabled());
        assert!(!LinkValue::String("p".into()).is_disabled());
    }
}
