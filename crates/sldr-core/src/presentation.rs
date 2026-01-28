//! Presentation management - collections of slides assembled for a talk

use crate::error::Result;
use crate::flavor::Flavor;
use crate::slide::Slide;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use tracing::info;

/// A presentation skeleton - defines which slides to include
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Slidev configuration overrides
    #[serde(default)]
    pub slidev_config: SlidevConfig,
}

/// Slidev-specific configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlidevConfig {
    /// Theme to use
    #[serde(default)]
    pub theme: Option<String>,

    /// Enable/disable drawing feature
    #[serde(default)]
    pub drawings: Option<bool>,

    /// Transition effect
    #[serde(default)]
    pub transition: Option<String>,

    /// Title override
    #[serde(default)]
    pub title: Option<String>,

    /// Enable dark mode
    #[serde(default)]
    pub dark_mode: Option<bool>,

    /// Aspect ratio (e.g., "16/9", "4/3")
    #[serde(default)]
    pub aspect_ratio: Option<String>,

    /// Canvas width
    #[serde(default)]
    pub canvas_width: Option<u32>,

    /// Enable slide recording
    #[serde(default)]
    pub record: Option<bool>,
}

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
}

/// A fully assembled presentation ready for slidev
#[derive(Debug)]
pub struct Presentation {
    /// Name of the presentation
    pub name: String,

    /// Title for display
    pub title: String,

    /// Resolved slides in order
    pub slides: Vec<Slide>,

    /// Flavor being used
    pub flavor: Option<Flavor>,

    /// Slidev config
    pub slidev_config: SlidevConfig,

    /// Output directory for this presentation
    pub output_dir: PathBuf,
}

impl Presentation {
    /// Generate the slidev frontmatter YAML
    fn generate_frontmatter(&self) -> String {
        let mut fm = String::from("---\n");

        // Theme
        let theme = self
            .slidev_config
            .theme
            .as_deref()
            .unwrap_or("default");
        let _ = writeln!(fm, "theme: {theme}");

        // Title
        let _ = writeln!(fm, "title: \"{}\"", self.title);

        // Drawings
        if let Some(drawings) = self.slidev_config.drawings {
            let _ = writeln!(
                fm,
                "drawings:\n  persist: {}",
                if drawings { "true" } else { "false" }
            );
        }

        // Transition
        if let Some(ref transition) = self.slidev_config.transition {
            let _ = writeln!(fm, "transition: {transition}");
        }

        // Aspect ratio
        if let Some(ref ratio) = self.slidev_config.aspect_ratio {
            let _ = writeln!(fm, "aspectRatio: \"{ratio}\"");
        }

        // Canvas width
        if let Some(width) = self.slidev_config.canvas_width {
            let _ = writeln!(fm, "canvasWidth: {width}");
        }

        // Apply flavor colors to CSS if available
        if self.flavor.is_some() {
            fm.push_str("css: unocss\n");
        }

        fm.push_str("---\n");
        fm
    }

    /// Generate the combined markdown for slidev
    pub fn to_slidev_markdown(&self) -> String {
        let mut output = self.generate_frontmatter();
        output.push('\n');

        // Add each slide
        for (i, slide) in self.slides.iter().enumerate() {
            if i > 0 {
                output.push_str("\n---\n\n");
            }

            // If slide has layout in metadata, add it
            if let Some(ref layout) = slide.metadata.layout {
                let _ = writeln!(output, "layout: {layout}\n");
            }

            output.push_str(&slide.content);
        }

        output
    }

    /// Generate package.json for the slidev project
    fn generate_package_json(&self) -> String {
        serde_json::json!({
            "name": format!("sldr-{}", self.name),
            "type": "module",
            "private": true,
            "scripts": {
                "dev": "slidev --open",
                "build": "slidev build",
                "export": "slidev export",
                "export-pdf": "slidev export --format pdf",
                "export-pptx": "slidev export --format pptx"
            },
            "dependencies": {
                "@slidev/cli": "^52.0.0",
                "@slidev/theme-default": "latest",
                "@slidev/theme-seriph": "latest",
                "vue": "^3.5.0"
            }
        })
        .to_string()
    }

