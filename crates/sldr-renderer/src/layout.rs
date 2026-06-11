//! Data-driven layout engine (ADR-0003, trx-gckd).
//!
//! A layout is a file: an HTML structure with `{{slot}}` placeholders plus
//! optional scoped CSS in a `<style>` block, binding only to flavor tokens
//! (`var(--sldr-*)`). Built-ins ship embedded in the binary in the same
//! format a user authors; a user layout dir overrides or extends them by
//! file stem. Nothing about a layout is code — authoring a new look never
//! requires recompiling.
//!
//! Slots are filled from the marker-driven markdown split (`::left::`,
//! `::right::`, `::content::`, `::image::` — see `markdown.rs`):
//!
//! | slot          | filled from                                        |
//! |---------------|----------------------------------------------------|
//! | `{{content}}` | single-block slides; or all parts concatenated when |
//! |               | the layout doesn't use the specific slots          |
//! | `{{heading}}` | text before `::left::` in a two-column slide        |
//! | `{{left}}` / `{{right}}` | the two column halves                   |
//! | `{{image}}`   | the `::image::` half of a content+image slide       |
//!
//! Directives are HTML comments the engine understands:
//! `<!-- sldr:transform collage -->` promotes image-only paragraphs to
//! `<figure>` elements gathered in a `.sldr-collage` wrapper.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::markdown::MarkdownOutput;

/// Options for `wrap_slide`. Bundled into a struct so we can grow the
/// per-slide knob set (alignment, transitions, click-step density, etc.)
/// without churning the call sites.
pub struct SlideOpts<'a> {
    pub index: usize,
    pub layout: &'a str,
    /// Horizontal alignment override: `Some("left" | "center" | "right")`.
    /// Emitted as `data-align` on the section so CSS overrides the
    /// layout's default without changing markup.
    pub align: Option<&'static str>,
    /// Vertical alignment override: `Some("top" | "center" | "bottom")`.
    pub valign: Option<&'static str>,
    pub rendered: MarkdownOutput,
    pub speaker_notes: Option<&'a str>,
}

/// A parsed layout definition: the structure that wraps a slide's content.
#[derive(Debug, Clone)]
pub struct LayoutDef {
    pub name: String,
    /// HTML fragment with `{{slot}}` placeholders (comments stripped).
    structure: String,
    /// Scoped CSS from the file's `<style>` block, emitted once into the
    /// document head when the layout is used. Built-ins keep their CSS in
    /// base.css and have none here.
    pub css: Option<String>,
    /// `<!-- sldr:transform collage -->` — promote image-only paragraphs
    /// to figures gathered in a `.sldr-collage` wrapper.
    collage: bool,
}

/// Built-in layouts, embedded in the binary in the exact same file format
/// a user authors. The file is the source of truth — there is no
/// hardcoded markup behind these names.
const BUILTIN_LAYOUTS: &[(&str, &str)] = &[
    ("center", include_str!("../layouts/center.html")),
    ("cover", include_str!("../layouts/cover.html")),
    ("default", include_str!("../layouts/default.html")),
    ("end", include_str!("../layouts/end.html")),
    ("image", include_str!("../layouts/image.html")),
    ("image-grid", include_str!("../layouts/image-grid.html")),
    ("image-left", include_str!("../layouts/image-left.html")),
    ("image-portraits", include_str!("../layouts/image-portraits.html")),
    ("image-right", include_str!("../layouts/image-right.html")),
    ("image-row", include_str!("../layouts/image-row.html")),
    ("image-stack", include_str!("../layouts/image-stack.html")),
    ("intro", include_str!("../layouts/intro.html")),
    ("quote", include_str!("../layouts/quote.html")),
    ("section", include_str!("../layouts/section.html")),
    ("two-cols", include_str!("../layouts/two-cols.html")),
    ("two-cols-header", include_str!("../layouts/two-cols-header.html")),
];

/// Layout name → definition. Built-ins first, user dirs override by name.
pub struct LayoutRegistry {
    layouts: HashMap<String, LayoutDef>,
    /// User dirs that were loaded, for fail-loud error messages.
    user_dirs: Vec<PathBuf>,
}

impl LayoutRegistry {
    /// Registry with only the embedded built-in layouts.
    pub fn builtin() -> Self {
        let mut layouts = HashMap::new();
        for (name, source) in BUILTIN_LAYOUTS {
            layouts.insert((*name).to_string(), parse_layout(name, source));
        }
        Self {
            layouts,
            user_dirs: Vec::new(),
        }
    }

