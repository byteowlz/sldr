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
    /// Library root: the single self-sufficient tree carrying slides,
    /// layouts, flavors, playlists, scaffolds, and media.
    /// Asset resolution searches the library first, then the extra dirs
    /// below, then the built-ins embedded in the binary.
    #[serde(default = "default_library")]
    pub library: String,

    /// Extra directory containing slide scaffolds (searched after the
    /// library's scaffolds/)
    #[serde(default = "default_scaffold_dir")]
    pub scaffold_dir: String,

    /// Directory containing user layouts (override/extend built-ins by name)
    #[serde(default = "default_layout_dir")]
    pub layout_dir: String,

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

    /// Directory containing presentation playlists
    #[serde(default = "default_playlist_dir")]
    pub playlist_dir: String,
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
fn default_library() -> String {
    "~/sldr".to_string()
}

fn default_scaffold_dir() -> String {
    "~/.config/sldr/scaffolds".to_string()
}

fn default_layout_dir() -> String {
    "~/.config/sldr/layouts".to_string()
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

fn default_playlist_dir() -> String {
    "~/sldr/playlists".to_string()
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
            library: default_library(),
            scaffold_dir: default_scaffold_dir(),
            layout_dir: default_layout_dir(),
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
            playlist_dir: default_playlist_dir(),
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

/// Resolve a base directory using "option B" rules (zero external deps):
///
/// 1. An explicit, absolute `XDG_*` value wins on ANY OS.
/// 2. Otherwise, on unix (incl. macOS) use `$HOME` joined with the XDG-style
///    relative path (e.g. `.config`) — never `~/Library`.
/// 3. Otherwise, on Windows use the relevant `%APPDATA%`/`%LOCALAPPDATA%` dir.
fn resolve_base(
    xdg: Option<PathBuf>,
    home: Option<PathBuf>,
    win_dir: Option<PathBuf>,
    is_windows: bool,
    unix_rel: &str,
) -> Option<PathBuf> {
    if let Some(p) = xdg.filter(|p| p.is_absolute()) {
        return Some(p);
    }
    if is_windows {
        win_dir
    } else {
        home.map(|h| h.join(unix_rel))
    }
}

/// Resolve a base directory from environment variables using "option B" rules.
fn base_dir(xdg_var: &str, unix_rel: &str, win_var: &str) -> anyhow::Result<PathBuf> {
    resolve_base(
        std::env::var_os(xdg_var).map(PathBuf::from),
        std::env::var_os("HOME").map(PathBuf::from),
        std::env::var_os(win_var).map(PathBuf::from),
        cfg!(windows),
        unix_rel,
    )
    .ok_or_else(|| anyhow::anyhow!("unable to determine base directory ({xdg_var})"))
}

impl Config {
    /// Get the XDG config directory for sldr
    pub fn config_dir() -> PathBuf {
        // Priority: $XDG_CONFIG_HOME > ~/.config (or %APPDATA% on Windows)
        base_dir("XDG_CONFIG_HOME", ".config", "APPDATA")
            .unwrap_or_else(|_| PathBuf::from("~/.config"))
            .join("sldr")
    }

    /// Get the XDG data directory for sldr
    pub fn data_dir() -> PathBuf {
        // Priority: $XDG_DATA_HOME > ~/.local/share (or %APPDATA% on Windows)
        base_dir("XDG_DATA_HOME", ".local/share", "APPDATA")
            .unwrap_or_else(|_| PathBuf::from("~/.local/share"))
            .join("sldr")
    }

    /// Get the XDG state directory for sldr
    pub fn state_dir() -> PathBuf {
        // Priority: $XDG_STATE_HOME > ~/.local/state (or %LOCALAPPDATA% on Windows)
        base_dir("XDG_STATE_HOME", ".local/state", "LOCALAPPDATA")
            .unwrap_or_else(|_| PathBuf::from("~/.local/state"))
            .join("sldr")
    }

    /// Get the XDG cache directory for sldr
    pub fn cache_dir() -> PathBuf {
        // Priority: $XDG_CACHE_HOME > ~/.cache (or %LOCALAPPDATA% on Windows)
        base_dir("XDG_CACHE_HOME", ".cache", "LOCALAPPDATA")
            .unwrap_or_else(|_| PathBuf::from("~/.cache"))
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

    /// Get the expanded playlist directory path
    pub fn playlist_dir(&self) -> PathBuf {
        Self::expand_path(&self.presentations.playlist_dir)
    }

    /// Get the expanded scaffold directory path
    pub fn scaffold_dir(&self) -> PathBuf {
        Self::expand_path(&self.config.scaffold_dir)
    }

    /// Get the expanded user layout directory path
    pub fn layout_dir(&self) -> PathBuf {
        Self::expand_path(&self.config.layout_dir)
    }

    /// Get the expanded library root (ADR-0007)
    pub fn library(&self) -> PathBuf {
        Self::expand_path(&self.config.library)
    }

    /// Flavor search dirs, highest priority first:
    /// library/flavors, then the configured extra flavor_dir.
    pub fn flavor_dirs(&self) -> Vec<PathBuf> {
        vec![self.library().join("flavors"), self.flavor_dir()]
    }

    /// Layout dirs in *load* order (later loads override earlier ones, so
    /// the library wins over the configured extra dir, which wins over
    /// built-ins).
    pub fn layout_dirs(&self) -> Vec<PathBuf> {
        vec![self.layout_dir(), self.library().join("layouts")]
    }

    /// Scaffold search dirs, highest priority first:
    /// library/scaffolds, then the configured extra scaffold_dir.
    pub fn scaffold_dirs(&self) -> Vec<PathBuf> {
        vec![self.library().join("scaffolds"), self.scaffold_dir()]
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

    #[test]
    fn resolve_base_absolute_xdg_wins_on_any_os() {
        let xdg = Some(PathBuf::from("/explicit/xdg"));
        let home = Some(PathBuf::from("/home/user"));
        let win = Some(PathBuf::from("C:\\Users\\u\\AppData\\Roaming"));

        // unix
        assert_eq!(
            resolve_base(xdg.clone(), home.clone(), win.clone(), false, ".config"),
            Some(PathBuf::from("/explicit/xdg"))
        );
        // windows
        assert_eq!(
            resolve_base(xdg.clone(), home.clone(), win.clone(), true, ".config"),
            Some(PathBuf::from("/explicit/xdg"))
        );
    }

    #[test]
    fn resolve_base_relative_xdg_is_ignored() {
        // A non-absolute XDG value must not win; fall back to HOME/unix_rel.
        let xdg = Some(PathBuf::from("relative/path"));
        let home = Some(PathBuf::from("/home/user"));
        assert_eq!(
            resolve_base(xdg, home, None, false, ".config"),
            Some(PathBuf::from("/home/user/.config"))
        );
    }

    #[test]
    fn resolve_base_unix_uses_home_not_library() {
        let home = Some(PathBuf::from("/Users/user"));
        let got = resolve_base(None, home, None, false, ".config");
        // macOS must not be routed to ~/Library; HOME/.config is used instead.
        assert_eq!(got, Some(PathBuf::from("/Users/user/.config")));
        let rendered = got.map(|p| p.to_string_lossy().into_owned());
        assert_eq!(rendered.as_deref(), Some("/Users/user/.config"));
        assert!(!rendered.unwrap_or_default().contains("Library"));
    }

    #[test]
    fn resolve_base_windows_uses_win_dir() {
        let win = Some(PathBuf::from("C:\\Users\\u\\AppData\\Roaming"));
        assert_eq!(
            resolve_base(
                None,
                Some(PathBuf::from("/home/user")),
                win.clone(),
                true,
                ".config"
            ),
            win
        );
    }

    #[test]
    fn resolve_base_none_when_unresolvable() {
        // unix without HOME
        assert_eq!(resolve_base(None, None, None, false, ".config"), None);
        // windows without win_dir
        assert_eq!(
            resolve_base(None, Some(PathBuf::from("/h")), None, true, ".config"),
            None
        );
    }
}
