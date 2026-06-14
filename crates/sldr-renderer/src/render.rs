//! HTML renderer - compiles slides into a single self-contained HTML file
//!
//! Embeds base.css, presenter.js, flavor CSS, and all slide content
//! into one file with zero external dependencies.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use sldr_core::flavor::Flavor;
use sldr_core::slide::Slide;
use tracing::info;

use crate::markdown::{render_markdown, MediaConfig};
use crate::media::{self, ImageMode, MediaEmbed};
use crate::layout::{wrap_slide, Chrome, LayoutRegistry, SlideOpts};

/// Validate horizontal alignment value, dropping unknown ones.
fn sanitize_align(v: Option<&str>) -> Option<&'static str> {
    match v {
        Some("left") => Some("left"),
        Some("center") => Some("center"),
        Some("right") => Some("right"),
        _ => None,
    }
}

/// Validate vertical alignment value, dropping unknown ones.
fn sanitize_valign(v: Option<&str>) -> Option<&'static str> {
    match v {
        Some("top") => Some("top"),
        Some("center") => Some("center"),
        Some("bottom") => Some("bottom"),
        _ => None,
    }
}

/// Base CSS embedded at compile time from assets/base.css
const BASE_CSS: &str = include_str!("../assets/base.css");

/// Presenter JS embedded at compile time from assets/presenter.js
const PRESENTER_JS: &str = include_str!("../assets/presenter.js");

/// Animated background effects (pure CSS, deterministic, baked particle
/// positions). Emitted inside the owning flavor's <style data-flavor>
/// block so effects switch with the flavor at runtime, exactly like
/// every other style-layer property (ADR-0003/0005).
const EFFECTS: &[(&str, &str)] = &[
    ("aurora", include_str!("../assets/effects/aurora.css")),
    ("bokeh", include_str!("../assets/effects/bokeh.css")),
    ("grain", include_str!("../assets/effects/grain.css")),
    ("grid-pan", include_str!("../assets/effects/grid-pan.css")),
    ("spotlight", include_str!("../assets/effects/spotlight.css")),
    ("stardust", include_str!("../assets/effects/stardust.css")),
];

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

    /// Languages to embed (ADR-0007 axis). Empty renders the deck default
    /// language; one entry renders that language; several entries embed
    /// every listed language as parallel `data-lang` slide variants with a
    /// runtime toggle — the first entry is active.
    pub languages: Vec<String>,

    /// Deck default language — what `::lang::`-blocked slides fall back to
    /// when the requested language is missing (with a loud warning).
    pub default_language: String,
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
            languages: Vec::new(),
            default_language: "en".to_string(),
        }
    }
}

/// A slide after markdown -> HTML conversion
struct RenderedSlide {
    html: String,
    layout: String,
    /// Language variant this rendering belongs to (multi-language decks
    /// duplicate each slide per embedded language).
    lang: Option<String>,
}

/// Main renderer that compiles everything into a self-contained HTML file
pub struct HtmlRenderer {
    config: RenderConfig,
    flavors: Vec<Flavor>,
    slides: Vec<RenderedSlide>,
    layouts: LayoutRegistry,
    /// Build warnings (e.g. language fallbacks). Warn, never refuse — but
    /// never silently either; the CLI prints these after rendering.
    warnings: Vec<String>,
}

