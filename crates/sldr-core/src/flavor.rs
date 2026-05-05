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

    /// Spacing rhythm
    #[serde(default)]
    pub spacing: Spacing,

    /// Shape (radius, borders)
    #[serde(default)]
    pub shape: Shape,

    /// Shadow / elevation scale
    #[serde(default)]
    pub shadow: Shadow,

    /// Motion / transitions
    #[serde(default)]
    pub motion: Motion,

    /// Decorative ornaments
    #[serde(default)]
    pub decoration: Decoration,

    /// Code block styling
    #[serde(default)]
    pub code: Code,

    /// Logo placements - positioned logo overlays on slides
    #[serde(default)]
    pub logos: Vec<LogoPlacement>,

    /// Path to additional assets (logos, images)
    #[serde(default)]
    pub assets_dir: Option<String>,

    /// Inline CSS escape hatch — loaded from `flavor.css` next to `flavor.toml`.
    /// Appended after generated tokens so it can override or extend anything.
    /// Use sparingly: prefer tokens. Reserved for visual ideas tokens cannot
    /// express (decorative SVGs, magazine layouts, frame ornaments).
    #[serde(skip)]
    #[schemars(skip)]
    pub custom_css: Option<String>,

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

    /// Surface color (cards, columns, callouts)
    #[serde(default)]
    pub surface: Option<String>,

    /// Elevated surface color (cards on cards, inset panels)
    #[serde(default)]
    pub surface2: Option<String>,

    /// Border color (subtle dividers)
    #[serde(default)]
    pub border: Option<String>,

    /// Brighter border color (emphasised dividers, hovered cards)
    #[serde(default)]
    pub border_bright: Option<String>,

    /// Dim text (captions, footnotes, secondary copy)
    #[serde(default)]
    pub text_dim: Option<String>,

    /// Dim accent (atmospheric glows, accent backgrounds)
    #[serde(default)]
    pub accent_dim: Option<String>,

    /// Muted background (callouts, blockquotes, disabled)
    #[serde(default)]
    pub muted: Option<String>,
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

    /// Base font size (CSS, e.g. "20px")
    #[serde(default)]
    pub base_size: Option<String>,

    /// Heading font weight (e.g. "700", "900")
    #[serde(default)]
    pub heading_weight: Option<String>,

    /// Body font weight
    #[serde(default)]
    pub body_weight: Option<String>,

    /// Heading letter-spacing (CSS, e.g. "-0.03em")
    #[serde(default)]
    pub heading_tracking: Option<String>,

    /// Body letter-spacing
    #[serde(default)]
    pub body_tracking: Option<String>,

    /// Heading line-height (unitless, e.g. "1.05")
    #[serde(default)]
    pub heading_leading: Option<String>,

    /// Body line-height
    #[serde(default)]
    pub body_leading: Option<String>,

    /// Heading text-transform: "none", "uppercase", "lowercase"
    #[serde(default)]
    pub heading_transform: Option<String>,

    /// Eyebrow / label text-transform (small overline labels)
    #[serde(default)]
    pub eyebrow_transform: Option<String>,

    /// Heading text-wrap: "balance" (even multi-line titles), "pretty",
    /// "auto", "nowrap". Default "balance" — suits hero titles.
    #[serde(default)]
    pub heading_wrap: Option<String>,

    /// Body text-wrap: "pretty" (avoid orphans), "balance", "auto".
    #[serde(default)]
    pub body_wrap: Option<String>,

    /// CSS font-feature-settings (e.g. `"ss01" 1, "kern" 1, "liga" 1`).
    /// Lets flavors enable stylistic sets, ligatures, kerning.
    #[serde(default)]
    pub font_features: Option<String>,

    /// CSS font-optical-sizing: "auto" (default) or "none".
    /// Variable fonts expose optical sizing; non-variable fonts ignore it.
    #[serde(default)]
    pub optical_sizing: Option<String>,
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

/// Spacing rhythm
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Spacing {
    /// Slide vertical padding (CSS, e.g. "6vh", "clamp(40px, 6vh, 80px)")
    #[serde(default)]
    pub slide_padding_y: Option<String>,

    /// Slide horizontal padding (CSS)
    #[serde(default)]
    pub slide_padding_x: Option<String>,

    /// Max content width (CSS, e.g. "70ch")
    #[serde(default)]
    pub content_max_width: Option<String>,

    /// Density preset: "compact" | "comfortable" | "spacious"
    #[serde(default)]
    pub density: Option<String>,

    /// Gap between stacked content blocks
    #[serde(default)]
    pub stack_gap: Option<String>,
}

