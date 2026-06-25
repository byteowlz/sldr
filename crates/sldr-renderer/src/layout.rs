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
    /// Language of this slide variant when the deck embeds several
    /// languages (`data-lang`); None for single-language decks.
    pub lang: Option<&'a str>,
    pub rendered: MarkdownOutput,
    pub speaker_notes: Option<&'a str>,
    /// Chrome slots — persistent deck framing fed from frontmatter and the
    /// flavor, not from the markdown body. Each is pre-escaped/rendered
    /// HTML; a framed layout places them, a plain layout ignores them, and
    /// an empty slot collapses to nothing.
    pub chrome: Chrome,
}

/// Persistent slide framing: the headline/subheadline zone, footer line,
/// and web-clipping source attribution. Distinct from body content so a
/// framed layout can pin them in fixed chrome positions (the PowerPoint
/// title-placeholder model) — see ADR-0008.
#[derive(Default)]
pub struct Chrome {
    /// `{{headline}}` — from the slide's `title` (plain, pre-escaped).
    pub headline: Option<String>,
    /// `{{subheadline}}` — from the slide's `subtitle` (plain, pre-escaped).
    pub subheadline: Option<String>,
    /// `{{footer}}` — slide `footer` ?? flavor `footer` (plain, pre-escaped).
    pub footer: Option<String>,
    /// `{{source}}` — fully rendered "Source: …" HTML (optionally a link),
    /// or None when the slide has no source.
    pub source: Option<String>,
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
    /// `<!-- sldr:category NAME -->` — the function group a layout belongs to
    /// (title / body / image / framed …). A navigation label, author-declared
    /// per layout; never drives selection.
    pub category: Option<String>,
    /// `<!-- sldr:tags a, b -->` — free tags (e.g. register: classic/expressive).
    pub tags: Vec<String>,
    /// `<!-- sldr:zone … -->` directives — the PPTX export contract for this
    /// layout (ADR-0008, trx-4s9s). Each zone declares how one region maps to
    /// native PowerPoint: an editable text placeholder, a positioned picture,
    /// an autoshape, or (last resort) a baked raster. Empty for layouts that
    /// have not opted into PPTX export — they fall back to the screenshot path.
    pub zones: Vec<Zone>,
}

/// How a single layout region is represented when exporting to native
/// PowerPoint OOXML (trx-4s9s.2). The bitter-lesson "honest wall" at the
/// finest grain: derive everything representable into native PPTX, carry
/// (rasterize) only the truly irreducible bits — per region, not per slide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneRep {
    /// Editable text placeholder (headline/body/footer/columns). Chrome and
    /// body text stay editable on every slide regardless of layout.
    PlaceholderText,
    /// One or more individually positioned, editable/movable `<p:pic>`
    /// elements (any image arrangement: single, grid, row, scatter).
    Picture,
    /// A PPTX autoshape — geometric decoration (e.g. a diagonal accent band).
    Shape,
    /// Last resort: rasterize JUST this region's bounding box and place it as
    /// a positioned picture. For genuinely un-representable visuals (CSS
    /// vector scenes, gradient/filter effects, decorative SVG).
    Bake,
}

impl ZoneRep {
    /// Parse a `rep=` token. `text` is an alias for `placeholder-text`,
    /// `pic` for `picture`.
    fn from_token(s: &str) -> Option<Self> {
        Some(match s {
            "placeholder-text" | "text" => Self::PlaceholderText,
            "picture" | "pic" => Self::Picture,
            "shape" => Self::Shape,
            "bake" => Self::Bake,
            _ => return None,
        })
    }

    /// Canonical token form, for inspection output and round-tripping.
    pub fn as_token(self) -> &'static str {
        match self {
            Self::PlaceholderText => "placeholder-text",
            Self::Picture => "picture",
            Self::Shape => "shape",
            Self::Bake => "bake",
        }
    }
}