    /// Generate custom CSS from flavor
    fn generate_style_css(&self) -> String {
        let mut css = String::new();

        if let Some(ref flavor) = self.flavor {
            // Add CSS variables from flavor
            css.push_str(&flavor.to_css_variables());
            css.push('\n');

            // Apply variables to slidev elements
            css.push_str(
                r"
/* Apply sldr flavor variables */
.slidev-layout {
  --slidev-theme-primary: var(--sldr-primary, #3b82f6);
  --slidev-theme-secondary: var(--sldr-secondary, #6366f1);
}

.slidev-page {
  background: var(--sldr-background, #ffffff);
  color: var(--sldr-text, #1f2937);
}

h1, h2, h3, h4, h5, h6 {
  font-family: var(--sldr-heading-font, inherit);
  color: var(--sldr-primary, inherit);
}

p, li, span {
  font-family: var(--sldr-body-font, inherit);
}

code, pre {
  font-family: var(--sldr-code-font, 'JetBrains Mono', monospace);
  background: var(--sldr-code-background, #f3f4f6);
}

a {
  color: var(--sldr-accent, var(--sldr-primary, #3b82f6));
}
",
            );

            // Add background styles if configured
            if let Some(ref bg_type) = flavor.background.background_type {
                if let Some(ref value) = flavor.background.value {
                    match bg_type.as_str() {
                        "color" => {
                            let _ = writeln!(
                                css,
                                "\n.slidev-page {{ background-color: {value} !important; }}"
                            );
                        }
                        "gradient" => {
                            let _ = writeln!(
                                css,
                                "\n.slidev-page {{ background: {value} !important; }}"
                            );
                        }
                        "image" => {
                            let _ = writeln!(
                                css,
                                "\n.slidev-page {{ background-image: url('{value}'); background-size: cover; background-position: center; }}"
                            );
                        }
                        _ => {}
                    }
                }
            }
        }

        css
    }

    /// Write the presentation as a complete slidev project
    pub fn write(&self) -> Result<()> {
        info!("Writing presentation to {:?}", self.output_dir);

        // Create output directory structure
        std::fs::create_dir_all(&self.output_dir)?;
        std::fs::create_dir_all(self.output_dir.join("public"))?;

        // Write slides.md
        let slides_path = self.output_dir.join("slides.md");
        let content = self.to_slidev_markdown();
        std::fs::write(&slides_path, content)?;
        info!("Wrote slides.md");

        // Write package.json
        let package_json_path = self.output_dir.join("package.json");
        let package_json = self.generate_package_json();
        std::fs::write(&package_json_path, package_json)?;
        info!("Wrote package.json");

        // Write custom styles
        let style_css = self.generate_style_css();
        if !style_css.is_empty() {
            let style_path = self.output_dir.join("style.css");
            std::fs::write(&style_path, style_css)?;
            info!("Wrote style.css");
        }

        // Copy flavor assets if available
        if let Some(ref flavor) = self.flavor {
            self.copy_flavor_assets(flavor)?;
        }

        // Write .gitignore
        let gitignore_content = "node_modules\ndist\n.slidev\n";
        std::fs::write(self.output_dir.join(".gitignore"), gitignore_content)?;

        Ok(())
    }

    /// Copy assets from flavor directory to presentation
    fn copy_flavor_assets(&self, flavor: &Flavor) -> Result<()> {
        if let Some(ref assets_dir) = flavor.assets_dir {
            let assets_path = crate::config::Config::expand_path(assets_dir);
            if assets_path.exists() {
                let dest = self.output_dir.join("public").join("assets");
                std::fs::create_dir_all(&dest)?;
                copy_dir_recursive(&assets_path, &dest)?;
                info!("Copied flavor assets to public/assets");
            }
        }
        Ok(())
    }
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Builder for creating presentations from skeletons
pub struct PresentationBuilder {
    name: String,
    title: Option<String>,
    slides: Vec<Slide>,
    flavor: Option<Flavor>,
    slidev_config: SlidevConfig,
    output_dir: PathBuf,
}

impl PresentationBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            title: None,
            name,
            slides: Vec::new(),
            flavor: None,
            slidev_config: SlidevConfig::default(),
            output_dir: PathBuf::new(),
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn flavor(mut self, flavor: Flavor) -> Self {
        self.flavor = Some(flavor);
        self
    }

    #[must_use]
    pub fn slidev_config(mut self, config: SlidevConfig) -> Self {
        self.slidev_config = config;
        self
    }

    #[must_use]
    pub fn output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = dir.into();
        self
    }

    #[must_use]
    pub fn add_slide(mut self, slide: Slide) -> Self {
        self.slides.push(slide);
        self
    }

    #[must_use]
    pub fn add_slides(mut self, slides: impl IntoIterator<Item = Slide>) -> Self {
        self.slides.extend(slides);
        self
    }

    pub fn build(self) -> Presentation {
        let title = self.title.unwrap_or_else(|| self.name.clone());
        Presentation {
            name: self.name,
            title,
            slides: self.slides,
            flavor: self.flavor,
            slidev_config: self.slidev_config,
            output_dir: self.output_dir,
        }
    }
}
