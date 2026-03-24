//! HTML renderer - compiles slides into a single self-contained HTML file
//!
//! Embeds base.css, presenter.js, flavor CSS, and all slide content
//! into one file with zero external dependencies.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use sldr_core::flavor::Flavor;
use sldr_core::slide::Slide;
use tracing::info;

use crate::markdown::{render_markdown, MediaConfig};
use crate::media::{self, ImageMode, MediaEmbed};
use crate::template::wrap_slide;

/// Base CSS embedded at compile time from assets/base.css
const BASE_CSS: &str = include_str!("../assets/base.css");

/// Presenter JS embedded at compile time from assets/presenter.js
const PRESENTER_JS: &str = include_str!("../assets/presenter.js");

/// Configuration for the HTML renderer
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Presentation title
    pub title: String,

    /// Transition style ("fade", "slide-left", "slide-right", "none")
    pub transition: String,

    /// Aspect ratio hint for PDF/PPTX export viewport. The HTML output is
    /// fully responsive and fills the browser viewport regardless of this value.
    pub aspect_ratio: String,

    /// Whether to include speaker notes support
    pub speaker_notes: bool,

    /// How to handle local images in slides
    pub image_mode: ImageMode,

    /// Output directory (used for creating assets/ subdirectory in external mode)
    pub output_dir: Option<std::path::PathBuf>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            title: "Presentation".to_string(),
            transition: "fade".to_string(),
            aspect_ratio: "16/9".to_string(),
            speaker_notes: true,
            image_mode: ImageMode::Embed,
            output_dir: None,
        }
    }
}

/// A slide after markdown -> HTML conversion
struct RenderedSlide {
    html: String,
    layout: String,
}

/// Main renderer that compiles everything into a self-contained HTML file
pub struct HtmlRenderer {
    config: RenderConfig,
    flavors: Vec<Flavor>,
    slides: Vec<RenderedSlide>,
}