impl HtmlRenderer {
    /// Create a new renderer with the given configuration and the built-in
    /// layout set. Call `load_layouts` to add or override from a user dir.
    #[must_use]
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            flavors: Vec::new(),
            slides: Vec::new(),
            layouts: LayoutRegistry::builtin(),
            warnings: Vec::new(),
        }
    }

    /// Build warnings accumulated while adding slides (language fallbacks
    /// etc.). Drain and surface these to the user — warn, never silently.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Load user layouts from a directory, overriding built-ins by name.
    /// Returns how many were loaded; a missing dir loads zero.
    pub fn load_layouts(&mut self, dir: &std::path::Path) -> Result<usize> {
        self.layouts.load_dir(dir)
    }

    /// Names of all available layouts (built-in + user), sorted.
    #[must_use]
    pub fn layout_names(&self) -> Vec<String> {
        self.layouts.names()
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

    /// Add a slide. Parses markdown content and applies its layout.
    ///
    /// Fails loudly when the slide references a layout that exists nowhere
    /// (built-ins or loaded user dirs) — a deck must never silently render
    /// with a substituted structure.
    pub fn add_slide(&mut self, slide: &Slide) -> Result<()> {
        // The embed set: one variant per language. A single-language deck
        // renders one untagged variant; a multi-language deck duplicates
        // every slide per language (uniform per-language page numbering)
        // tagged with data-lang for the runtime toggle.
        let variants: Vec<Option<String>> = if self.config.languages.len() > 1 {
            self.config.languages.iter().cloned().map(Some).collect()
        } else {
            vec![self.config.languages.first().cloned()]
        };
        let multi = variants.len() > 1;

        for variant in variants {
            self.add_slide_variant(slide, variant.as_deref().filter(|_| multi), variant.as_deref())?;
        }
        Ok(())
    }

    /// Render one language variant of a slide. `tag` becomes the section's
    /// data-lang attribute (None on single-language decks); `request` is
    /// the language selected from in-file ::lang:xx:: blocks.
    fn add_slide_variant(
        &mut self,
        slide: &Slide,
        tag: Option<&str>,
        request: Option<&str>,
    ) -> Result<()> {
        let layout = slide.metadata.layout.as_deref().unwrap_or("default");

        // Per-language slide index so each language's deck numbers 1..N.
        let index = self
            .slides
            .iter()
            .filter(|s| s.lang.as_deref() == tag)
            .count();

        // Select the variant's language from in-file ::lang:xx:: blocks.
        // A gap falls back to the deck default — loudly, never silently.
        let lang_sel = sldr_core::lang::select_language(
            &slide.content,
            request,
            &self.config.default_language,
        );
        if let sldr_core::lang::LanguageOutcome::Fallback {
            requested,
            used,
            available,
        } = &lang_sel.outcome
        {
            self.warnings.push(format!(
                "Slide '{}' has no '{requested}' content — using '{used}' \
                 (available: {})",
                slide.name,
                available.join(", ")
            ));
        }
        let content = lang_sel.content;

        // Parse speaker notes from content (<!-- notes: ... --> convention)
        let notes = extract_speaker_notes(&content);

        // Build media config from renderer config and slide path
        let assets_dir = self.config.output_dir.as_ref().map(|d| d.join("assets"));
        let slide_dir = slide.path.parent().map(std::path::Path::to_path_buf);

        let media_config = MediaConfig {
            image_mode: self.config.image_mode,
            slide_dir,
            assets_dir,
        };

        // Render markdown to HTML with media embedding
        let rendered = render_markdown(&content, &media_config);

        // Per-slide alignment overrides (orthogonal to layout). Validated
        // here so a typo in frontmatter doesn't ship a silently broken
        // attribute selector.
        let align = sanitize_align(slide.metadata.align.as_deref());
        let valign = sanitize_valign(slide.metadata.valign.as_deref());

        // Chrome: persistent deck framing fed from frontmatter + flavor,
        // not the markdown body. Footer resolves slide override over the
        // flavor default; the source line becomes a link when source_url
        // is set (ADR-0008).
        // Pre-render each chrome element with a standard class so framed
        // layouts place it bare (`{{headline}}` alone on a line collapses
        // when empty) and flavors style it by class.
        let flavor_footer = self.flavors.first().and_then(|f| f.footer.as_deref());
        let chrome = Chrome {
            headline: slide.metadata.title.as_deref().map(|t| {
                format!("<h1 class=\"sldr-headline\">{}</h1>", html_escape_text(t))
            }),
            subheadline: slide.metadata.subtitle.as_deref().map(|t| {
                format!("<p class=\"sldr-subheadline\">{}</p>", html_escape_text(t))
            }),
            footer: slide
                .metadata
                .footer
                .as_deref()
                .or(flavor_footer)
                .map(|t| format!("<div class=\"sldr-footer\">{}</div>", html_escape_text(t))),
            source: slide.metadata.source.as_deref().map(|s| {
                format!(
                    "<div class=\"sldr-source\">{}</div>",
                    render_source(s, slide.metadata.source_url.as_deref())
                )
            }),
        };

        // Wrap in the slide's layout (fail loud on an unknown layout —
        // the error names the slide and everything that was searched).
        let def = self
            .layouts
            .resolve(layout)
            .with_context(|| format!("Slide '{}'", slide.name))?;
        let html = wrap_slide(
            SlideOpts {
                index,
                layout,
                align,
                valign,
                lang: tag,
                rendered,
                speaker_notes: notes.as_deref(),
                chrome,
            },
            def,
        );

        self.slides.push(RenderedSlide {
            html,
            layout: layout.to_string(),
            lang: tag.map(str::to_string),
        });
        Ok(())
    }

    /// Add multiple slides in order
    pub fn add_slides(&mut self, slides: &[Slide]) -> Result<()> {
        for slide in slides {
            self.add_slide(slide)?;
        }
        Ok(())
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

        // Fonts: embed each flavor-declared web font as inline @font-face
        // (base64 woff2) so the deck is self-contained and renders without
        // a network round-trip — no fallback-then-swap flash, works offline
        // (ADR-0006). Build-time fetch is cached; a font that can't be
        // fetched degrades to a plain <link> (online-only) instead of
        // failing the build. There is no hardcoded default font set — a
        // flavor that wants a web font declares it in font_imports; one
        // that uses system fonts (no imports) pulls nothing.
        let mut seen_imports: std::collections::HashSet<&str> =
            std::collections::HashSet::new();
        for flavor in &self.flavors {
            for url in &flavor.font_imports {
                if !seen_imports.insert(url.as_str()) {
                    continue;
                }
                // Local stylesheet shipped by the flavor (no http) → embed
                // its local font files; otherwise fetch from the network.
                let embedded = if url.starts_with("http") {
                    crate::fonts::embed_font_css(url)
                } else if let Some(dir) = flavor.source_dir.as_deref() {
                    crate::fonts::embed_local_font_css(dir, url)
                } else {
                    None
                };
                match embedded {
                    Some(css) => {
                        html.push_str("  <style data-font-embed>\n");
                        html.push_str(&css);
                        html.push_str("\n  </style>\n");
                    }
                    None => {
                        eprintln!(
                            "  ! Could not embed font (using a network <link>; \
                             deck will need internet): {url}"
                        );
                        let _ = writeln!(
                            html,
                            "  <link href=\"{}\" rel=\"stylesheet\">",
                            html_escape_attr(url)
                        );
                    }
                }
            }
        }

        // Base CSS (inlined)
        html.push_str("  <style>\n");
        html.push_str(BASE_CSS);
        html.push_str("\n  </style>\n");

        // Scoped CSS of the layouts actually used by this deck, in
        // first-use order, once each. Built-ins carry no scoped CSS (their
        // rules live in base.css); this is the user-layout channel.
        let mut seen_layouts: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for slide in &self.slides {
            if !seen_layouts.insert(slide.layout.as_str()) {
                continue;
            }
            if let Some(css) = self.layouts.get(&slide.layout).and_then(|d| d.css.as_deref()) {
                let _ = writeln!(
                    html,
                    "  <style data-layout-css=\"{}\">\n{}\n  </style>",
                    html_escape_attr(&slide.layout),
                    css
                );
            }
        }

        // Flavor styles
        self.write_flavor_styles(&mut html);

        html.push_str("</head>\n<body>\n");

        // Slide deck
        let any_effect = self
            .flavors
            .iter()
            .any(|f| f.decoration.effect.is_some());
        if self.config.languages.len() > 1 {
            let _ = writeln!(
                html,
                "  <div class=\"sldr-deck\" data-transition=\"{}\" data-langs=\"{}\">",
                html_escape_attr(&self.config.transition),
                html_escape_attr(&self.config.languages.join(","))
            );
        } else {
            let _ = writeln!(
                html,
                "  <div class=\"sldr-deck\" data-transition=\"{}\">",
                html_escape_attr(&self.config.transition)
            );
        }

        // Decoration-effect layer: engine-owned, content-free, inert
        // (pointer-events: none, aria-hidden). Which effect renders — or
        // none — is decided purely by the active flavor's CSS.
        if any_effect {
            html.push_str("    <div class=\"sldr-fx\" aria-hidden=\"true\"></div>\n");
        }
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

    /// Background CSS with image assets embedded for self-containment.
    /// For `image`/`svg` backgrounds the value is a file in the flavor's
    /// assets dir; resolve and embed it (data URI or asset-file copy) so
    /// there is no dangling `url('/…')`. Other background types pass through.
    fn embed_background(&self, flavor: &Flavor) -> String {
        let is_image = matches!(
            flavor.background.background_type.as_deref(),
            Some("image" | "svg")
        );
        let value = flavor.background.value.as_deref();
        let (Some(value), true) = (value, is_image) else {
            return flavor.to_background_css();
        };
        // Already a URL — leave it (author opted into an external ref).
        if value.starts_with("http") || value.starts_with("data:") {
            return flavor.to_background_css();
        }
        let Some(assets) = flavor.source_dir.as_ref().map(|d| d.join("assets")) else {
            return flavor.to_background_css();
        };
        let path = assets.join(value.trim_start_matches('/'));
        if !path.exists() {
            eprintln!(
                "  ! Flavor '{}': background image not found: {}",
                flavor.name,
                path.display()
            );
            return String::new();
        }
        let embed = media::process_media_src(
            &path.to_string_lossy(),
            assets.parent(),
            self.config.image_mode,
            self.config.output_dir.as_ref().map(|d| d.join("assets")).as_deref(),
        );
        let src = match embed {
            MediaEmbed::DataUri(uri) => uri,
            MediaEmbed::External(url) => url,
            MediaEmbed::AssetFile { html_src, .. } => html_src,
            MediaEmbed::NotFound(_) => return String::new(),
        };
        let mut css = format!(
            ".sldr-slide {{ background-image: url('{src}'); background-size: cover; background-position: center; }}\n"
        );
        if let Some(op) = flavor.background.opacity {
            if op < 1.0 {
                let _ = write!(
                    css,
                    ".sldr-slide::before {{ content: ''; position: absolute; inset: 0; background: inherit; opacity: {op}; z-index: -1; }}\n"
                );
            }
        }
        css
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

            // Background CSS. For image/svg backgrounds, embed the asset
            // (data URI / asset file) so the output stays self-contained —
            // to_background_css emits a bare url() that has no server to
            // resolve against. Color/gradient backgrounds pass through.
            let bg_css = self.embed_background(flavor);
            if !bg_css.is_empty() {
                html.push_str(&bg_css);
            }

            // Syntax-highlighting colors from [code] syntax_theme. Code
            // markup carries class-based syn-* spans (no inline styles),
            // so highlighting lives in the flavor's style layer and swaps
            // with the flavor at runtime (trx-e9bd, ADR-0003).
            html.push_str(&syntax_theme_css(flavor.code.syntax_theme.as_deref()));

            // Animated background effect (decoration.effect) — part of
            // this flavor's style block so the T toggle swaps it.
            if let Some(effect) = flavor.decoration.effect.as_deref() {
                match EFFECTS.iter().find(|(name, _)| *name == effect) {
                    Some((_, css)) => {
                        html.push('\n');
                        html.push_str(css);
                    }
                    None => {
                        let known: Vec<&str> = EFFECTS.iter().map(|(n, _)| *n).collect();
                        // Warn, never refuse — but never silently.
                        eprintln!(
                            "  ! Flavor '{}': unknown decoration.effect '{effect}' (known: {})",
                            flavor.name,
                            known.join(", ")
                        );
                    }
                }
            }

            // Per-flavor escape-hatch CSS (loaded from flavor.css)
            if let Some(ref custom) = flavor.custom_css {
                html.push('\n');
                html.push_str(custom);
                if !custom.ends_with('\n') {
                    html.push('\n');
                }
            }

            html.push_str("  </style>\n");
        }
    }
}

