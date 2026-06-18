//! Presentation management - collections of slides assembled for a talk

use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A presentation playlist - defines which slides to include
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr playlist schema",
    description = "Configuration schema for sldr presentation playlists (playlist.toml)"
)]
pub struct Playlist {
    /// Name of the playlist/presentation
    pub name: String,

    /// Optional title for the presentation
    #[serde(default)]
    pub title: Option<String>,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// List of slide references (names or paths)
    #[serde(default)]
    pub slides: Vec<String>,

    /// Default flavor to use
    #[serde(default)]
    pub flavor: Option<String>,

    /// Default language for slides with in-file language blocks
    /// (`::lang:xx::`). A suggestion, not identity — the build's --lang
    /// takes precedence.
    #[serde(default)]
    pub default_lang: Option<String>,

    /// Rendering configuration
    #[serde(default, alias = "slidev_config")]
    pub render: RenderOpts,
}

/// Presentation rendering configuration
///
/// Serialized as the `[render]` table of a playlist (the legacy
/// `[slidev_config]` key is still read for older playlist.toml files).
/// Controls transition style, aspect ratio, and other rendering options for
/// the self-contained HTML output.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct RenderOpts {
    /// Theme name (reserved for future use)
    #[serde(default)]
    pub theme: Option<String>,

    /// Enable/disable drawing feature (reserved for future use)
    #[serde(default)]
    pub drawings: Option<bool>,

    /// Transition effect between slides: "fade", "slide-left", "slide-right", "none"
    #[serde(default)]
    pub transition: Option<String>,

    /// Title override for the presentation
    #[serde(default)]
    pub title: Option<String>,

    /// Start in dark mode
    #[serde(default)]
    pub dark_mode: Option<bool>,

    /// Aspect ratio hint for PDF/PPTX export viewport (e.g., "16/9", "4/3").
    /// The HTML output is fully responsive and fills the browser viewport.
    #[serde(default)]
    pub aspect_ratio: Option<String>,

    /// Canvas width hint in pixels for export (informational)
    #[serde(default)]
    pub canvas_width: Option<u32>,

    /// Enable slide recording (reserved for future use)
    #[serde(default)]
    pub record: Option<bool>,
}

impl Playlist {
    /// Load a playlist from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let playlist: Playlist = toml::from_str(&content)?;
        Ok(playlist)
    }

    /// Save playlist to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create a playlist from JSON input
    pub fn from_json(json: &str) -> Result<Self> {
        let playlist: Playlist = serde_json::from_str(json)?;
        Ok(playlist)
    }
}

/// Input structure for creating a playlist via JSON
/// Used by agents/LLMs to create a presentation playlist
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr playlist input schema",
    description = "JSON schema for creating presentation playlists via sldr CLI"
)]
pub struct PlaylistInput {
    /// Name of the playlist (used as filename)
    pub name: String,

    /// Title for the presentation
    pub title: String,

    /// Optional description
    #[serde(default)]
    pub description: Option<String>,

    /// List of slide references (paths relative to slides directory)
    pub slides: Vec<String>,

    /// Flavor to use (e.g., "acme", "default")
    #[serde(default)]
    pub flavor: Option<String>,

    /// Rendering configuration
    #[serde(default, alias = "slidev_config")]
    pub render: Option<RenderOpts>,
}

impl From<PlaylistInput> for Playlist {
    fn from(input: PlaylistInput) -> Self {
        Playlist {
            name: input.name,
            title: Some(input.title),
            description: input.description,
            slides: input.slides,
            flavor: input.flavor,
            default_lang: None,
            render: input.render.unwrap_or_default(),
        }
    }
}
