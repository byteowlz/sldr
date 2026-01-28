//! Error types for sldr-core

use thiserror::Error;

/// Result type alias using sldr Error
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in sldr operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Slide not found: {0}")]
    SlideNotFound(String),

    #[error("Presentation not found: {0}")]
    PresentationNotFound(String),

    #[error("Flavor not found: {0}")]
    FlavorNotFound(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Multiple matches found for '{query}': {matches:?}")]
    MultipleMatches { query: String, matches: Vec<String> },

    #[error("Invalid slide format: {0}")]
    InvalidSlideFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerialize(#[from] toml::ser::Error),

    #[error("{0}")]
    Other(String),
}
