//! The zone document (ADR-0011): one derived answer to "what regions does
//! this slide have, what fills each one, and which file owns it?"
//!
//! It joins three existing sources — a slide's frontmatter and markdown, its
//! layout's `<!-- sldr:zone … -->` directives, and the flavor — into one list
//! that every visual surface reads instead of keeping a private model:
//!
//! - the studio slide editor draws the percent boxes over the real render and
//!   routes each edit to the file named in `writes`;
//! - PPTX export maps each entry to a placeholder / picture in EMU;
//! - PPTX import runs the same table backwards.
//!
//! Nothing here is stored. It is computed on request from files as they
//! stand, so it adds no field to any format (ADR-0001). The binding rules are
//! the ones the HTML renderer and the PPTX exporter already apply — this
//! module just makes them inspectable.

use serde::Serialize;
use sldr_core::flavor::Flavor;
use sldr_core::slide::Slide;

use crate::layout::{LayoutDef, Zone, ZoneRep};
use crate::markdown::{split_segments, MarkdownSegments};

/// Where an edit to a zone's *content* is written. Geometry is never here —
/// a zone's box always belongs to the layout (see [`ZoneDocument::geometry`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "target")]
pub enum Writes {
    /// The slide's own markdown file.
    Slide { path: String },
    /// The flavor's `flavor.toml` — a shared source; edits must show blast
    /// radius first (ADR-0002).
    Flavor { name: String },
}

/// What fills a zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Binding {
    /// A frontmatter field (`title`, `subtitle`, `footer`, `source`).
    Frontmatter { field: String },
    /// A markdown body segment: `content`, `heading`, `left` or `right`.
    /// `range` is the byte span of the segment inside the slide body when it
    /// can be located exactly (it is `None` for a synthesised fallback such as
    /// the concat of left+right into a plain `{{content}}` slot).
    Markdown {
        slot: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        range: Option<(usize, usize)>,
    },
    /// The first image reference found in the bound segment.
    Image {
        slot: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        src: Option<String>,
    },
    /// A flavor field (`footer`, `logos`) — style chrome per ADR-0008.
    Flavor { field: String },
    /// A pure decoration zone (`shape` / `bake`) or a name the slide has no
    /// input for. Nothing to edit here.
    None,
}

/// One zone of the document: the layout's declaration plus what the join knows.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZoneEntry {
    pub name: String,
    /// Representation policy token (`placeholder-text`, `picture`, `shape`, `bake`).
    pub rep: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ph: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<u32>,
    /// Percent of the slide box: `[x, y, w, h]`.
    #[serde(rename = "box")]
    pub bbox: [f64; 4],
    pub binding: Binding,
    /// The bound content as it stands (plain text for frontmatter/flavor
    /// fields, raw markdown for body segments, the src for images).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Where a content edit lands. `None` for decoration zones.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writes: Option<Writes>,
}

/// Slide input that no zone of the current layout shows. Mirrors the PPTX
/// exporter's accounting so nothing is silently invisible: a `::left::` body
/// on a plain layout, a `subtitle` on a layout without a subheadline zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unbound {
    pub name: String,
    pub binding: Binding,
}

/// Who owns the boxes. Always the layout: there is no per-slide geometry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Geometry {
    pub layout: String,
    /// `true` when the layout is an embedded built-in (editing its zones
    /// writes a library override rather than the built-in).
    pub builtin: bool,
    /// How many slides use this layout, when the caller knows (where-used).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used_by: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ZoneDocument {
    /// The slide's library-relative path.
    pub slide: String,
    pub layout: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavor: Option<String>,
    /// Language the body and chrome were resolved for.
    pub language: String,
    pub zones: Vec<ZoneEntry>,
    pub unbound: Vec<Unbound>,
    pub geometry: Geometry,
}

/// Inputs beyond the three files: language selection and layout provenance.
#[derive(Debug, Clone, Default)]
pub struct ZoneOpts<'a> {
    /// Requested language (`--lang`); `None` means the deck default.
    pub lang: Option<&'a str>,
    /// The deck's default language (config `default_language`).
    pub default_lang: &'a str,
    /// Whether `layout` came from the embedded built-ins.
    pub layout_builtin: bool,
    /// Slides using this layout, when known.
    pub layout_used_by: Option<usize>,
}