/// One declared region of a layout and its native-PowerPoint mapping
/// (trx-4s9s.2). Authored as a `<!-- sldr:zone … -->` directive; consumed by
/// the PPTX template/deck generator to position placeholders, pictures, and
/// shapes in EMU. Coordinates are percent of the slide box (0–100), the same
/// unit the layouts already think in (`--sldr-u`), converted to EMU at export.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// Slot or chrome name this zone fills — must match a layout slot
    /// (`content`, `left`, `right`, `image`, `heading`) or chrome field
    /// (`headline`, `subheadline`, `footer`, `source`). The key the deck
    /// generator (and round-trip import) uses to bind content ↔ placeholder.
    pub name: String,
    /// PPTX placeholder type token (`title`, `body`, `pic`, …) for
    /// `placeholder-text`/`picture` zones; `None` for `shape`/`bake`.
    pub ph: Option<String>,
    /// PPTX placeholder index. Distinct body placeholders on one layout need
    /// distinct `idx` values; `None` for a `title` (which takes no idx).
    pub idx: Option<u32>,
    /// Representation policy for this region.
    pub rep: ZoneRep,
    /// Position/size as percent of the slide box (0–100); EMU at export.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl LayoutDef {
    /// Whether the layout places a dedicated image slot (`{{image}}`) — i.e.
    /// it expects the body to split via `::content::` / `::image::` markers.
    pub fn expects_image(&self) -> bool {
        self.structure.contains("{{image}}")
    }

    /// Whether the layout places two column slots (`{{left}}` / `{{right}}`) —
    /// i.e. it expects the body to split via `::left::` / `::right::` markers.
    pub fn expects_columns(&self) -> bool {
        self.structure.contains("{{left}}") || self.structure.contains("{{right}}")
    }
}

/// Extract a single-line `<!-- sldr:KEY VALUE -->` directive's value.
fn directive_value<'a>(source: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("<!-- sldr:{key} ");
    let start = source.find(&pat)? + pat.len();
    let rest = &source[start..];
    let end = rest.find("-->")?;
    Some(rest[..end].trim())
}

/// Parse every `<!-- sldr:zone name=… ph=… idx=… rep=… x=… y=… w=… h=… -->`
/// directive in a layout file, in source order. Tokens are space-separated
/// `key=value` pairs; unknown keys are ignored, a malformed/incomplete zone
/// (missing `name`, or an unparseable number/rep) is skipped (fail-soft:
/// annotation never breaks a layout that still renders fine to HTML).
fn parse_zones(source: &str) -> Vec<Zone> {
    const PAT: &str = "<!-- sldr:zone ";
    let mut zones = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find(PAT) {
        let after = &rest[start + PAT.len()..];
        let Some(end) = after.find("-->") else { break };
        if let Some(zone) = parse_zone_attrs(after[..end].trim()) {
            zones.push(zone);
        }
        rest = &after[end + "-->".len()..];
    }
    zones
}

/// Parse the `key=value …` body of one zone directive. Returns `None` if a
/// required field is missing or a value fails to parse.
fn parse_zone_attrs(body: &str) -> Option<Zone> {
    let mut name: Option<String> = None;
    let mut ph: Option<String> = None;
    let mut idx: Option<u32> = None;
    let mut rep = ZoneRep::PlaceholderText;
    let (mut x, mut y, mut w, mut h) = (0.0_f64, 0.0_f64, 100.0_f64, 100.0_f64);
    for tok in body.split_whitespace() {
        let Some((key, val)) = tok.split_once('=') else {
            continue;
        };
        match key {
            "name" => name = Some(val.to_string()),
            "ph" => ph = (val != "none" && val != "-").then(|| val.to_string()),
            "idx" => idx = val.parse().ok(),
            "rep" => rep = ZoneRep::from_token(val)?,
            "x" => x = val.parse().ok()?,
            "y" => y = val.parse().ok()?,
            "w" => w = val.parse().ok()?,
            "h" => h = val.parse().ok()?,
            _ => {}
        }
    }
    Some(Zone {
        name: name?,
        ph,
        idx,
        rep,
        x,
        y,
        w,
        h,
    })
}