/// Shape (radius, borders)
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Shape {
    /// Base border-radius (CSS, e.g. "12px", "0")
    #[serde(default)]
    pub radius: Option<String>,

    /// Small radius (chips, code inline)
    #[serde(default)]
    pub radius_sm: Option<String>,

    /// Large radius (hero cards, full-bleed media frames)
    #[serde(default)]
    pub radius_lg: Option<String>,

    /// Border width (CSS, e.g. "1px")
    #[serde(default)]
    pub border_width: Option<String>,

    /// Border style: "solid", "dashed", "none"
    #[serde(default)]
    pub border_style: Option<String>,
}

/// Shadow / elevation scale
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Shadow {
    /// Small elevation (subtle lift)
    #[serde(default)]
    pub sm: Option<String>,

    /// Medium elevation (cards)
    #[serde(default)]
    pub md: Option<String>,

    /// Large elevation (hero, modals)
    #[serde(default)]
    pub lg: Option<String>,
}

/// Motion / transitions
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Motion {
    /// Slide transition preset: "slide", "fade", "morph", "none"
    #[serde(default)]
    pub transition: Option<String>,

    /// CSS easing function (sets the base `--sldr-easing`)
    #[serde(default)]
    pub easing: Option<String>,

    /// Transition duration (CSS, e.g. "300ms" — sets the base
    /// `--sldr-duration` and the mid tier `--sldr-duration-base`)
    #[serde(default)]
    pub duration: Option<String>,

    /// Fast duration tier (CSS, e.g. "150ms") — UI feedback, hover states
    #[serde(default)]
    pub duration_fast: Option<String>,

    /// Slow duration tier (CSS, e.g. "600ms") — emphasis transitions
    #[serde(default)]
    pub duration_slow: Option<String>,

    /// `ease-out-quart` curve (defaults to cubic-bezier(0.25, 1, 0.5, 1))
    #[serde(default)]
    pub ease_out_quart: Option<String>,

    /// `ease-out-expo` curve (defaults to cubic-bezier(0.16, 1, 0.3, 1))
    #[serde(default)]
    pub ease_out_expo: Option<String>,
}

/// Decorative ornaments — applied as a CSS class hook.
/// Renderer adds `data-decoration="<accent>"` to the deck root.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Decoration {
    /// Named ornament: "none", "corner-rules", "side-rail", "dot-grid",
    /// "halftone", "swiss-bar", "page-numbers"
    #[serde(default)]
    pub accent: Option<String>,

    /// Intensity 0.0–1.0 (controls opacity / scale of the ornament)
    #[serde(default)]
    pub intensity: Option<f32>,
}

/// Code block styling
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Code {
    /// Syntax highlighting theme name (syntect)
    #[serde(default)]
    pub syntax_theme: Option<String>,

    /// Code frame style: "card", "minimal", "terminal"
    #[serde(default)]
    pub frame_style: Option<String>,

    /// Show line numbers
    #[serde(default)]
    pub line_numbers: Option<bool>,
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
            spacing: Spacing::default(),
            shape: Shape::default(),
            shadow: Shadow::default(),
            motion: Motion::default(),
            decoration: Decoration::default(),
            code: Code::default(),
            logos: Vec::new(),
            assets_dir: None,
            custom_css: None,
            source_dir: None,
        }
    }
}

impl Flavor {
    /// Load a flavor from its directory.
    ///
    /// Reads `flavor.toml` (required for non-default flavors) and optionally
    /// `flavor.css` (escape hatch for visual ideas tokens cannot express —
    /// inlined into the build after generated tokens).
    pub fn load(dir: &Path) -> Result<Self> {
        let config_path = dir.join("flavor.toml");

        let mut flavor = if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let mut flavor: Flavor = toml::from_str(&content)?;

            if flavor.name.is_empty() {
                flavor.name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
            }

            flavor
        } else {
            Self {
                name: dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                ..Default::default()
            }
        };

        flavor.source_dir = Some(dir.to_path_buf());

        let css_path = dir.join("flavor.css");
        if css_path.exists() {
            match std::fs::read_to_string(&css_path) {
                Ok(css) => flavor.custom_css = Some(css),
                Err(e) => tracing::warn!("Failed to read flavor.css at {}: {}", css_path.display(), e),
            }
        }

