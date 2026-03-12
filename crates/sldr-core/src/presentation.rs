//! Presentation management - collections of slides assembled for a talk

use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A presentation skeleton - defines which slides to include
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr skeleton schema",
    description = "Configuration schema for sldr presentation skeletons (skeleton.toml)"
)]
pub struct Skeleton {
    /// Name of the skeleton/presentation
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

    /// Rendering configuration
    #[serde(default)]
    pub slidev_config: RenderOpts,
}

/// Presentation rendering configuration
///
/// Serialized as `slidev_config` for backwards compatibility with existing
/// skeleton.toml files. Controls transition style, aspect ratio, and other
/// rendering options for the self-contained HTML output.
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

/// Backwards-compatible type alias
pub type SlidevConfig = RenderOpts;

impl Skeleton {
    /// Load a skeleton from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let skeleton: Skeleton = toml::from_str(&content)?;
        Ok(skeleton)
    }

    /// Save skeleton to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Create a skeleton from JSON input
    pub fn from_json(json: &str) -> Result<Self> {
        let skeleton: Skeleton = serde_json::from_str(json)?;
        Ok(skeleton)
    }
}

/// Input structure for creating a skeleton via JSON
/// Used by agents/LLMs to create a presentation skeleton
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr skeleton input schema",
    description = "JSON schema for creating presentation skeletons via sldr CLI"
)]
pub struct SkeletonInput {
    /// Name of the skeleton (used as filename)
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
    #[serde(default)]
    pub slidev_config: Option<RenderOpts>,
}

impl From<SkeletonInput> for Skeleton {
    fn from(input: SkeletonInput) -> Self {
        Skeleton {
            name: input.name,
            title: Some(input.title),
            description: input.description,
            slides: input.slides,
            flavor: input.flavor,
            slidev_config: input.slidev_config.unwrap_or_default(),
        }
    }
}
