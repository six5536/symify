//! Error type for `symify-core`.

use std::path::PathBuf;

/// Convenience alias for results from this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while loading config, planning, or executing.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// An I/O error tied to a specific path.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The path being operated on.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to parse a TOML config file.
    #[error("failed to parse config {path}: {source}")]
    Toml {
        /// The config file that failed to parse.
        path: PathBuf,
        /// The underlying parse error.
        #[source]
        source: toml::de::Error,
    },

    /// A semantic problem with the resolved configuration.
    #[error("config error: {0}")]
    Config(String),

    /// The user's home directory could not be determined (for `~` expansion).
    #[error("could not determine home directory for `~` expansion")]
    NoHome,
}

impl Error {
    /// Build an [`Error::Io`] for `path` from a [`std::io::Error`].
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// Build an [`Error::Config`] from a message.
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;
    use std::io;
    use std::path::Path;

    #[test]
    fn io_constructor_and_display() {
        let e = Error::io(
            Path::new("/tmp/x"),
            io::Error::new(io::ErrorKind::PermissionDenied, "denied"),
        );
        assert!(matches!(e, Error::Io { .. }));
        let msg = e.to_string();
        assert!(msg.contains("/tmp/x"), "display names the path: {msg}");
        assert!(msg.contains("denied"), "display includes the source: {msg}");
        // The underlying io::Error is exposed as the error source.
        assert!(e.source().is_some());
    }

    #[test]
    fn config_constructor_and_display() {
        let e = Error::config("bad mapping");
        assert!(matches!(e, Error::Config(_)));
        assert_eq!(e.to_string(), "config error: bad mapping");
        // A semantic error has no underlying source.
        assert!(e.source().is_none());
    }

    #[test]
    fn config_accepts_string_and_str() {
        // Both `&str` and `String` satisfy `impl Into<String>`.
        let _from_str = Error::config("literal");
        let owned = format!("dynamic {}", 1);
        assert_eq!(Error::config(owned).to_string(), "config error: dynamic 1");
    }

    #[test]
    fn nohome_display() {
        let e = Error::NoHome;
        assert_eq!(
            e.to_string(),
            "could not determine home directory for `~` expansion"
        );
        assert!(e.source().is_none());
    }

    #[test]
    fn toml_variant_display_and_source() {
        // Build a real parse error to populate the Toml variant.
        let parse = toml::from_str::<crate::model::Config>("= not valid toml").unwrap_err();
        let e = Error::Toml {
            path: "/cfg/symify.toml".into(),
            source: parse,
        };
        let msg = e.to_string();
        assert!(msg.contains("/cfg/symify.toml"), "names the file: {msg}");
        assert!(msg.starts_with("failed to parse config"));
        assert!(e.source().is_some());
    }
}