/// CSS rules for a syntect theme, scoped to the class-based syn-* spans.
/// Unknown or missing theme names fall back to base16-ocean.dark. The
/// `.syn-code` background is suppressed — the `--sldr-code-background`
/// token owns the code-block surface.
fn syntax_theme_css(theme_name: Option<&str>) -> String {
    use syntect::highlighting::ThemeSet;
    use syntect::html::css_for_theme_with_class_style;

    let ts = ThemeSet::load_defaults();
    let name = theme_name
        .filter(|n| ts.themes.contains_key(*n))
        .unwrap_or("base16-ocean.dark");
    let css = css_for_theme_with_class_style(&ts.themes[name], crate::markdown::SYN_CLASS_STYLE)
        .unwrap_or_default();
    format!("{css}\n.sldr-code .syn-code {{ background: transparent; }}\n")
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

/// Escape text for safe placement in element content (chrome slots carry
/// plain frontmatter text, never markdown).
fn html_escape_text(input: &str) -> String {
    input.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Render the web-clipping source line: "Source: …", linked when a URL is
/// given. Self-contained — the link is inert until clicked.
fn render_source(text: &str, url: Option<&str>) -> String {
    let label = html_escape_text(text);
    match url {
        Some(u) => format!(
            "<span class=\"sldr-source-label\">Source:</span> <a href=\"{}\">{label}</a>",
            html_escape_attr(u)
        ),
        None => format!("<span class=\"sldr-source-label\">Source:</span> {label}"),
    }
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