impl HtmlRenderer {
    /// Create a new renderer with the given configuration
    #[must_use]
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            flavors: Vec::new(),
            slides: Vec::new(),
        }
    }

    /// Add a single flavor. The first flavor added is the active default.
    #[must_use]
    pub fn add_flavor(mut self, flavor: Flavor) -> Self {
        self.flavors.push(flavor);
        self
    }

    /// Add multiple flavors. The first is active by default.
    #[must_use]
    pub fn add_flavors(mut self, flavors: impl IntoIterator<Item = Flavor>) -> Self {
        self.flavors.extend(flavors);
        self
    }

    /// Add a slide. Parses markdown content and applies layout template.
    pub fn add_slide(&mut self, slide: &Slide) {
        let layout = slide
            .metadata
            .layout
            .as_deref()
            .unwrap_or("default");

        let index = self.slides.len();

        // Parse speaker notes from content (<!-- notes: ... --> convention)
        let notes = extract_speaker_notes(&slide.content);

        // Build media config from renderer config and slide path
        let assets_dir = self.config.output_dir.as_ref().map(|d| d.join("assets"));
        let slide_dir = slide.path.parent().map(std::path::Path::to_path_buf);

        let media_config = MediaConfig {
            image_mode: self.config.image_mode,
            slide_dir,
            assets_dir,
        };

        // Render markdown to HTML with media embedding
        let rendered = render_markdown(&slide.content, &media_config);

        // Wrap in layout template
        let html = wrap_slide(index, layout, rendered, notes.as_deref());

        self.slides.push(RenderedSlide {
            html,
            layout: layout.to_string(),
        });
    }

    /// Add multiple slides in order
    pub fn add_slides(&mut self, slides: &[Slide]) {
        for slide in slides {
            self.add_slide(slide);
        }
    }

    /// Compile everything into a single self-contained HTML string
    pub fn render(&self) -> Result<String> {
        let mut html = String::with_capacity(64 * 1024);

        // DOCTYPE and head
        html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n");
        html.push_str("  <meta charset=\"UTF-8\">\n");
        html.push_str(
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
        );
        let _ = writeln!(
            html,
            "  <title>{}</title>",
            html_escape_attr(&self.config.title)
        );
        html.push_str("  <meta name=\"generator\" content=\"sldr\">\n");

        // Google Fonts - default font pairing (flavor can override via CSS vars)
        html.push_str("  <link rel=\"preconnect\" href=\"https://fonts.googleapis.com\">\n");
        html.push_str("  <link rel=\"preconnect\" href=\"https://fonts.gstatic.com\" crossorigin>\n");
        html.push_str("  <link href=\"https://fonts.googleapis.com/css2?family=DM+Sans:ital,opsz,wght@0,9..40,300..700;1,9..40,300..700&family=Fira+Code:wght@400;500;600&family=Instrument+Serif:ital@0;1&display=swap\" rel=\"stylesheet\">\n");

        // Base CSS (inlined)
        html.push_str("  <style>\n");
        html.push_str(BASE_CSS);
        html.push_str("\n  </style>\n");

        // Flavor styles
        self.write_flavor_styles(&mut html);

        html.push_str("</head>\n<body>\n");

        // Slide deck
        let _ = writeln!(
            html,
            "  <div class=\"sldr-deck\" data-transition=\"{}\">",
            html_escape_attr(&self.config.transition)
        );
        html.push('\n');

        // All slides (with logo injection)
        for slide in &self.slides {
            let slide_html = self.inject_logos(&slide.html, &slide.layout);
            html.push_str("    ");
            html.push_str(&slide_html);
            html.push('\n');
        }

        html.push_str("  </div>\n\n");

        // Progress bar and nav
        html.push_str("  <div class=\"sldr-progress\" style=\"width: 0%\"></div>\n");
        html.push_str("  <div class=\"sldr-nav\">\n");
        let _ = writeln!(
            html,
            "    <span class=\"sldr-page-num\">1 / {}</span>",
            self.slides.len()
        );
        html.push_str("  </div>\n\n");

        // Presenter JS (inlined)
        html.push_str("  <script>\n");
        html.push_str(PRESENTER_JS);
        html.push_str("\n  </script>\n");

        html.push_str("</body>\n</html>\n");

        Ok(html)
    }

    /// Render and write directly to a file
    pub fn render_to_file(&self, path: &Path) -> Result<()> {
        let html = self.render()?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(path, &html)?;
        info!("Wrote presentation to {}", path.display());

        Ok(())
    }

    /// Generate logo overlay HTML for a specific slide layout.
    ///
    /// Resolves logo files from the flavor's assets directory, embeds them
    /// (as WebP/SVG data URIs or external refs based on image_mode), and
    /// returns positioned `<img>` tags.
    fn generate_logo_html(&self, layout: &str) -> String {
        let Some(flavor) = self.flavors.first() else {
            return String::new();
        };

        if flavor.logos.is_empty() {
            return String::new();
        }

        let assets_dir = flavor
            .source_dir
            .as_ref()
            .map(|d| d.join("assets"));

        let mut logo_html = String::new();

        for logo in &flavor.logos {
            if !logo.applies_to_layout(layout) {
                continue;
            }

            // Resolve the logo file from the flavor's assets directory
            let logo_src = if let Some(ref assets) = assets_dir {
                let logo_path = assets.join(&logo.file);
                if logo_path.exists() {
                    let embed = media::process_media_src(
                        &logo_path.to_string_lossy(),
                        assets.parent(),
                        self.config.image_mode,
                        self.config.output_dir.as_ref().map(|d| d.join("assets")).as_deref(),
                    );
                    match embed {
                        MediaEmbed::DataUri(uri) => uri,
                        MediaEmbed::External(url) => url,
                        MediaEmbed::AssetFile { html_src, .. } => html_src,
                        MediaEmbed::NotFound(_) => continue,
                    }
                } else {
                    tracing::warn!("Logo file not found: {}", logo_path.display());
                    continue;
                }
            } else {
                // No assets dir, try the file path directly
                logo.file.clone()
            };

            let style = logo.to_css_position();
            let _ = writeln!(
                logo_html,
                "    <img class=\"sldr-logo\" src=\"{logo_src}\" alt=\"\" style=\"{style}\">"
            );
        }

        logo_html
    }

    /// Inject logo overlays into a slide's HTML (before the closing </section>)
    fn inject_logos(&self, slide_html: &str, layout: &str) -> String {
        let logo_html = self.generate_logo_html(layout);
        if logo_html.is_empty() {
            return slide_html.to_string();
        }

        // Insert logo HTML before </section>
        if let Some(pos) = slide_html.rfind("</section>") {
            let mut result = String::with_capacity(slide_html.len() + logo_html.len());
            result.push_str(&slide_html[..pos]);
            result.push_str(&logo_html);
            result.push_str(&slide_html[pos..]);
            result
        } else {
            slide_html.to_string()
        }
    }

    /// Write flavor <style> blocks into the head
    fn write_flavor_styles(&self, html: &mut String) {
        if self.flavors.is_empty() {
            return;
        }

        for (i, flavor) in self.flavors.iter().enumerate() {
            let name = flavor
                .display_name
                .as_deref()
                .unwrap_or(&flavor.name);

            if i == 0 {
                // First flavor is active (no disabled attribute)
                let _ = writeln!(html, "  <style data-flavor=\"{}\">", html_escape_attr(name));
            } else {
                let _ = writeln!(
                    html,
                    "  <style data-flavor=\"{}\" disabled>",
                    html_escape_attr(name)
                );
            }

            // CSS custom properties
            html.push_str(&flavor.to_css_variables());

            // Background CSS
            let bg_css = flavor.to_background_css();
            if !bg_css.is_empty() {
                html.push_str(&bg_css);
            }

            html.push_str("  </style>\n");
        }
    }
}