/// Built-in layouts, embedded in the binary in the exact same file format
/// a user authors. The file is the source of truth — there is no
/// hardcoded markup behind these names.
const BUILTIN_LAYOUTS: &[(&str, &str)] = &[
    ("agenda", include_str!("../layouts/agenda.html")),
    ("center", include_str!("../layouts/center.html")),
    ("contact", include_str!("../layouts/contact.html")),
    ("cover", include_str!("../layouts/cover.html")),
    ("default", include_str!("../layouts/default.html")),
    ("end", include_str!("../layouts/end.html")),
    ("feature-image", include_str!("../layouts/feature-image.html")),
    ("framed", include_str!("../layouts/framed.html")),
    ("framed-cols", include_str!("../layouts/framed-cols.html")),
    ("framed-cover", include_str!("../layouts/framed-cover.html")),
    ("framed-figure", include_str!("../layouts/framed-figure.html")),
    ("framed-full", include_str!("../layouts/framed-full.html")),
    ("framed-gallery", include_str!("../layouts/framed-gallery.html")),
    ("framed-image", include_str!("../layouts/framed-image.html")),
    ("framed-scatter", include_str!("../layouts/framed-scatter.html")),
    ("framed-section", include_str!("../layouts/framed-section.html")),
    ("hero-stat", include_str!("../layouts/hero-stat.html")),
    ("image", include_str!("../layouts/image.html")),
    ("image-center", include_str!("../layouts/image-center.html")),
    ("image-grid", include_str!("../layouts/image-grid.html")),
    ("image-left", include_str!("../layouts/image-left.html")),
    ("image-portraits", include_str!("../layouts/image-portraits.html")),
    ("image-right", include_str!("../layouts/image-right.html")),
    ("image-row", include_str!("../layouts/image-row.html")),
    ("image-stack", include_str!("../layouts/image-stack.html")),
    ("intro", include_str!("../layouts/intro.html")),
    ("pillars", include_str!("../layouts/pillars.html")),
    ("quote", include_str!("../layouts/quote.html")),
    ("section", include_str!("../layouts/section.html")),
    ("split-accent", include_str!("../layouts/split-accent.html")),
    ("statement", include_str!("../layouts/statement.html")),
    ("terminal", include_str!("../layouts/terminal.html")),
    ("timeline", include_str!("../layouts/timeline.html")),
    ("two-cols", include_str!("../layouts/two-cols.html")),
    ("two-cols-header", include_str!("../layouts/two-cols-header.html")),
    ("versus", include_str!("../layouts/versus.html")),
];

/// Raw source of a built-in layout by exact name — the authored `.html`
/// with comments and `<style>` intact (unlike a parsed `LayoutDef`, which
/// strips them). For `sldr show layout` and other inspection. `None` if no
/// built-in carries that name.
pub fn builtin_layout_source(name: &str) -> Option<&'static str> {
    BUILTIN_LAYOUTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, src)| *src)
}

