//! Top-level error types and exit codes for the skill-tree CLI.
//! Exit codes: 0 success, 1 general, 2 cycle detected,
//! 3 GitHub API error, 4 configuration error.
//!

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    /// The file could not be read from disk.
    #[error("could not read config file `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The file contained invalid TOML or was missing required fields.
    #[error("could not parse config file `{path}`: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    /// `[colors] github-name` does not match any declared `[[field]]`.
    #[error(
        "`[colors] github-name` is \"{colors_github_name}\" \
         but no [[field]] has that github-name.\n\
         Declared fields: {declared}"
    )]
    ColorsFieldNotDeclared {
        colors_github_name: String,
        declared: String,
    },

    /// A value in `[colors.values]` is not a valid CSS hex color.
    #[error(
        "invalid color `{value}` for `{key}` in [colors.values]: \
         expected a hex color like `#4a90d9` or `#fff`"
    )]
    InvalidColor { key: String, value: String },
}