    /// Load every `*.html` file in `dir` as a layout named after its file
    /// stem, overriding any built-in of the same name. Returns how many
    /// were loaded. A missing dir is not an error (zero loaded).
    pub fn load_dir(&mut self, dir: &Path) -> Result<usize> {
        self.user_dirs.push(dir.to_path_buf());
        if !dir.is_dir() {
            return Ok(0);
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .with_context(|| format!("Failed to read layout dir {}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "html"))
            .collect();
        // Deterministic load order regardless of filesystem iteration.
        entries.sort();

        let mut loaded = 0;
        for path in entries {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read layout {}", path.display()))?;
            self.layouts
                .insert(name.to_string(), parse_layout(name, &source));
            loaded += 1;
        }
        Ok(loaded)
    }

    pub fn get(&self, name: &str) -> Option<&LayoutDef> {
        self.layouts.get(name)
    }

    /// Resolve a layout or fail loudly with everything that was searched —
    /// the error is all an agent needs to fix the reference.
    pub fn resolve(&self, name: &str) -> Result<&LayoutDef> {
        self.get(name).with_context(|| {
            let mut names: Vec<&str> = self.layouts.keys().map(String::as_str).collect();
            names.sort_unstable();
            let searched = if self.user_dirs.is_empty() {
                "built-ins".to_string()
            } else {
                format!(
                    "built-ins, {}",
                    self.user_dirs
                        .iter()
                        .map(|d| d.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            format!(
                "Layout '{name}' not found (searched: {searched}). Available: {}",
                names.join(", ")
            )
        })
    }

    /// All layout names, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.layouts.keys().cloned().collect();
        names.sort();
        names
    }
}

impl Default for LayoutRegistry {
    fn default() -> Self {
        Self::builtin()
    }
}

/// Parse a layout file: extract the `<style>` block, read directives,
/// strip comments from the structure.
fn parse_layout(name: &str, source: &str) -> LayoutDef {
    let collage = source.contains("<!-- sldr:transform collage -->");

    // Extract the first top-level <style>...</style> block as scoped CSS.
    let (css, without_style) = match (source.find("<style>"), source.find("</style>")) {
        (Some(start), Some(end)) if end > start => {
            let css = source[start + "<style>".len()..end].trim().to_string();
            let mut rest = String::with_capacity(source.len());
            rest.push_str(&source[..start]);
            rest.push_str(&source[end + "</style>".len()..]);
            (Some(css).filter(|c| !c.is_empty()), rest)
        }
        _ => (None, source.to_string()),
    };

    LayoutDef {
        name: name.to_string(),
        structure: strip_html_comments(&without_style).trim().to_string(),
        css,
        collage,
    }
}

/// Remove `<!-- ... -->` comments (descriptions, directives) so they don't
/// get duplicated into every rendered slide.
fn strip_html_comments(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end_rel) => rest = &rest[start + end_rel + "-->".len()..],
            None => {
                rest = "";
                break;
            }
        }
        // Swallow a newline left behind by a comment-only line.
        if out.ends_with('\n') && rest.starts_with('\n') {
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Layouts whose content is a heading + a sequence of `<img>` paragraphs.
/// For these we promote each `<p><img alt="..."/></p>` to a `<figure>` so
/// alt text can render as a visible caption via `<figcaption>`. The
/// transform is declared per layout file (`<!-- sldr:transform collage -->`)
/// — non-collage layouts keep bare `<p><img/>`.
///
/// Promote image-only paragraphs to `<figure>` elements with optional
/// `<figcaption>` from the alt attribute, and gather all resulting figures
/// into a single `<div class="sldr-collage">` wrapper. The wrapper is
/// inserted at the position of the first image-paragraph so it lands
/// after any leading heading + subheadline.
///
/// pulldown-cmark groups consecutive image lines into one `<p>`, so each
/// tag inside an image-only paragraph is split into its own figure
/// (otherwise the grid would treat N images as 1 cell).
fn promote_images_to_figures(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut figures_buf = String::new();
    let mut wrapper_pos: Option<usize> = None;
    let mut rest = html;
    while let Some(p_start) = rest.find("<p>") {
        out.push_str(&rest[..p_start]);
        let after_open = &rest[p_start + 3..];
        let p_end_rel = match after_open.find("</p>") {
            Some(i) => i,
            None => {
                out.push_str(&rest[p_start..]);
                rest = "";
                break;
            }
        };
        let inner = &after_open[..p_end_rel];
        if let Some(figures) = try_render_image_paragraph(inner) {
            if wrapper_pos.is_none() {
                wrapper_pos = Some(out.len());
            }
            figures_buf.push_str(&figures);
        } else {
            out.push_str("<p>");
            out.push_str(inner);
            out.push_str("</p>");
        }
        rest = &after_open[p_end_rel + 4..];
    }
    out.push_str(rest);
    if !figures_buf.is_empty() {
        let pos = wrapper_pos.unwrap_or(out.len());
        let wrapper = format!("<div class=\"sldr-collage\">{figures_buf}</div>");
        out.insert_str(pos, &wrapper);
    }
    out
}

/// If `inner` is composed only of `<img/>` tags (any whitespace between),
/// return the rendered `<figure>` sequence. Otherwise return None — caller
/// keeps the paragraph as-is.
fn try_render_image_paragraph(inner: &str) -> Option<String> {
    let mut figures = String::new();
    let mut cursor = inner;
    let mut found_any = false;
    loop {
        let trimmed = cursor.trim_start();
        if trimmed.is_empty() {
            break;
        }
        if !trimmed.starts_with("<img ") {
            return None;
        }
        let close = trimmed.find("/>")?;
        let img_tag = &trimmed[..close + 2];
        let alt = extract_alt(img_tag).unwrap_or_default();
        figures.push_str("<figure class=\"sldr-collage-item\">");
        figures.push_str(img_tag);
        if !alt.is_empty() {
            figures.push_str("<figcaption>");
            figures.push_str(&alt);
            figures.push_str("</figcaption>");
        }
        figures.push_str("</figure>");
        cursor = &trimmed[close + 2..];
        found_any = true;
    }
    if found_any {
        Some(figures)
    } else {
        None
    }
}

fn extract_alt(img_tag: &str) -> Option<String> {
    let needle = " alt=\"";
    let start = img_tag.find(needle)? + needle.len();
    let end_rel = img_tag[start..].find('"')?;
    Some(img_tag[start..start + end_rel].to_string())
}

/// Build the slot map for a rendered slide. Every slot is always present;
/// slots the variant doesn't produce are empty. `content` doubles as the
/// graceful-degradation slot: when a slide uses split markers but the
/// layout file only references `{{content}}`, the parts concatenate in
/// source order instead of disappearing. The concat applies *only* when
/// the structure lacks the specific slots — a layout that uses
/// `{{image}}` gets the plain content in `{{content}}`, never a copy of
/// the image too.
fn slot_map(
    rendered: MarkdownOutput,
    collage: bool,
    structure: &str,
) -> HashMap<&'static str, String> {
    let mut slots: HashMap<&'static str, String> = HashMap::new();
    match rendered {
        MarkdownOutput::Single(content) => {
            let content = if collage {
                promote_images_to_figures(content.trim())
            } else {
                content.trim().to_string()
            };
            slots.insert("content", content);
            slots.insert("heading", String::new());
            slots.insert("left", String::new());
            slots.insert("right", String::new());
            slots.insert("image", String::new());
        }
        MarkdownOutput::TwoCols {
            heading,
            left,
            right,
        } => {
            let content = if structure.contains("{{left}}") {
                String::new()
            } else {
                concat_parts(&[&heading, &left, &right])
            };
            slots.insert("content", content);
            slots.insert("heading", heading.trim().to_string());
            slots.insert("left", left.trim().to_string());
            slots.insert("right", right.trim().to_string());
            slots.insert("image", String::new());
        }
        MarkdownOutput::ContentImage { content, image } => {
            let content_slot = if structure.contains("{{image}}") {
                content.trim().to_string()
            } else {
                concat_parts(&[&content, &image])
            };
            slots.insert("heading", String::new());
            slots.insert("left", String::new());
            slots.insert("right", String::new());
            slots.insert("image", image.trim().to_string());
            slots.insert("content", content_slot);
        }
    }
    slots
}

fn concat_parts(parts: &[&String]) -> String {
    let mut out = String::new();
    for part in parts {
        if !part.trim().is_empty() {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(part.trim());
        }
    }
    out
}

/// Substitute `{{slot}}` placeholders. Lines consisting solely of an empty
/// slot are dropped (so an absent heading leaves no blank line), matching
/// hand-written markup. Substituted values are never re-scanned.
fn fill_slots(structure: &str, slots: &HashMap<&'static str, String>) -> String {
    let mut out = String::with_capacity(structure.len());
    for line in structure.lines() {
        let trimmed = line.trim();
        // A line that is exactly one placeholder of an empty slot vanishes.
        if let Some(name) = trimmed
            .strip_prefix("{{")
            .and_then(|r| r.strip_suffix("}}"))
        {
            if slots.get(name.trim()).is_some_and(String::is_empty) {
                continue;
            }
        }
        let mut filled = line.to_string();
        for (name, value) in slots {
            let token = format!("{{{{{name}}}}}");
            if filled.contains(&token) {
                filled = filled.replace(&token, value);
            }
        }
        out.push_str(&filled);
        out.push('\n');
    }
    out
}

/// Wrap rendered markdown in a slide section using the given layout.
///
/// Returns a complete `<section class="sldr-slide" ...>` element. The
/// section shell (data-layout / data-index / data-page / alignment
/// attributes, speaker notes) is engine-owned; everything inside comes
/// from the layout file.
pub fn wrap_slide(opts: SlideOpts<'_>, def: &LayoutDef) -> String {
    use std::fmt::Write as _;

    let SlideOpts {
        index,
        layout,
        align,
        valign,
        rendered,
        speaker_notes,
    } = opts;

    let mut html = String::new();

    // 1-indexed, zero-padded page label. Emitted as a data attribute so
    // flavors can render page numbers via `content: attr(data-page)` —
    // CSS `counter-increment` would not work because the presenter sets
    // `display: none` on inactive slides and CSS counters only fire for
    // rendered elements (every slide would show '1'). See trx-jbpj.16.
    let page_num = index + 1;
    let _ = write!(
        html,
        "<section class=\"sldr-slide\" data-layout=\"{layout}\" data-index=\"{index}\" data-page=\"{page_num:02}\""
    );
    if let Some(a) = align {
        let _ = write!(html, " data-align=\"{a}\"");
    }
    if let Some(v) = valign {
        let _ = write!(html, " data-valign=\"{v}\"");
    }
    html.push_str(">\n");

    let slots = slot_map(rendered, def.collage, &def.structure);
    html.push_str(&fill_slots(&def.structure, &slots));

    // Speaker notes (hidden, read by presenter.js)
    if let Some(notes) = speaker_notes {
        if !notes.trim().is_empty() {
            html.push_str("  <aside class=\"sldr-notes\">\n    ");
            html.push_str(notes.trim());
            html.push_str("\n  </aside>\n");
        }
    }

    html.push_str("</section>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> LayoutRegistry {
        LayoutRegistry::builtin()
    }

    fn wrap(layout: &str, rendered: MarkdownOutput, notes: Option<&str>) -> String {
        let reg = registry();
        let def = reg.resolve(layout).unwrap();
        wrap_slide(
            SlideOpts {
                index: 0,
                layout,
                align: None,
                valign: None,
                rendered,
                speaker_notes: notes,
            },
            def,
        )
    }

    #[test]
    fn test_wrap_default() {
        let html = wrap(
            "default",
            MarkdownOutput::Single("<h1>Hello</h1>".to_string()),
            None,
        );
        assert!(html.contains("data-layout=\"default\""));
        assert!(html.contains("data-index=\"0\""));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(!html.contains("sldr-notes"));
        assert!(!html.contains("data-align"));
        assert!(!html.contains("data-valign"));
    }

    #[test]
    fn test_wrap_two_cols() {
        let reg = registry();
        let def = reg.resolve("two-cols").unwrap();
        let html = wrap_slide(
            SlideOpts {
                index: 1,
                layout: "two-cols",
                align: None,
                valign: None,
                rendered: MarkdownOutput::TwoCols {
                    heading: "<h1>Compare</h1>".to_string(),
                    left: "<p>Left</p>".to_string(),
                    right: "<p>Right</p>".to_string(),
                },
                speaker_notes: Some("Speaker note here"),
            },
            def,
        );
        assert!(html.contains("data-layout=\"two-cols\""));
        assert!(html.contains("sldr-columns"));
        assert!(html.contains("<p>Left</p>"));
        assert!(html.contains("<p>Right</p>"));
        assert!(html.contains("sldr-notes"));
        assert!(html.contains("Speaker note here"));
    }

    #[test]
    fn test_two_cols_empty_heading_leaves_no_blank_slot() {
        let html = wrap(
            "two-cols",
            MarkdownOutput::TwoCols {
                heading: String::new(),
                left: "<p>L</p>".to_string(),
                right: "<p>R</p>".to_string(),
            },
            None,
        );
        assert!(!html.contains("{{heading}}"));
        assert!(html.contains("<p>L</p>"));
    }

    #[test]
    fn test_wrap_with_notes() {
        let html = wrap(
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("My notes"),
        );
        assert!(html.contains("<aside class=\"sldr-notes\">"));
        assert!(html.contains("My notes"));
    }

    #[test]
    fn test_wrap_empty_notes_omitted() {
        let html = wrap(
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("   "),
        );
        assert!(!html.contains("sldr-notes"));
    }

    #[test]
    fn test_collage_promotes_images_to_figures() {
        let html = wrap(
            "image-grid",
            MarkdownOutput::Single(
                "<h1>Team</h1>\n<p><img src=\"a.jpg\" alt=\"Anna\" />\n</p>\n<p><img src=\"b.jpg\" alt=\"Bilal\" />\n</p>".to_string(),
            ),
            None,
        );
        assert!(html.contains("<div class=\"sldr-collage\">"));
        assert!(html.contains("<figure class=\"sldr-collage-item\">"));
        assert!(html.contains("<figcaption>Anna</figcaption>"));
        assert!(html.contains("<figcaption>Bilal</figcaption>"));
        // Heading is preserved untouched, before the collage wrapper
        let h1_pos = html.find("<h1>Team</h1>").unwrap();
        let collage_pos = html.find("<div class=\"sldr-collage\">").unwrap();
        assert!(h1_pos < collage_pos);
    }

    #[test]
    fn test_collage_splits_multi_image_paragraph() {
        // pulldown-cmark groups consecutive image lines into one <p>.
        // Each <img> should still get its own <figure>.
        let html = wrap(
            "image-grid",
            MarkdownOutput::Single(
                "<p><img src=\"a.jpg\" alt=\"Anna\" />\n<img src=\"b.jpg\" alt=\"Bilal\" />\n<img src=\"c.jpg\" alt=\"Chen\" /></p>".to_string(),
            ),
            None,
        );
        assert_eq!(html.matches("<figure class=\"sldr-collage-item\">").count(), 3);
        // All three figures share one wrapper.
        assert_eq!(html.matches("<div class=\"sldr-collage\">").count(), 1);
        assert!(html.contains("<figcaption>Anna</figcaption>"));
        assert!(html.contains("<figcaption>Bilal</figcaption>"));
        assert!(html.contains("<figcaption>Chen</figcaption>"));
    }

    #[test]
    fn test_collage_supports_subheadline() {
        // h1 + p (subhead) + figures: subhead stays out of the collage wrapper.
        let html = wrap(
            "image-grid",
            MarkdownOutput::Single(
                "<h1>Team</h1>\n<p>The folks behind the project.</p>\n<p><img src=\"a.jpg\" alt=\"Anna\" /></p>".to_string(),
            ),
            None,
        );
        let subhead_pos = html.find("<p>The folks behind the project.</p>").unwrap();
        let collage_pos = html.find("<div class=\"sldr-collage\">").unwrap();
        assert!(subhead_pos < collage_pos);
    }

    #[test]
    fn test_collage_skips_non_image_paragraphs() {
        let html = wrap(
            "image-row",
            MarkdownOutput::Single(
                "<p>Just text</p>\n<p><img src=\"a.jpg\" alt=\"\" />\n</p>".to_string(),
            ),
            None,
        );
        assert!(html.contains("<p>Just text</p>"));
        assert!(html.contains("<figure class=\"sldr-collage-item\">"));
        // Empty alt → no figcaption
        assert!(!html.contains("<figcaption>"));
    }

    #[test]
    fn test_non_collage_layout_keeps_bare_images() {
        let html = wrap(
            "default",
            MarkdownOutput::Single("<p><img src=\"a.jpg\" alt=\"Anna\" />\n</p>".to_string()),
            None,
        );
        assert!(!html.contains("<figure"));
        assert!(html.contains("<img src=\"a.jpg\" alt=\"Anna\""));
    }

    #[test]
    fn test_wrap_emits_alignment_attrs() {
        let reg = registry();
        let def = reg.resolve("default").unwrap();
        let html = wrap_slide(
            SlideOpts {
                index: 0,
                layout: "default",
                align: Some("right"),
                valign: Some("bottom"),
                rendered: MarkdownOutput::Single("<h1>Right-aligned</h1>".to_string()),
                speaker_notes: None,
            },
            def,
        );
        assert!(html.contains("data-align=\"right\""));
        assert!(html.contains("data-valign=\"bottom\""));
    }

    #[test]
    fn test_image_left_puts_image_column_first() {
        let html = wrap(
            "image-left",
            MarkdownOutput::ContentImage {
                content: "<p>Words</p>".to_string(),
                image: "<p><img src=\"a.jpg\" alt=\"\" /></p>".to_string(),
            },
            None,
        );
        let img_col = html.find("sldr-col-image").unwrap();
        let content_col = html.find("sldr-col-content").unwrap();
        assert!(img_col < content_col, "image-left: image column first");

        let html = wrap(
            "image-right",
            MarkdownOutput::ContentImage {
                content: "<p>Words</p>".to_string(),
                image: "<p><img src=\"a.jpg\" alt=\"\" /></p>".to_string(),
            },
            None,
        );
        let img_col = html.find("sldr-col-image").unwrap();
        let content_col = html.find("sldr-col-content").unwrap();
        assert!(content_col < img_col, "image-right: content column first");
    }

    #[test]
    fn test_split_content_degrades_into_content_only_layout() {
        // A slide with ::left::/::right:: markers rendered through a layout
        // that only uses {{content}} concatenates the parts instead of
        // losing them.
        let html = wrap(
            "default",
            MarkdownOutput::TwoCols {
                heading: "<h1>H</h1>".to_string(),
                left: "<p>L</p>".to_string(),
                right: "<p>R</p>".to_string(),
            },
            None,
        );
        assert!(html.contains("<h1>H</h1>"));
        assert!(html.contains("<p>L</p>"));
        assert!(html.contains("<p>R</p>"));
        assert!(!html.contains("sldr-columns"));
    }

    #[test]
    fn test_unknown_layout_fails_with_names_and_locations() {
        let reg = registry();
        let err = reg.resolve("does-not-exist").unwrap_err().to_string();
        assert!(err.contains("does-not-exist"));
        assert!(err.contains("built-ins"));
        assert!(err.contains("two-cols"));
    }

    #[test]
    fn test_user_layout_with_scoped_css() {
        let dir = std::env::temp_dir().join("sldr-layout-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("hero-split.html"),
            "<!-- A user layout -->\n<style>\n.sldr-slide[data-layout=\"hero-split\"] h1 { color: var(--sldr-accent); }\n</style>\n<div class=\"sldr-content hero\">\n  {{content}}\n</div>\n",
        )
        .unwrap();

        let mut reg = LayoutRegistry::builtin();
        let loaded = reg.load_dir(&dir).unwrap();
        assert_eq!(loaded, 1);

        let def = reg.resolve("hero-split").unwrap();
        assert!(def.css.as_deref().unwrap().contains("--sldr-accent"));

        let html = wrap_slide(
            SlideOpts {
                index: 0,
                layout: "hero-split",
                align: None,
                valign: None,
                rendered: MarkdownOutput::Single("<h1>Big</h1>".to_string()),
                speaker_notes: None,
            },
            def,
        );
        assert!(html.contains("class=\"sldr-content hero\""));
        assert!(html.contains("<h1>Big</h1>"));
        // Comments and style block never reach the output.
        assert!(!html.contains("<!--"));
        assert!(!html.contains("<style>"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_user_layout_overrides_builtin() {
        let dir = std::env::temp_dir().join("sldr-layout-override-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("default.html"),
            "<div class=\"sldr-content custom-default\">\n  {{content}}\n</div>\n",
        )
        .unwrap();

        let mut reg = LayoutRegistry::builtin();
        reg.load_dir(&dir).unwrap();
        let def = reg.resolve("default").unwrap();
        assert!(def.structure.contains("custom-default"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