/// All built-in layout names, sorted.
pub fn builtin_layout_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = BUILTIN_LAYOUTS.iter().map(|(n, _)| *n).collect();
    names.sort_unstable();
    names
}

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

    /// Every layout's navigation metadata, sorted by name:
    /// `(name, category, tags)`. Backs grouped `ls layouts` output.
    pub fn catalog(&self) -> Vec<(String, Option<String>, Vec<String>)> {
        let mut out: Vec<_> = self
            .layouts
            .values()
            .map(|d| (d.name.clone(), d.category.clone(), d.tags.clone()))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
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

    let category = directive_value(source, "category").map(str::to_string);
    let tags = directive_value(source, "tags")
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    LayoutDef {
        name: name.to_string(),
        structure: strip_html_comments(&without_style).trim().to_string(),
        css,
        collage,
        category,
        tags,
        zones: parse_zones(source),
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
    chrome: &Chrome,
) -> HashMap<&'static str, String> {
    let mut slots: HashMap<&'static str, String> = HashMap::new();
    // Chrome slots are fed from frontmatter + flavor, not the markdown body.
    // Always present (empty collapses); a framed layout places them.
    slots.insert("headline", chrome.headline.clone().unwrap_or_default());
    slots.insert("subheadline", chrome.subheadline.clone().unwrap_or_default());
    slots.insert("footer", chrome.footer.clone().unwrap_or_default());
    slots.insert("source", chrome.source.clone().unwrap_or_default());
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
            } else if structure.contains("{{heading}}") {
                // The heading has its own slot — don't duplicate it into
                // the concat fallback.
                concat_parts(&[&left, &right])
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
        lang,
        rendered,
        speaker_notes,
        chrome,
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
    if let Some(l) = lang {
        let _ = write!(html, " data-lang=\"{l}\"");
    }
    html.push_str(">\n");

    let slots = slot_map(rendered, def.collage, &def.structure, &chrome);
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
                lang: None,
                rendered,
                speaker_notes: notes,
                chrome: Chrome::default(),
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
                lang: None,
                rendered: MarkdownOutput::TwoCols {
                    heading: "<h1>Compare</h1>".to_string(),
                    left: "<p>Left</p>".to_string(),
                    right: "<p>Right</p>".to_string(),
                },
                speaker_notes: Some("Speaker note here"),
                chrome: Chrome::default(),
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
                lang: None,
                rendered: MarkdownOutput::Single("<h1>Right-aligned</h1>".to_string()),
                speaker_notes: None,
                chrome: Chrome::default(),
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
                lang: None,
                rendered: MarkdownOutput::Single("<h1>Big</h1>".to_string()),
                speaker_notes: None,
                chrome: Chrome::default(),
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
    fn test_parse_zones_from_framed_builtin() {
        let reg = registry();
        let def = reg.resolve("framed").unwrap();
        assert_eq!(def.zones.len(), 4);

        let head = &def.zones[0];
        assert_eq!(head.name, "headline");
        assert_eq!(head.ph.as_deref(), Some("title"));
        assert_eq!(head.idx, None);
        assert_eq!(head.rep, ZoneRep::PlaceholderText);
        assert_eq!(head.x, 4.4);
        assert_eq!(head.w, 70.0);

        assert_eq!(def.zones[1].name, "subheadline");
        assert_eq!(def.zones[1].idx, Some(3));

        let body = &def.zones[2];
        assert_eq!(body.name, "content");
        assert_eq!(body.ph.as_deref(), Some("body"));
        assert_eq!(body.idx, Some(1));
        assert_eq!(body.w, 91.1);

        assert_eq!(def.zones[3].name, "footer");
        assert_eq!(def.zones[3].idx, Some(2));
    }

    #[test]
    fn test_layouts_without_zones_are_empty_not_error() {
        let reg = registry();
        // default has no zone directives → empty, falls back to screenshot path.
        assert!(reg.resolve("default").unwrap().zones.is_empty());
    }

    #[test]
    fn test_parse_zone_attrs_variants() {
        // rep aliases, ph=none, missing optional fields default sanely.
        let z = parse_zone_attrs("name=art rep=bake x=10 y=20 w=30 h=40").unwrap();
        assert_eq!(z.name, "art");
        assert_eq!(z.rep, ZoneRep::Bake);
        assert_eq!(z.ph, None);
        assert_eq!(z.idx, None);
        assert_eq!((z.x, z.y, z.w, z.h), (10.0, 20.0, 30.0, 40.0));

        let z = parse_zone_attrs("name=pics ph=pic rep=pic x=0 y=0 w=100 h=100").unwrap();
        assert_eq!(z.rep, ZoneRep::Picture);
        assert_eq!(z.ph.as_deref(), Some("pic"));

        let z = parse_zone_attrs("name=band ph=none rep=shape x=0 y=0 w=50 h=100").unwrap();
        assert_eq!(z.rep, ZoneRep::Shape);
        assert_eq!(z.ph, None);

        // missing name → skipped.
        assert!(parse_zone_attrs("ph=title rep=text x=0 y=0 w=10 h=10").is_none());
        // unparseable rep → skipped.
        assert!(parse_zone_attrs("name=x rep=bogus").is_none());
    }

    #[test]
    fn test_zone_directives_do_not_leak_into_rendered_html() {
        let html = wrap(
            "framed",
            MarkdownOutput::Single("<h1>Hi</h1>".to_string()),
            None,
        );
        assert!(!html.contains("sldr:zone"));
        assert!(!html.contains("placeholder-text"));
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