/// Compute the zone document for one slide rendered with `layout` and
/// (optionally) `flavor`. Pure: reads nothing from disk.
pub fn zone_document(
    slide: &Slide,
    layout: &LayoutDef,
    flavor: Option<&Flavor>,
    opts: &ZoneOpts,
) -> ZoneDocument {
    let default_lang = if opts.default_lang.is_empty() { "en" } else { opts.default_lang };
    let chrome = slide.metadata.chrome_for(opts.lang, default_lang);
    let selection = sldr_core::lang::select_language(&slide.content, opts.lang, default_lang);
    let body = selection.content.as_str();
    let segments = split_segments(body);
    let slide_writes = Writes::Slide { path: slide.relative_path.clone() };

    let mut zones = Vec::with_capacity(layout.zones.len());
    let mut covered: Vec<&str> = Vec::new();

    for z in &layout.zones {
        let (binding, content, writes) = match z.rep {
            ZoneRep::Shape | ZoneRep::Bake => (Binding::None, None, None),
            ZoneRep::Picture => {
                let (slot, md) = picture_segment(&z.name, &segments);
                let src = md.and_then(first_image_src);
                (
                    Binding::Image { slot: slot.to_string(), src: src.clone() },
                    src,
                    Some(slide_writes.clone()),
                )
            }
            ZoneRep::PlaceholderText => match z.name.as_str() {
                "headline" => frontmatter("title", chrome.title.clone(), &slide_writes),
                "subheadline" => frontmatter("subtitle", chrome.subtitle.clone(), &slide_writes),
                "source" => frontmatter("source", chrome.source.clone(), &slide_writes),
                "footer" => footer_binding(chrome.footer.clone(), flavor, &slide_writes),
                slot @ ("heading" | "content" | "left" | "right") => {
                    let text = segment(slot, &segments);
                    let range = text.and_then(|t| locate(body, t));
                    (
                        Binding::Markdown { slot: slot.to_string(), range },
                        text.map(str::to_string),
                        Some(slide_writes.clone()),
                    )
                }
                _ => (Binding::None, None, None),
            },
        };
        covered.push(z.name.as_str());
        zones.push(ZoneEntry {
            name: z.name.clone(),
            rep: z.rep.as_token(),
            ph: z.ph.clone(),
            idx: z.idx,
            bbox: bbox(z),
            binding,
            content,
            writes,
        });
    }

    // Account for every input the slide carries that the layout shows nowhere.
    let mut unbound = Vec::new();
    let mut note = |name: &str, present: bool, binding: Binding| {
        if present && !covered.contains(&name) {
            unbound.push(Unbound { name: name.to_string(), binding });
        }
    };
    note("headline", chrome.title.is_some(), Binding::Frontmatter { field: "title".into() });
    note("subheadline", chrome.subtitle.is_some(), Binding::Frontmatter { field: "subtitle".into() });
    note("source", chrome.source.is_some(), Binding::Frontmatter { field: "source".into() });
    note("footer", chrome.footer.is_some(), Binding::Frontmatter { field: "footer".into() });
    for slot in ["heading", "content", "left", "right", "image"] {
        let present = segment(slot, &segments).is_some_and(|s| !s.trim().is_empty());
        let binding = if slot == "image" {
            Binding::Image { slot: slot.into(), src: segments.image.as_deref().and_then(first_image_src) }
        } else {
            Binding::Markdown { slot: slot.into(), range: None }
        };
        note(slot, present, binding);
    }

    ZoneDocument {
        slide: slide.relative_path.clone(),
        layout: layout.name.clone(),
        flavor: flavor.map(|f| f.name.clone()),
        language: opts.lang.unwrap_or(default_lang).to_lowercase(),
        zones,
        unbound,
        geometry: Geometry {
            layout: layout.name.clone(),
            builtin: opts.layout_builtin,
            used_by: opts.layout_used_by,
        },
    }
}

fn bbox(z: &Zone) -> [f64; 4] {
    [z.x, z.y, z.w, z.h]
}