        Ok(flavor)
    }

    /// Save flavor configuration to its directory
    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let config_path = dir.join("flavor.toml");
        let content = toml::to_string_pretty(self)?;
        std::fs::write(config_path, content)?;
        Ok(())
    }

    /// Generate CSS custom properties for this flavor.
    ///
    /// Emits a `:root { ... }` block with every token the flavor sets,
    /// followed by an `html.dark { ... }` block for dark-mode color overrides.
    /// Unset tokens fall through to the defaults defined in `base.css`.
    pub fn to_css_variables(&self) -> String {
        let mut css = String::from(":root {\n");

        write_color_vars(&mut css, &self.colors);
        write_typography_vars(&mut css, &self.typography);
        write_spacing_vars(&mut css, &self.spacing);
        write_shape_vars(&mut css, &self.shape);
        write_shadow_vars(&mut css, &self.shadow);
        write_motion_vars(&mut css, &self.motion);
        write_decoration_vars(&mut css, &self.decoration);

        css.push_str("}\n");

        if let Some(ref dark) = self.dark_colors {
            css.push_str("html.dark {\n");
            write_color_vars(&mut css, dark);
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

fn write_var(css: &mut String, name: &str, value: &Option<String>) {
    if let Some(v) = value {
        let _ = writeln!(css, "  --sldr-{name}: {v};");
    }
}

fn write_color_vars(css: &mut String, c: &ColorScheme) {
    write_var(css, "primary", &c.primary);
    write_var(css, "secondary", &c.secondary);
    write_var(css, "background", &c.background);
    write_var(css, "text", &c.text);
    write_var(css, "accent", &c.accent);
    write_var(css, "code-background", &c.code_background);
    write_var(css, "code-text", &c.code_text);
    write_var(css, "surface", &c.surface);
    write_var(css, "surface2", &c.surface2);
    write_var(css, "border", &c.border);
    write_var(css, "border-bright", &c.border_bright);
    write_var(css, "text-dim", &c.text_dim);
    write_var(css, "accent-dim", &c.accent_dim);
    write_var(css, "muted", &c.muted);
}

fn write_typography_vars(css: &mut String, t: &Typography) {
    write_var(css, "heading-font", &t.heading_font);
    write_var(css, "body-font", &t.body_font);
    write_var(css, "code-font", &t.code_font);
    write_var(css, "base-size", &t.base_size);
    write_var(css, "heading-weight", &t.heading_weight);
    write_var(css, "body-weight", &t.body_weight);
    write_var(css, "heading-tracking", &t.heading_tracking);
    write_var(css, "body-tracking", &t.body_tracking);
    write_var(css, "heading-leading", &t.heading_leading);
    write_var(css, "body-leading", &t.body_leading);
    write_var(css, "heading-transform", &t.heading_transform);
    write_var(css, "eyebrow-transform", &t.eyebrow_transform);
    write_var(css, "heading-wrap", &t.heading_wrap);
    write_var(css, "body-wrap", &t.body_wrap);
    write_var(css, "font-features", &t.font_features);
    write_var(css, "optical-sizing", &t.optical_sizing);
}

fn write_spacing_vars(css: &mut String, s: &Spacing) {
    write_var(css, "slide-padding-y", &s.slide_padding_y);
    write_var(css, "slide-padding-x", &s.slide_padding_x);
    write_var(css, "content-max-width", &s.content_max_width);
    write_var(css, "stack-gap", &s.stack_gap);
    if let Some(d) = &s.density {
        let scale = match d.as_str() {
            "compact" => "0.85",
            "spacious" => "1.15",
            _ => "1",
        };
        let _ = writeln!(css, "  --sldr-density-scale: {scale};");
    }
}

fn write_shape_vars(css: &mut String, s: &Shape) {
    write_var(css, "radius", &s.radius);
    write_var(css, "radius-sm", &s.radius_sm);
    write_var(css, "radius-lg", &s.radius_lg);
    write_var(css, "border-width", &s.border_width);
    write_var(css, "border-style", &s.border_style);
}

fn write_shadow_vars(css: &mut String, s: &Shadow) {
    write_var(css, "shadow-sm", &s.sm);
    write_var(css, "shadow-md", &s.md);
    write_var(css, "shadow-lg", &s.lg);
}

fn write_motion_vars(css: &mut String, m: &Motion) {
    write_var(css, "transition", &m.transition);
    write_var(css, "easing", &m.easing);
    write_var(css, "duration", &m.duration);
    write_var(css, "duration-base", &m.duration);
    write_var(css, "duration-fast", &m.duration_fast);
    write_var(css, "duration-slow", &m.duration_slow);
    write_var(css, "ease-out-quart", &m.ease_out_quart);
    write_var(css, "ease-out-expo", &m.ease_out_expo);
}

fn write_decoration_vars(css: &mut String, d: &Decoration) {
    // `intensity` doubles as the atmosphere multiplier (0..=1). Flat
    // flavors set 0 to suppress the deck-wide radial-glow backdrop.
    if let Some(i) = d.intensity {
        let _ = writeln!(css, "  --sldr-atmosphere: {i};");
    }
    if let Some(ref accent) = d.accent {
        let _ = writeln!(css, "  --sldr-decoration-accent: {accent};");
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
