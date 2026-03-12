//! Configuration management for sldr
//!
//! Handles loading and saving configuration from XDG-compliant paths.
//! Priority order: CLI args > env vars > local config > global config > defaults

use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr config schema",
    description = "Configuration schema for sldr (main config.toml)"
)]
pub struct Config {
    #[serde(default)]
    pub config: CoreConfig,

    #[serde(default)]
    pub presentations: PresentationsConfig,

    #[serde(default)]
    pub matching: MatchingConfig,
}

/// Core application settings
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CoreConfig {
    /// Directory containing slide templates
    #[serde(default = "default_template_dir")]
    pub template_dir: String,

    /// Directory containing flavors (themes/styles)
    #[serde(default = "default_flavor_dir")]
    pub flavor_dir: String,

    /// Default flavor to use when none specified
    #[serde(default = "default_flavor")]
    pub default_flavor: String,

    /// Port for the local dev/watch server
    #[serde(default = "default_dev_port", alias = "slidev_port")]
    pub dev_port: String,

    /// Preferred AI agent for slide generation
    #[serde(default = "default_agent")]
    pub agent: String,
}

/// Presentations and slides configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PresentationsConfig {
    /// Directory containing individual slide files
    #[serde(default = "default_slide_dir")]
    pub slide_dir: String,

    /// Directory for generated presentations
    #[serde(default = "default_output_dir")]
    pub output_dir: String,

    /// Directory containing presentation skeletons
    #[serde(default = "default_skeleton_dir")]
    pub skeleton_dir: String,
}

/// Fuzzy matching configuration
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MatchingConfig {
    /// Order in which to try resolution methods
    #[serde(default = "default_resolution_order")]
    pub resolution_order: Vec<String>,

    /// Minimum fuzzy match score (0-100)
    #[serde(default = "default_threshold")]
    pub threshold: f64,

    /// Maximum number of suggestions to show
    #[serde(default = "default_max_suggestions")]
    pub max_suggestions: usize,
}

// Default value functions
fn default_template_dir() -> String {
    "~/.config/sldr/templates".to_string()
}

fn default_flavor_dir() -> String {
    "~/.config/sldr/flavors".to_string()
}

fn default_flavor() -> String {
    "default".to_string()
}

fn default_dev_port() -> String {
    "3030".to_string()
}

fn default_agent() -> String {
    "opencode".to_string()
}

fn default_slide_dir() -> String {
    "~/sldr/slides".to_string()
}

fn default_output_dir() -> String {
    "~/sldr/presentations".to_string()
}

fn default_skeleton_dir() -> String {
    "~/sldr/skeletons".to_string()
}

fn default_resolution_order() -> Vec<String> {
    vec![
        "anchor".to_string(),
        "exact".to_string(),
        "fuzzy".to_string(),
        "index".to_string(),
        "interactive".to_string(),
    ]
}

fn default_threshold() -> f64 {
    50.0
}

fn default_max_suggestions() -> usize {
    6
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            template_dir: default_template_dir(),
            flavor_dir: default_flavor_dir(),
            default_flavor: default_flavor(),
            dev_port: default_dev_port(),
            agent: default_agent(),
        }
    }
}

impl Default for PresentationsConfig {
    fn default() -> Self {
        Self {
            slide_dir: default_slide_dir(),
            output_dir: default_output_dir(),
            skeleton_dir: default_skeleton_dir(),
        }
    }
}

impl Default for MatchingConfig {
    fn default() -> Self {
        Self {
            resolution_order: default_resolution_order(),
            threshold: default_threshold(),
            max_suggestions: default_max_suggestions(),
        }
    }
}

impl Config {
    /// Get the XDG config directory for sldr
    pub fn config_dir() -> PathBuf {
        // Priority: $XDG_CONFIG_HOME > ~/.config
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("sldr");
            }
        }

        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("~/.config"))
            .join("sldr")
    }

    /// Get the XDG data directory for sldr
    pub fn data_dir() -> PathBuf {
        // Priority: $XDG_DATA_HOME > ~/.local/share
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("sldr");
            }
        }

        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("sldr")
    }

    /// Get the XDG state directory for sldr
    pub fn state_dir() -> PathBuf {
        // Priority: $XDG_STATE_HOME > ~/.local/state
        if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("sldr");
            }
        }

        dirs::state_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/state"))
            .join("sldr")
    }

    /// Get the path to the global config file
    pub fn config_file_path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = Self::config_file_path();

        if config_path.exists() {
            debug!("Loading config from {:?}", config_path);
            Self::load_from_path(&config_path)
        } else {
            info!(
                "Config file not found, creating default at {:?}",
                config_path
            );
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Load configuration from a specific path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to the default location
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_file_path();
        self.save_to_path(&config_path)
    }

    /// Save configuration to a specific path
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        info!("Saved config to {:?}", path);
        Ok(())
    }

    /// Expand a path string, resolving ~ and environment variables
    pub fn expand_path(path: &str) -> PathBuf {
        let expanded = shellexpand::full(path).unwrap_or_else(|_| path.into());
        PathBuf::from(expanded.as_ref())
    }

    /// Get the expanded slide directory path
    pub fn slide_dir(&self) -> PathBuf {
        Self::expand_path(&self.presentations.slide_dir)
    }

    /// Get the expanded output directory path
    pub fn output_dir(&self) -> PathBuf {
        Self::expand_path(&self.presentations.output_dir)
    }

    /// Get the expanded skeleton directory path
    pub fn skeleton_dir(&self) -> PathBuf {
        Self::expand_path(&self.presentations.skeleton_dir)
    }

    /// Get the expanded template directory path
    pub fn template_dir(&self) -> PathBuf {
        Self::expand_path(&self.config.template_dir)
    }

    /// Get the expanded flavor directory path
    pub fn flavor_dir(&self) -> PathBuf {
        Self::expand_path(&self.config.flavor_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.config.default_flavor, "default");
        assert_eq!(config.config.dev_port, "3030");
        // Use approximate comparison for floats
        assert!((config.matching.threshold - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_expand_path() {
        let path = Config::expand_path("~/test");
        assert!(!path.to_string_lossy().contains('~'));
    }
}