fn frontmatter(
    field: &str,
    value: Option<String>,
    writes: &Writes,
) -> (Binding, Option<String>, Option<Writes>) {
    (Binding::Frontmatter { field: field.to_string() }, value, Some(writes.clone()))
}

/// The footer follows the same precedence as the renderer: a slide `footer`
/// wins, else the flavor's. An empty zone writes to the slide — a new footer
/// typed here becomes a per-slide override, never a silent flavor edit.
fn footer_binding(
    slide_footer: Option<String>,
    flavor: Option<&Flavor>,
    slide_writes: &Writes,
) -> (Binding, Option<String>, Option<Writes>) {
    if slide_footer.is_some() {
        return frontmatter("footer", slide_footer, slide_writes);
    }
    if let Some(f) = flavor {
        if let Some(text) = f.footer.as_deref().filter(|t| !t.trim().is_empty()) {
            return (
                Binding::Flavor { field: "footer".into() },
                Some(text.to_string()),
                Some(Writes::Flavor { name: f.name.clone() }),
            );
        }
    }
    (Binding::None, None, Some(slide_writes.clone()))
}

fn segment<'a>(slot: &str, s: &'a MarkdownSegments) -> Option<&'a str> {
    match slot {
        "heading" => s.heading.as_deref(),
        "content" => s.content.as_deref(),
        "left" => s.left.as_deref(),
        "right" => s.right.as_deref(),
        "image" => s.image.as_deref(),
        _ => None,
    }
}

/// A picture zone reads the `image` segment when the body split one off,
/// otherwise the segment sharing its name (a lone image in `content`).
fn picture_segment<'a>(name: &'a str, s: &'a MarkdownSegments) -> (&'a str, Option<&'a str>) {
    if name == "image" {
        return ("image", s.image.as_deref());
    }
    match s.image.as_deref() {
        Some(img) if name == "content" => ("image", Some(img)),
        _ => (name, segment(name, s)),
    }
}

/// Byte span of a trimmed segment inside the body it came from.
fn locate(body: &str, segment: &str) -> Option<(usize, usize)> {
    if segment.is_empty() {
        return None;
    }
    body.find(segment).map(|start| (start, start + segment.len()))
}

