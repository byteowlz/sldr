//! Flavor management - themes and styling for presentations
//!
//! Flavors separate content from style, allowing the same slides
//! to be rendered with different visual themes.

use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A flavor definition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr flavor schema",
    description = "Configuration schema for sldr flavors (flavor.toml)"
)]
pub struct Flavor {
    /// Unique name for this flavor
    pub name: String,

    /// Human-readable display name
    #[serde(default)]
    pub display_name: Option<String>,

    /// Description of the flavor
    #[serde(default)]
    pub description: Option<String>,

    /// Color scheme (light mode / default)
    #[serde(default)]
    pub colors: ColorScheme,

    /// Dark mode color overrides (merged on top of colors when dark mode is active)
    #[serde(default)]
    pub dark_colors: Option<ColorScheme>,

    /// Typography settings
    #[serde(default)]
    pub typography: Typography,

    /// Background settings
    #[serde(default)]
    pub background: BackgroundConfig,

    /// Logo placements - positioned logo overlays on slides
    #[serde(default)]
    pub logos: Vec<LogoPlacement>,

    /// Path to additional assets (logos, images)
    #[serde(default)]
    pub assets_dir: Option<String>,

    /// Source directory where the flavor was loaded from (not serialized)
    #[serde(skip)]
    #[schemars(skip)]
    pub source_dir: Option<PathBuf>,
}

/// Color scheme for a flavor
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ColorScheme {
    /// Primary brand color
    #[serde(default)]
    pub primary: Option<String>,

    /// Secondary color
    #[serde(default)]
    pub secondary: Option<String>,

    /// Background color
    #[serde(default)]
    pub background: Option<String>,

    /// Text color
    #[serde(default)]
    pub text: Option<String>,

    /// Accent color for highlights
    #[serde(default)]
    pub accent: Option<String>,

    /// Code block background
    #[serde(default)]
    pub code_background: Option<String>,

    /// Code text color
    #[serde(default)]
    pub code_text: Option<String>,
}

/// Typography settings
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Typography {
    /// Heading font family
    #[serde(default)]
    pub heading_font: Option<String>,

    /// Body text font family
    #[serde(default)]
    pub body_font: Option<String>,

    /// Code font family
    #[serde(default)]
    pub code_font: Option<String>,

    /// Base font size
    #[serde(default)]
    pub base_size: Option<String>,
}

/// Background configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct BackgroundConfig {
    /// Background type: color, image, gradient, svg
    #[serde(default)]
    pub background_type: Option<String>,

    /// Value depends on type (color hex, image path, gradient CSS, svg path)
    #[serde(default)]
    pub value: Option<String>,

    /// Opacity for background overlay
    #[serde(default)]
    pub opacity: Option<f32>,
}

/// A logo placement slot.
///
/// Defines where a logo should appear, at what size/opacity,
/// and on which slide layouts. The actual logo file is resolved
/// from the flavor's assets directory at build time.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LogoPlacement {
    /// Filename in the flavor's assets directory (e.g. "company-logo.svg")
    pub file: String,

    /// Named position preset: "top-left", "top-right", "top-center",
    /// "bottom-left", "bottom-right", "bottom-center"
    #[serde(default = "default_logo_position")]
    pub position: String,

    /// Custom X offset (CSS value, e.g. "5%", "20px"). Overrides position preset.
    #[serde(default)]
    pub x: Option<String>,

    /// Custom Y offset (CSS value, e.g. "5%", "20px"). Overrides position preset.
    #[serde(default)]
    pub y: Option<String>,

    /// Logo width (CSS value, e.g. "120px", "8vw")
    #[serde(default = "default_logo_width")]
    pub width: String,

    /// Opacity 0.0-1.0
    #[serde(default = "default_logo_opacity")]
    pub opacity: f32,

    /// Which layouts this logo appears on.
    /// Use ["all"] for every slide, or specific layouts like ["default", "two-cols"].
    #[serde(default = "default_logo_templates")]
    pub templates: Vec<String>,
}

fn default_logo_position() -> String {
    "top-right".to_string()
}

