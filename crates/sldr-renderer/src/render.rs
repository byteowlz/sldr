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

use crate::markdown::{render_markdown, MarkdownOutput, MediaConfig};
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

/// Mermaid v10.9.3 (UMD build, MIT) embedded at compile time. ~3 MB, so it is
/// only inlined into decks that actually contain a ```mermaid block — see
/// `render`. Keeps the deck self-contained: diagrams render client-side at
/// present time with no network.
const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");

/// The bundled mermaid.js source — for rendering diagrams outside the presenter
/// (e.g. baking a diagram to an image for PPTX export).
pub fn mermaid_js() -> &'static str {
    MERMAID_JS
}

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

    /// Aspect-lock: render the deck as a centered, letterboxed 16:9 box
    /// instead of filling the browser window. Off by default (fill-window).
    /// On → the on-screen slide is the exact shape that gets exported, so
    /// browser, projector, and PDF all match.
    pub aspect_lock: bool,
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
            aspect_lock: false,
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

    /// Per-layout navigation metadata `(name, category, tags)`, sorted by name.
    pub fn layout_catalog(&self) -> Vec<(String, Option<String>, Vec<String>)> {
        self.layouts.catalog()
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
        // Strip split markers that won't form a recognized pair (a lone
        // ::content::, markers on a single-block layout). They would otherwise
        // render as literal "::content::" text — the most common authoring
        // mistake. Remove them and warn loudly rather than ship visible junk.
        let (content, stray_markers) = crate::markdown::strip_stray_markers(&lang_sel.content);
        if !stray_markers.is_empty() {
            let list = stray_markers
                .iter()
                .map(|m| format!("::{m}::"))
                .collect::<Vec<_>>()
                .join(", ");
            self.warnings.push(format!(
                "Slide '{}' has stray marker(s) {list} that don't form a recognized \
                 pair (::content::+::image:: or ::left::+::right::) — removed from \
                 output. Use a layout/markers that match, or delete them.",
                slide.name
            ));
        }

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
        // Resolve chrome for this variant's language (translations.<lang>
        // overriding the top-level default), then warn loudly if the slide
        // carries chrome but no translation for a non-default language — the
        // headline must never stay in the wrong language silently.
        let resolved = slide
            .metadata
            .chrome_for(request, &self.config.default_language);
        if let Some(target) = &resolved.untranslated_to {
            self.warnings.push(format!(
                "Slide '{}' chrome not translated to '{target}' — showing \
                 default-language headline/subtitle/source",
                slide.name
            ));
        }
        let flavor_footer = self.flavors.first().and_then(|f| f.footer.as_deref());
        // The "Source:" prefix is a built-in UI label, localized by the same
        // active language as the chrome/body (not a frontmatter string).
        let source_label = source_label_for(request, &self.config.default_language);
        let chrome = Chrome {
            headline: resolved.title.as_deref().map(|t| {
                format!("<h1 class=\"sldr-headline\">{}</h1>", html_escape_text(t))
            }),
            subheadline: resolved.subtitle.as_deref().map(|t| {
                format!("<p class=\"sldr-subheadline\">{}</p>", html_escape_text(t))
            }),
            // Slide `footer` overrides the flavor's; an explicit empty
            // (`footer: ""`) suppresses it on this slide (and wins over the
            // flavor default) rather than rendering an empty line.
            footer: resolved
                .footer
                .as_deref()
                .or(flavor_footer)
                .filter(|t| !t.trim().is_empty())
                .map(|t| format!("<div class=\"sldr-footer\">{}</div>", html_escape_text(t))),
            source: resolved.source.as_deref().map(|s| {
                format!(
                    "<div class=\"sldr-source\">{}</div>",
                    render_source(s, resolved.source_url.as_deref(), source_label)
                )
            }),
        };

        // Wrap in the slide's layout (fail loud on an unknown layout —
        // the error names the slide and everything that was searched).
        let def = self
            .layouts
            .resolve(layout)
            .with_context(|| format!("Slide '{}'", slide.name))?;

        // Marker/layout mismatch is a silent trap otherwise: a layout with an
        // image or column slot whose body lacks the matching markers degrades
        // to plain content (image lands in the text area, columns vanish). Warn
        // loudly and actionably instead — the body shape is known here, and so
        // is what the layout expects.
        let expected_markers = match &rendered {
            MarkdownOutput::Single(_) if def.expects_image() => {
                Some("::content:: / ::image::")
            }
            MarkdownOutput::Single(_) if def.expects_columns() => {
                Some("::left:: / ::right::")
            }
            MarkdownOutput::TwoCols { .. } if def.expects_image() => {
                Some("::content:: / ::image::")
            }
            MarkdownOutput::ContentImage { .. } if def.expects_columns() => {
                Some("::left:: / ::right::")
            }
            _ => None,
        };
        if let Some(markers) = expected_markers {
            self.warnings.push(format!(
                "Slide '{}' uses layout '{layout}', which expects {markers} \
                 markers, but the body has none (or the wrong ones) — it \
                 rendered as plain content. Add the markers or pick a layout \
                 that matches the body.",
                slide.name
            ));
        }

        // Persistent bottom chrome (footer + source) shows on the layouts the
        // flavor opts in via `chrome_layouts`; empty (default) = the framed
        // family only, preserving the clean look of cover/statement/image.
        let chrome_overlay = {
            let cfg = self.flavors.first().map(|f| f.chrome_layouts.as_slice());
            match cfg {
                Some(list) if list.iter().any(|l| l == "all") => true,
                Some(list) if !list.is_empty() => list.iter().any(|l| l == layout),
                // Default: the framed *body* family — not the title/divider
                // covers, which are meant to be clean (a flavor can still opt
                // them in via chrome_layouts).
                _ => {
                    def.category.as_deref() == Some("framed")
                        && !matches!(layout, "framed-cover" | "framed-section")
                }
            }
        };

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
                chrome_overlay,
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
        let lock_attr = if self.config.aspect_lock {
            " data-aspect-lock=\"on\""
        } else {
            ""
        };
        if self.config.languages.len() > 1 {
            let _ = writeln!(
                html,
                "  <div class=\"sldr-deck\"{} data-transition=\"{}\" data-langs=\"{}\">",
                lock_attr,
                html_escape_attr(&self.config.transition),
                html_escape_attr(&self.config.languages.join(","))
            );
        } else {
            let _ = writeln!(
                html,
                "  <div class=\"sldr-deck\"{} data-transition=\"{}\">",
                lock_attr,
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

        // All slides (logos are a persistent deck-level overlay, not part
        // of any slide, so they don't transition/flicker on slide change).
        for slide in &self.slides {
            html.push_str("    ");
            html.push_str(&slide.html);
            html.push('\n');
        }

        // Persistent logo overlay (toggled per active layout by the presenter)
        html.push_str(&self.generate_deck_logos());

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

        // Mermaid (inlined only when the deck uses it — it's large). The
        // presenter renders each diagram lazily when its slide is shown, so
        // hidden-slide measurement bugs don't bite. Loaded before the presenter
        // so `window.mermaid` exists when slides are shown.
        if self.slides.iter().any(|s| s.html.contains("sldr-mermaid")) {
            html.push_str("  <script>\n");
            html.push_str(MERMAID_JS);
            html.push_str("\n  </script>\n");
            // presenter.js initializes mermaid with theme:'base' + themeVariables
            // pulled from the flavor's CSS tokens, and re-themes on dark/flavor
            // switch — so diagrams match the deck instead of a fixed palette.
        }

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

    /// Deck-level logo overlay: every flavor logo rendered once, persistent
    /// across slide transitions (so logos don't fade/flicker with each
    /// slide). Each carries `data-logo-layouts` so the presenter shows it
    /// only on matching layouts. Sits in `.sldr-logos`, a fixed overlay
    /// above the transitioning slides.
    fn generate_deck_logos(&self) -> String {
        let Some(flavor) = self.flavors.first() else {
            return String::new();
        };
        if flavor.logos.is_empty() {
            return String::new();
        }
        let assets_dir = flavor.source_dir.as_ref().map(|d| d.join("assets"));

        let mut out = String::from("  <div class=\"sldr-logos\" aria-hidden=\"true\">\n");
        for logo in &flavor.logos {
            let logo_src = if let Some(ref assets) = assets_dir {
                let logo_path = assets.join(&logo.file);
                if !logo_path.exists() {
                    tracing::warn!("Logo file not found: {}", logo_path.display());
                    continue;
                }
                match media::process_media_src(
                    &logo_path.to_string_lossy(),
                    assets.parent(),
                    self.config.image_mode,
                    self.config.output_dir.as_ref().map(|d| d.join("assets")).as_deref(),
                ) {
                    MediaEmbed::DataUri(uri) => uri,
                    MediaEmbed::External(url) => url,
                    MediaEmbed::AssetFile { html_src, .. } => html_src,
                    MediaEmbed::NotFound(_) => continue,
                }
            } else {
                logo.file.clone()
            };
            let _ = writeln!(
                out,
                "    <img class=\"sldr-logo\" src=\"{}\" alt=\"\" data-logo-layouts=\"{}\" style=\"{}\">",
                logo_src,
                html_escape_attr(&logo.layouts.join(" ")),
                logo.to_css_position()
            );
        }
        out.push_str("  </div>\n");
        out
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

/// Built-in localization of the framed "Source:" chrome label, keyed by
/// language code. These are shipped defaults resolved by the slide's active
/// language (the same axis as the body and chrome) — the label is a UI string
/// the renderer injects, not slide frontmatter, so it needs its own table.
/// Add a language by appending one row.
const SOURCE_LABELS: &[(&str, &str)] = &[
    ("en", "Source:"),
    ("de", "Quelle:"),
    ("fr", "Source :"),
    ("es", "Fuente:"),
    ("it", "Fonte:"),
    ("pt", "Fonte:"),
    ("nl", "Bron:"),
];

/// Resolve the "Source:" label for the active language (`requested`, else the
/// deck default), falling back to the deck default's label, then English. A
/// language absent from the table degrades to "Source:" rather than failing —
/// a cosmetic gap, unlike a missing headline.
fn source_label_for(requested: Option<&str>, deck_default: &str) -> &'static str {
    let lookup = |code: &str| {
        SOURCE_LABELS
            .iter()
            .find(|(c, _)| *c == code)
            .map(|(_, l)| *l)
    };
    let target = requested.unwrap_or(deck_default).to_lowercase();
    lookup(&target)
        .or_else(|| lookup(&deck_default.to_lowercase()))
        .unwrap_or("Source:")
}

/// Render the web-clipping source line: "<prefix> …", linked when a URL is
/// given. `prefix` is the localized label (e.g. "Source:", "Quelle:").
/// Self-contained — the link is inert until clicked.
fn render_source(text: &str, url: Option<&str>, prefix: &str) -> String {
    let text_esc = html_escape_text(text);
    let prefix_esc = html_escape_text(prefix);
    match url {
        Some(u) => format!(
            "<span class=\"sldr-source-label\">{prefix_esc}</span> <a href=\"{}\">{text_esc}</a>",
            html_escape_attr(u)
        ),
        None => format!("<span class=\"sldr-source-label\">{prefix_esc}</span> {text_esc}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_label_localizes_and_falls_back() {
        assert_eq!(source_label_for(Some("de"), "en"), "Quelle:");
        assert_eq!(source_label_for(Some("FR"), "en"), "Source :"); // case-insensitive
        assert_eq!(source_label_for(None, "de"), "Quelle:"); // no request → deck default
        // requested language absent → deck default's label …
        assert_eq!(source_label_for(Some("zz"), "de"), "Quelle:");
        // … then English when neither is in the table.
        assert_eq!(source_label_for(Some("zz"), "qq"), "Source:");
    }

    #[test]
    fn render_source_uses_localized_prefix() {
        let html = render_source("NPR", Some("https://npr.org"), "Quelle:");
        assert!(html.contains(">Quelle:</span>"));
        assert!(html.contains("href=\"https://npr.org\""));
        assert!(!html.contains("Source:"));
    }

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
    fn test_stray_marker_stripped_and_warned() {
        // The session's #1 bug: a lone ::content:: on a plain framed layout
        // leaked as literal text. Now it's removed and warned.
        let slide = sldr_core::slide::Slide::from_str(
            "leak",
            "leak.md",
            "---\nlayout: framed\n---\n::content::\n\nReal body here.\n",
        );
        let mut renderer = HtmlRenderer::new(RenderConfig::default());
        renderer.add_slide(&slide).unwrap();
        let html = renderer.render().unwrap();
        assert!(!html.contains("::content::"), "stray marker leaked into output");
        assert!(html.contains("Real body here."));
        assert!(
            renderer.warnings().iter().any(|w| w.contains("leak") && w.contains("stray")),
            "expected a stray-marker warning: {:?}",
            renderer.warnings()
        );
    }

    #[test]
    fn test_empty_footer_suppresses_chrome() {
        // `footer: ""` on a slide suppresses the footer (and wins over the
        // flavor footer), rather than rendering an empty line.
        let flavor = Flavor {
            name: "f".to_string(),
            footer: Some("© Flavor".to_string()),
            ..Default::default()
        };
        let slide = sldr_core::slide::Slide::from_str(
            "nf",
            "nf.md",
            "---\nlayout: framed\nfooter: \"\"\n---\nbody\n",
        );
        let mut r = HtmlRenderer::new(RenderConfig::default()).add_flavor(flavor);
        r.add_slide(&slide).unwrap();
        let html = r.render().unwrap();
        assert!(!html.contains("<div class=\"sldr-footer\""), "footer should be suppressed");
        assert!(!html.contains("© Flavor"), "flavor footer should not leak through");
    }

    #[test]
    fn test_framed_cover_has_no_footer_by_default() {
        let flavor = Flavor {
            name: "f".to_string(),
            footer: Some("© Flavor".to_string()),
            ..Default::default()
        };
        let slide = sldr_core::slide::Slide::from_str(
            "cov",
            "cov.md",
            "---\nlayout: framed-cover\ntitle: Hi\n---\n2026\n",
        );
        let mut r = HtmlRenderer::new(RenderConfig::default()).add_flavor(flavor);
        r.add_slide(&slide).unwrap();
        let html = r.render().unwrap();
        assert!(!html.contains("<div class=\"sldr-chrome\""), "covers stay clean by default");
    }

    #[test]
    fn test_marker_layout_mismatch_warns() {
        // image-left expects ::content::/::image:: but the body has neither →
        // it degrades to plain content. That must warn, not happen silently.
        let slide = sldr_core::slide::Slide::from_str(
            "broken",
            "broken.md",
            "---\nlayout: image-left\n---\nJust some text and an image.\n\n![x](pic.png)\n",
        );
        let mut renderer = HtmlRenderer::new(RenderConfig::default());
        renderer.add_slide(&slide).unwrap();
        let warned = renderer.warnings().iter().any(|w| {
            w.contains("broken") && w.contains("image-left") && w.contains("::content::")
        });
        assert!(warned, "expected a marker/layout mismatch warning: {:?}", renderer.warnings());
    }

    #[test]
    fn test_correct_markers_no_warning() {
        let slide = sldr_core::slide::Slide::from_str(
            "ok",
            "ok.md",
            "---\nlayout: image-left\n---\n::content::\nText here.\n::image::\n![x](pic.png)\n",
        );
        let mut renderer = HtmlRenderer::new(RenderConfig::default());
        renderer.add_slide(&slide).unwrap();
        assert!(
            !renderer.warnings().iter().any(|w| w.contains("expects")),
            "unexpected mismatch warning: {:?}",
            renderer.warnings()
        );
    }

    #[test]
    fn test_plain_layout_split_body_no_warning() {
        // A plain layout (default) receiving split markers is intentional
        // graceful degradation — it must NOT warn.
        let slide = sldr_core::slide::Slide::from_str(
            "deg",
            "deg.md",
            "---\nlayout: default\n---\n::content::\nText.\n::image::\n![x](pic.png)\n",
        );
        let mut renderer = HtmlRenderer::new(RenderConfig::default());
        renderer.add_slide(&slide).unwrap();
        assert!(!renderer.warnings().iter().any(|w| w.contains("expects")));
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
