use thiserror::Error;

#[derive(Error, Debug)]
pub enum SkillsError {
    #[error("Path not found: {0}")]
    PathNotFound(String),

    #[error("Invalid skill format: {0}")]
    InvalidSkillFormat(String),

    #[error("Skill already exists: {0}")]
    SkillAlreadyExists(String),

    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}