/// First markdown image (`![alt](src)`) or `<img src>` in a segment.
fn first_image_src(md: &str) -> Option<String> {
    if let Some(start) = md.find("![") {
        let rest = &md[start..];
        let open = rest.find("](")? + 2;
        let close = rest[open..].find(')')? + open;
        let inner = rest[open..close].trim();
        // Strip an optional title: `](path "title")`.
        let src = inner.split_whitespace().next().unwrap_or(inner);
        return Some(src.to_string());
    }
    let start = md.find("<img")?;
    let rest = &md[start..];
    let s = rest.find("src=\"")? + 5;
    let e = rest[s..].find('"')? + s;
    Some(rest[s..e].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::LayoutRegistry;

    fn layout(name: &str) -> LayoutDef {
        LayoutRegistry::builtin().get(name).expect("built-in layout").clone()
    }

    fn slide(body: &str) -> Slide {
        Slide::from_str("s", "genai/s.md", body)
    }

    fn flavor(footer: Option<&str>) -> Flavor {
        let f = footer.map(|f| format!("footer = \"{f}\"")).unwrap_or_default();
        toml::from_str(&format!("name = \"byteowlz\"\n{f}\n[colors]\nprimary = \"#000\"\nbackground = \"#fff\"\ntext = \"#111\"\n"))
            .expect("flavor")
    }

    fn opts() -> ZoneOpts<'static> {
        ZoneOpts { default_lang: "en", layout_builtin: true, ..Default::default() }
    }

    fn by_name<'a>(doc: &'a ZoneDocument, name: &str) -> &'a ZoneEntry {
        doc.zones.iter().find(|z| z.name == name).unwrap_or_else(|| panic!("zone {name}"))
    }

    #[test]
    fn two_cols_binds_heading_and_columns_with_ranges() {
        let s = slide("---\ntitle: T\n---\n# Intro\n\n::left::\n- a\n- b\n\n::right::\n![d](media/x.png)\n");
        let doc = zone_document(&s, &layout("two-cols"), None, &opts());
        let left = by_name(&doc, "left");
        assert_eq!(left.content.as_deref(), Some("- a\n- b"));
        let (a, b) = match &left.binding {
            Binding::Markdown { slot, range } => {
                assert_eq!(slot, "left");
                range.expect("range")
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(&s.content[a..b], "- a\n- b");
        assert_eq!(left.writes, Some(Writes::Slide { path: "genai/s.md".into() }));
        assert_eq!(by_name(&doc, "heading").content.as_deref(), Some("# Intro"));
        assert_eq!(by_name(&doc, "right").content.as_deref(), Some("![d](media/x.png)"));
        // The frontmatter title is real slide input the layout never shows.
        assert!(doc.unbound.iter().any(|u| u.name == "headline"));
        assert_eq!(doc.geometry.layout, "two-cols");
    }

    #[test]
    fn framed_footer_routes_to_flavor_unless_slide_overrides() {
        let s = slide("---\ntitle: Head\nsubtitle: Sub\n---\nbody\n");
        let fl = flavor(Some("byteowlz · 2026"));
        let doc = zone_document(&s, &layout("framed"), Some(&fl), &opts());
        assert_eq!(by_name(&doc, "headline").binding, Binding::Frontmatter { field: "title".into() });
        assert_eq!(by_name(&doc, "headline").content.as_deref(), Some("Head"));
        let footer = by_name(&doc, "footer");
        assert_eq!(footer.binding, Binding::Flavor { field: "footer".into() });
        assert_eq!(footer.writes, Some(Writes::Flavor { name: "byteowlz".into() }));

        let s2 = slide("---\ntitle: Head\nfooter: mine\n---\nbody\n");
        let doc2 = zone_document(&s2, &layout("framed"), Some(&fl), &opts());
        let footer = by_name(&doc2, "footer");
        assert_eq!(footer.binding, Binding::Frontmatter { field: "footer".into() });
        assert_eq!(footer.content.as_deref(), Some("mine"));
    }

    #[test]
    fn empty_footer_still_writes_to_the_slide() {
        let s = slide("---\ntitle: Head\n---\nbody\n");
        let doc = zone_document(&s, &layout("framed"), Some(&flavor(None)), &opts());
        let footer = by_name(&doc, "footer");
        assert_eq!(footer.binding, Binding::None);
        assert_eq!(footer.writes, Some(Writes::Slide { path: "genai/s.md".into() }));
    }

    #[test]
    fn picture_zone_reports_image_src() {
        let s = slide("::content::\n- point\n\n::image::\n![alt](media/pic.png \"t\")\n");
        let doc = zone_document(&s, &layout("image-left"), None, &opts());
        let img = by_name(&doc, "image");
        assert_eq!(img.rep, "picture");
        assert_eq!(img.binding, Binding::Image { slot: "image".into(), src: Some("media/pic.png".into()) });
        assert_eq!(by_name(&doc, "content").content.as_deref(), Some("- point"));
        assert!(doc.unbound.is_empty(), "{:?}", doc.unbound);
    }

    #[test]
    fn language_selects_body_and_chrome() {
        let s = slide("---\ntitle: Hello\ntranslations:\n  de:\n    title: Hallo\n---\n::lang:en::\nhi\n::lang:de::\nhallo\n");
        let o = ZoneOpts { lang: Some("de"), default_lang: "en", layout_builtin: true, layout_used_by: None };
        let doc = zone_document(&s, &layout("framed"), None, &o);
        assert_eq!(doc.language, "de");
        assert_eq!(by_name(&doc, "headline").content.as_deref(), Some("Hallo"));
        assert_eq!(by_name(&doc, "content").content.as_deref(), Some("hallo"));
    }

    #[test]
    fn serializes_with_stable_shape() {
        let s = slide("---\ntitle: T\n---\nbody\n");
        let doc = zone_document(&s, &layout("framed"), None, &opts());
        let json = serde_json::to_value(&doc).unwrap();
        let z = &json["zones"][0];
        assert!(z["box"].is_array());
        assert_eq!(z["binding"]["kind"], "frontmatter");
        assert_eq!(z["writes"]["target"], "slide");
        assert_eq!(json["geometry"]["builtin"], true);
    }
}