fn default_logo_width() -> String {
    "100px".to_string()
}

fn default_logo_opacity() -> f32 {
    0.8
}

fn default_logo_templates() -> Vec<String> {
    vec!["all".to_string()]
}

impl LogoPlacement {
    /// Convert the position preset + custom offsets into inline CSS for
    /// absolute positioning within a slide.
    pub fn to_css_position(&self) -> String {
        // Custom x/y override the preset
        if self.x.is_some() || self.y.is_some() {
            let x = self.x.as_deref().unwrap_or("auto");
            let y = self.y.as_deref().unwrap_or("auto");
            return format!(
                "position:absolute;left:{x};top:{y};width:{w};opacity:{o};z-index:10;pointer-events:none;",
                w = self.width,
                o = self.opacity,
            );
        }

        let (pos_css, transform) = match self.position.as_str() {
            "top-left" => ("top:3%;left:3%;", ""),
            "top-center" => ("top:3%;left:50%;", "transform:translateX(-50%);"),
            "top-right" => ("top:3%;right:3%;", ""),
            "bottom-left" => ("bottom:3%;left:3%;", ""),
            "bottom-center" => ("bottom:3%;left:50%;", "transform:translateX(-50%);"),
            "bottom-right" => ("bottom:3%;right:3%;", ""),
            _ => ("top:3%;right:3%;", ""), // fallback to top-right
        };

        format!(
            "position:absolute;{pos_css}{transform}width:{w};opacity:{o};z-index:10;pointer-events:none;",
            w = self.width,
            o = self.opacity,
        )
    }

    /// Check if this logo should appear on a given layout
    pub fn applies_to_layout(&self, layout: &str) -> bool {
        self.templates.iter().any(|t| t == "all" || t == layout)
    }
}

impl Default for Flavor {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            display_name: Some("Default".to_string()),
            description: Some("Default sldr flavor".to_string()),
            colors: ColorScheme::default(),
            dark_colors: None,
            typography: Typography::default(),
            background: BackgroundConfig::default(),
            logos: Vec::new(),
            assets_dir: None,
            source_dir: None,
        }
    }
}

impl Flavor {
    /// Load a flavor from its directory
    pub fn load(dir: &Path) -> Result<Self> {
        let config_path = dir.join("flavor.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut flavor: Flavor = toml::from_str(&content)?;

            // Set name from directory if not specified
            if flavor.name.is_empty() {
                flavor.name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
            }

            // Store the source directory for asset copying
            flavor.source_dir = Some(dir.to_path_buf());

            Ok(flavor)
        } else {
            // Create a minimal flavor from directory name
            Ok(Self {
                name: dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                source_dir: Some(dir.to_path_buf()),
                ..Default::default()
            })
        }
    }

