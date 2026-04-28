use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillTreeError {
    #[error("group `{group}` requires `{dep}` which does not exist")]
    UnknownDependency { group: String, dep: String },

    #[error("circular dependency detected: {cycle}")]
    CircularDependency { cycle: String },

    #[error("item in group `{group}` is missing a required `label` field")]
    MissingLabel { group: String },

    #[error("failed to parse skill tree TOML: {0}")]
    ParseError(#[from] toml::de::Error),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