/// Extract speaker notes from slide content.
///
/// Notes can be placed after a `<!-- notes -->` HTML comment, or inside
/// a `<!-- notes: ... -->` inline comment.
fn extract_speaker_notes(content: &str) -> Option<String> {
    // Pattern 1: <!-- notes --> followed by content until end or next ---
    if let Some(idx) = content.find("<!-- notes -->") {
        let after = &content[idx + "<!-- notes -->".len()..];
        let notes = after.trim();
        if !notes.is_empty() {
            return Some(notes.to_string());
        }
    }

    // Pattern 2: <!-- notes: some inline note -->
    if let Some(start) = content.find("<!-- notes:") {
        if let Some(end) = content[start..].find("-->") {
            let notes = &content[start + "<!-- notes:".len()..start + end];
            let notes = notes.trim();
            if !notes.is_empty() {
                return Some(notes.to_string());
            }
        }
    }

    None
}

/// Escape a string for use in an HTML attribute value
fn html_escape_attr(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_notes_block() {
        let content = "# Title\n\n<!-- notes -->\nThese are my notes";
        let notes = extract_speaker_notes(content);
        assert_eq!(notes, Some("These are my notes".to_string()));
    }

    #[test]
    fn test_extract_notes_inline() {
        let content = "# Title\n<!-- notes: Quick reminder about X -->";
        let notes = extract_speaker_notes(content);
        assert_eq!(notes, Some("Quick reminder about X".to_string()));
    }

    #[test]
    fn test_no_notes() {
        let content = "# Title\n\nJust content";
        let notes = extract_speaker_notes(content);
        assert!(notes.is_none());
    }

    #[test]
    fn test_html_escape_attr() {
        assert_eq!(html_escape_attr("A & B"), "A &amp; B");
        assert_eq!(html_escape_attr("say \"hi\""), "say &quot;hi&quot;");
    }

    #[test]
    fn test_render_empty() {
        let renderer = HtmlRenderer::new(RenderConfig::default());
        let html = renderer.render().unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("sldr-deck"));
        assert!(html.contains("sldr-progress"));
    }

    #[test]
    fn test_render_config() {
        let config = RenderConfig {
            title: "My Talk".to_string(),
            transition: "slide-left".to_string(),
            ..Default::default()
        };
        let renderer = HtmlRenderer::new(config);
        let html = renderer.render().unwrap();
        assert!(html.contains("<title>My Talk</title>"));
        assert!(html.contains("data-transition=\"slide-left\""));
    }

    #[test]
    fn test_single_flavor_no_disabled() {
        let flavor = Flavor {
            name: "test".to_string(),
            display_name: Some("Test Flavor".to_string()),
            ..Default::default()
        };
        let renderer = HtmlRenderer::new(RenderConfig::default()).add_flavor(flavor);
        let html = renderer.render().unwrap();
        assert!(html.contains("data-flavor=\"Test Flavor\""));
        // Single flavor should NOT have disabled on its style tag
        assert!(!html.contains("data-flavor=\"Test Flavor\" disabled"));
    }

    #[test]
    fn test_multi_flavor_disabled() {
        let f1 = Flavor {
            name: "a".to_string(),
            display_name: Some("Alpha".to_string()),
            ..Default::default()
        };
        let f2 = Flavor {
            name: "b".to_string(),
            display_name: Some("Beta".to_string()),
            ..Default::default()
        };
        let renderer = HtmlRenderer::new(RenderConfig::default())
            .add_flavor(f1)
            .add_flavor(f2);
        let html = renderer.render().unwrap();
        // First flavor active
        assert!(html.contains("data-flavor=\"Alpha\">"));
        // Second flavor disabled
        assert!(html.contains("data-flavor=\"Beta\" disabled>"));
    }
}