    /// Save flavor configuration to its directory
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let config_path = dir.join("flavor.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// Generate CSS variables for this flavor
    pub fn to_css_variables(&self) -> String {
        let mut css = String::from(":root {\n");

        if let Some(ref color) = self.colors.primary {
            let _ = writeln!(css, "  --sldr-primary: {color};");
        }
        if let Some(ref color) = self.colors.secondary {
            let _ = writeln!(css, "  --sldr-secondary: {color};");
        }
        if let Some(ref color) = self.colors.background {
            let _ = writeln!(css, "  --sldr-background: {color};");
        }
        if let Some(ref color) = self.colors.text {
            let _ = writeln!(css, "  --sldr-text: {color};");
        }
        if let Some(ref color) = self.colors.accent {
            let _ = writeln!(css, "  --sldr-accent: {color};");
        }
        if let Some(ref color) = self.colors.code_background {
            let _ = writeln!(css, "  --sldr-code-background: {color};");
        }
        if let Some(ref color) = self.colors.code_text {
            let _ = writeln!(css, "  --sldr-code-text: {color};");
        }
        if let Some(ref font) = self.typography.heading_font {
            let _ = writeln!(css, "  --sldr-heading-font: {font};");
        }
        if let Some(ref font) = self.typography.body_font {
            let _ = writeln!(css, "  --sldr-body-font: {font};");
        }
        if let Some(ref font) = self.typography.code_font {
            let _ = writeln!(css, "  --sldr-code-font: {font};");
        }
        if let Some(ref size) = self.typography.base_size {
            let _ = writeln!(css, "  --sldr-base-size: {size};");
        }

        css.push_str("}\n");

        // Dark mode overrides
        if let Some(ref dark) = self.dark_colors {
            css.push_str("html.dark {\n");
            if let Some(ref color) = dark.primary {
                let _ = writeln!(css, "  --sldr-primary: {color};");
            }
            if let Some(ref color) = dark.secondary {
                let _ = writeln!(css, "  --sldr-secondary: {color};");
            }
            if let Some(ref color) = dark.background {
                let _ = writeln!(css, "  --sldr-background: {color};");
            }
            if let Some(ref color) = dark.text {
                let _ = writeln!(css, "  --sldr-text: {color};");
            }
            if let Some(ref color) = dark.accent {
                let _ = writeln!(css, "  --sldr-accent: {color};");
            }
            if let Some(ref color) = dark.code_background {
                let _ = writeln!(css, "  --sldr-code-background: {color};");
            }
            if let Some(ref color) = dark.code_text {
                let _ = writeln!(css, "  --sldr-code-text: {color};");
            }
            css.push_str("}\n");
        }

        css
    }

    /// Generate CSS for background styling (used by the HTML renderer)
    ///
    /// Returns CSS rules that apply the configured background to `.sldr-slide`.
    /// For image/svg backgrounds, the caller must ensure the asset file is
    /// available at the path referenced by `value`.
    pub fn to_background_css(&self) -> String {
        let mut css = String::new();

        if let Some(ref bg_type) = self.background.background_type {
            if let Some(ref value) = self.background.value {
                match bg_type.as_str() {
                    "color" => {
                        let _ = writeln!(
                            css,
                            ".sldr-slide {{ background-color: {value}; }}"
                        );
                    }
                    "gradient" => {
                        let _ = writeln!(
                            css,
                            ".sldr-slide {{ background: {value}; }}"
                        );
                    }
                    "image" | "svg" => {
                        let web_path = if value.starts_with('/') || value.starts_with("http") {
                            value.clone()
                        } else {
                            format!("/{value}")
                        };
                        let _ = writeln!(
                            css,
                            ".sldr-slide {{ background-image: url('{web_path}'); background-size: cover; background-position: center; }}"
                        );
                    }
                    _ => {}
                }
            }
        }

        if let Some(opacity) = self.background.opacity {
            if opacity < 1.0 {
                let _ = writeln!(
                    css,
                    ".sldr-slide::before {{ content: ''; position: absolute; inset: 0; background: inherit; opacity: {opacity}; z-index: -1; }}"
                );
            }
        }

        css
    }
}

/// Collection of available flavors
#[derive(Debug)]
pub struct FlavorCollection {
    pub flavors: Vec<Flavor>,
    pub base_dir: PathBuf,
}

impl FlavorCollection {
    /// Load all flavors from a directory
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut flavors = Vec::new();

        if !dir.exists() {
            return Ok(Self {
                flavors,
                base_dir: dir.to_path_buf(),
            });
        }

        // Each subdirectory is a flavor
        for entry in WalkDir::new(dir)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            if entry.file_type().is_dir() {
                match Flavor::load(entry.path()) {
                    Ok(flavor) => flavors.push(flavor),
                    Err(e) => {
                        tracing::warn!("Failed to load flavor {:?}: {}", entry.path(), e);
                    }
                }
            }
        }

        flavors.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(Self {
            flavors,
            base_dir: dir.to_path_buf(),
        })
    }

    /// Get flavor names for matching
    pub fn names(&self) -> Vec<String> {
        self.flavors.iter().map(|f| f.name.clone()).collect()
    }

    /// Find a flavor by name
    pub fn find(&self, name: &str) -> Option<&Flavor> {
        self.flavors.iter().find(|f| f.name == name)
    }
}
