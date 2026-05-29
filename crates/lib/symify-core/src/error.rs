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
