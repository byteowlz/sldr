//! Template-backed export (trx-4s9s.11, ADR-0010 "two export paths" #2):
//! slides placed on an existing PowerPoint master's *own* layouts and
//! placeholders, instead of on layouts sldr generates.
//!
//! Why: slides pasted into a deck on an organisation's master with "Use
//! Destination Theme" are re-bound by layout name and placeholder type+index.
//! Native export brings its own layout names and indices, so nothing matches
//! and every zone falls back to default geometry. Here the slides already sit
//! on the destination's layouts, so that re-binding is the identity.
//!
//! The master is an external source asset (ADR-0010), never a chrome layer:
//! its package is copied — the input bytes are not modified — with its
//! masters, layouts, theme and artwork untouched. Its own slides are dropped
//! (an example deck around a master is content, not branding). The mapping
//! from sldr layouts/zones to master layouts/placeholders is always explicit
//! ([`MasterMap`]); nothing is guessed, and a mapping that does not fit the
//! master fails before anything is written.
//!
//! sldr identity (per-shape markers + manifest) is attached exactly as in
//! native export, so `sldr import --apply` maps PowerPoint edits back.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use anyhow::{bail, Context, Result};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::deck::{Target, ZoneTarget};
use crate::package::{rels_path, resolve, Package, A, P, R};
use crate::{Conversion, Disposition as D, SlideInput, ZoneRep};

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const SLIDE_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
/// `p14:sectionLst` extension: sections point at slide ids, which are dropped.
const SECTIONS_EXT: &str = "{521415D9-36F7-43E2-AB2F-B90AF26B5E84}";

/// How a deck maps onto a master. Usually read from a TOML file next to the
/// master (the CLI's `--master-map`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MasterMap {
    /// The master `.pptx`, relative to the map file (resolved by the caller).
    #[serde(default)]
    pub master: Option<String>,
    /// Flavor for zone geometry and HTML-side chrome (resolved by the caller).
    #[serde(default)]
    pub flavor: Option<String>,
    /// 1-based slide master inside the package (default 1).
    #[serde(default)]
    pub slide_master: Option<usize>,
    /// sldr layout name → master layout + zone placeholders.
    #[serde(default)]
    pub layouts: BTreeMap<String, LayoutMapping>,
    /// Master layout for sldr layouts not listed. Without it, an unlisted
    /// layout fails the export.
    #[serde(default)]
    pub fallback: Option<String>,
    /// Box (percent of the slide) for zones that have no master placeholder,
    /// by zone name. Default: the zone's own sldr geometry.
    #[serde(default)]
    pub positions: BTreeMap<String, Position>,
    /// Fill the master layout's slide-number placeholder (default true).
    #[serde(default = "default_true")]
    pub slide_number: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutMapping {
    /// Master layout name (as shown in PowerPoint; surrounding spaces ignored).
    pub layout: String,
    /// sldr zone → placeholder `idx` on that layout (`0` = the title).
    /// Text zones not listed become text boxes (see [`MasterMap::positions`]).
    #[serde(default)]
    pub zones: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What a master package offers: the inventory a mapping is written against.
#[derive(Debug, Clone, Serialize)]
pub struct MasterInventory {
    /// Slide size in EMU.
    pub slide_size: (i64, i64),
    /// Slides in the package (dropped on export).
    pub slides: usize,
    pub masters: Vec<MasterInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MasterInfo {
    /// 1-based, as `slide_master` in a [`MasterMap`].
    pub index: usize,
    pub theme: Option<String>,
    pub heading_font: Option<String>,
    pub body_font: Option<String>,
    pub layouts: Vec<LayoutInfo>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LayoutInfo {
    /// Display name, trimmed.
    pub name: String,
    pub placeholders: Vec<PlaceholderInfo>,
    #[serde(skip)]
    part: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaceholderInfo {
    /// OOXML placeholder type (`title`, `body`, `obj`, `pic`, `ftr`, `sldNum`, …).
    pub kind: String,
    /// Placeholder index (`0` for a title without one).
    pub idx: u32,
    /// Shape name in the master (a label, not an identifier).
    pub name: String,
    /// `[x, y, w, h]` in percent of the slide, when the layout positions it.
    pub bbox: Option<[f64; 4]>,
    #[serde(skip)]
    has_idx: bool,
}

/// Inventory a master package: masters, their layouts and placeholders.
pub fn inspect(bytes: &[u8]) -> Result<MasterInventory> {
    inventory(&Package::read(bytes)?)
}

fn inventory(pkg: &Package) -> Result<MasterInventory> {
    let pres = pkg.xml("ppt/presentation.xml")?;
    let size = pres
        .descendants()
        .find(|n| n.has_tag_name((P, "sldSz")))
        .and_then(|n| Some((n.attribute("cx")?.parse().ok()?, n.attribute("cy")?.parse().ok()?)))
        .context("presentation has no slide size")?;
    let slides = pres.descendants().filter(|n| n.has_tag_name((P, "sldId"))).count();
    let pres_rels = pkg.rels("ppt/presentation.xml")?;
    let mut masters = Vec::new();
    for node in pres.descendants().filter(|n| n.has_tag_name((P, "sldMasterId"))) {
        let rid = node.attribute((R, "id")).context("slide master without relationship")?;
        let part = pres_rels.iter().find(|r| r.id == rid && !r.external).context("missing slide master")?.target.clone();
        let doc = pkg.xml(&part)?;
        let rels = pkg.rels(&part)?;
        let theme_part = rels.iter().find(|r| r.kind.ends_with("/theme") && !r.external).map(|r| r.target.clone());
        let (theme, heading_font, body_font) = match &theme_part {
            Some(t) => theme_names(&pkg.xml(t)?),
            None => (None, None, None),
        };
        let mut layouts = Vec::new();
        for id in doc.descendants().filter(|n| n.has_tag_name((P, "sldLayoutId"))) {
            let rid = id.attribute((R, "id")).context("layout without relationship")?;
            let lpart = rels.iter().find(|r| r.id == rid && !r.external).context("missing slide layout")?.target.clone();
            let ldoc = pkg.xml(&lpart)?;
            let name = ldoc
                .descendants()
                .find(|n| n.has_tag_name((P, "cSld")))
                .and_then(|n| n.attribute("name"))
                .unwrap_or("")
                .trim()
                .to_string();
            layouts.push(LayoutInfo { name, placeholders: placeholders(&ldoc, size), part: lpart });
        }
        masters.push(MasterInfo { index: masters.len() + 1, theme, heading_font, body_font, layouts });
    }
    if masters.is_empty() {
        bail!("this .pptx has no slide master");
    }
    Ok(MasterInventory { slide_size: size, slides, masters })
}

fn theme_names(doc: &Document) -> (Option<String>, Option<String>, Option<String>) {
    let font = |tag: &str| {
        doc.descendants()
            .find(|n| n.has_tag_name((A, tag)))
            .and_then(|n| n.children().find(|c| c.has_tag_name((A, "latin"))))
            .and_then(|n| n.attribute("typeface"))
            .map(str::to_string)
    };
    (doc.root_element().attribute("name").map(str::to_string), font("majorFont"), font("minorFont"))
}

fn placeholders(doc: &Document, size: (i64, i64)) -> Vec<PlaceholderInfo> {
    let mut out = Vec::new();
    for shape in doc.descendants().filter(|n| n.has_tag_name((P, "sp")) || n.has_tag_name((P, "pic"))) {
        let Some(ph) = shape.descendants().find(|n| n.has_tag_name((P, "ph"))) else { continue };
        let name = shape
            .descendants()
            .find(|n| n.has_tag_name((P, "cNvPr")))
            .and_then(|n| n.attribute("name"))
            .unwrap_or("")
            .to_string();
        let bbox = shape.descendants().find(|n| n.has_tag_name((A, "xfrm"))).and_then(|x| {
            let off = x.children().find(|c| c.has_tag_name((A, "off")))?;
            let ext = x.children().find(|c| c.has_tag_name((A, "ext")))?;
            let n = |node: Node, a: &str| node.attribute(a)?.parse::<f64>().ok();
            let (w, h) = (size.0 as f64, size.1 as f64);
            Some([n(off, "x")? / w * 100.0, n(off, "y")? / h * 100.0, n(ext, "cx")? / w * 100.0, n(ext, "cy")? / h * 100.0])
        });
        out.push(PlaceholderInfo {
            kind: ph.attribute("type").unwrap_or("obj").to_string(),
            idx: ph.attribute("idx").and_then(|i| i.parse().ok()).unwrap_or(0),
            has_idx: ph.attribute("idx").is_some(),
            name,
            bbox,
        });
    }
    out
}

/// A TOML mapping skeleton for `inv`, listing every layout and its
/// placeholders as comments. The operator fills in `[layouts.*]`; nothing is
/// pre-mapped (no layout guessing, ADR-0010).
pub fn map_skeleton(inv: &MasterInventory, master_file: &str) -> String {
    let mut s = format!(
        "# sldr master map: sldr layouts/zones -> this master's layouts/placeholders.\n\
         # Use: sldr export <deck> --format pptx --master-map <this file>\n\
         master = \"{master_file}\"\n\
         slide_master = 1\n\
         slide_number = true\n\
         # fallback = \"<master layout for unlisted sldr layouts>\"\n\n\
         # [layouts.framed]\n\
         # layout = \"<master layout name>\"\n\
         # zones = {{ headline = 0, subheadline = <idx>, content = <idx>, footer = <idx> }}\n\n\
         # [positions.source]   # zones without a placeholder become text boxes here (% of slide)\n\
         # x = 4.4\n# y = 88.0\n# w = 45.0\n# h = 3.5\n\n"
    );
    for m in &inv.masters {
        s.push_str(&format!("# --- slide_master = {} (theme {:?})\n", m.index, m.theme.as_deref().unwrap_or("")));
        for l in &m.layouts {
            let phs: Vec<String> = l.placeholders.iter().map(|p| format!("{} {}", p.kind, p.idx)).collect();
            s.push_str(&format!("#   {:?}: {}\n", l.name, phs.join(", ")));
        }
    }
    s
}

/// Export `slides` onto the master in `master_bytes` per `map`. The report
/// carries the same accounting as native export plus the master provenance.
pub fn build_deck_on_master(master_bytes: &[u8], map: &MasterMap, title: &str, slides: &[SlideInput]) -> Result<Conversion<Vec<u8>>> {
    if slides.is_empty() {
        bail!("PPTX deck needs at least one slide");
    }
    let mut report = crate::preflight::deck(slides);
    // The master's layouts carry all styling here; sldr layout CSS is moot.
    report.findings.retain(|f| f.feature != "custom_css");
    report.enforce(true)?;

    let pkg = Package::read(master_bytes)?;
    let inv = inventory(&pkg)?;
    let mi = map.slide_master.unwrap_or(1);
    let master = inv.masters.get(mi.wrapping_sub(1)).with_context(|| {
        format!("slide_master = {mi}, but the package has {} slide master(s)", inv.masters.len())
    })?;
    let by_name: HashMap<&str, &LayoutInfo> = master.layouts.iter().map(|l| (l.name.as_str(), l)).collect();
    let available = || master.layouts.iter().map(|l| format!("{:?}", l.name)).collect::<Vec<_>>().join(", ");

    // Resolve every slide before writing anything: a mapping that does not fit
    // the master is a configuration error, never a partial deck.
    let mut errors = BTreeSet::new();
    let mut targets = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let sldr_layout = slide.layout.name.as_str();
        let (layout_name, zone_map) = match map.layouts.get(sldr_layout) {
            Some(m) => (m.layout.trim(), Some(&m.zones)),
            None => match &map.fallback {
                Some(f) => (f.trim(), None),
                None => {
                    errors.insert(format!("sldr layout {sldr_layout:?} is not mapped (add [layouts.{sldr_layout}] or a fallback)"));
                    continue;
                }
            },
        };
        if let Some(zm) = zone_map {
            for zone in zm.keys() {
                if !slide.layout.zones.iter().any(|z| &z.name == zone) {
                    errors.insert(format!("[layouts.{sldr_layout}] maps zone {zone:?}, which sldr layout {sldr_layout:?} does not have"));
                }
            }
        }
        let Some(layout) = by_name.get(layout_name) else {
            errors.insert(format!("master layout {layout_name:?} does not exist in slide master {mi}; available: {}", available()));
            continue;
        };
        let text_zones: Vec<_> = slide.layout.zones.iter().filter(|z| z.rep == ZoneRep::PlaceholderText && z.ph.is_some()).collect();
        let mut zones = HashMap::new();
        for zone in text_zones {
            let target = match zone_map.and_then(|m| m.get(&zone.name)) {
                Some(idx) => match layout.placeholders.iter().find(|p| p.idx == *idx) {
                    Some(p) => ZoneTarget::Placeholder {
                        typ: (p.kind != "obj").then(|| p.kind.clone()),
                        idx: p.has_idx.then_some(p.idx),
                    },
                    None => {
                        let have: Vec<String> = layout.placeholders.iter().map(|p| format!("{} {}", p.kind, p.idx)).collect();
                        errors.insert(format!("master layout {layout_name:?} has no placeholder idx {idx} (for zone {:?}); it has: {}", zone.name, have.join(", ")));
                        continue;
                    }
                },
                None => {
                    let p = map.positions.get(&zone.name).copied().unwrap_or(Position { x: zone.x, y: zone.y, w: zone.w, h: zone.h });
                    ZoneTarget::TextBox { x: p.x, y: p.y, w: p.w, h: p.h }
                }
            };
            zones.insert(zone.name.clone(), target);
        }
        let slide_number = map
            .slide_number
            .then(|| layout.placeholders.iter().find(|p| p.kind == "sldNum"))
            .flatten()
            .map(|p| (p.kind.clone(), p.idx, i + 1));
        targets.push(Target {
            layout_target: relative_to_slides(&layout.part)?,
            size: inv.slide_size,
            media_prefix: "sldr-",
            zones,
            slide_number,
        });
    }
    if !errors.is_empty() {
        bail!("the master map does not fit this deck/master:\n  - {}", errors.into_iter().collect::<Vec<_>>().join("\n  - "));
    }

    // ---- assemble: the master package, its slides replaced by ours ----------
    let mut text: BTreeMap<String, String> = BTreeMap::new();
    let mut binary: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for (name, bytes) in &pkg.parts {
        let xmlish = name.ends_with(".xml") || name.ends_with(".rels");
        match (xmlish, std::str::from_utf8(bytes)) {
            (true, Ok(s)) => {
                text.insert(name.clone(), s.to_string());
            }
            _ => {
                binary.insert(name.clone(), bytes.clone());
            }
        }
    }

    // The master's own slides and their notes are dropped up front: the new
    // deck reuses those part names, and notes are appended, not replaced.
    let is_old_slide = |p: &str| p.starts_with("ppt/slides/") || p.starts_with("ppt/notesSlides/");
    text.retain(|p, _| !is_old_slide(p));
    binary.retain(|p, _| !is_old_slide(p));

    let pres_path = "ppt/presentation.xml";
    let pres_rels_path = rels_path(pres_path);
    let pres = text.get(pres_path).context("missing presentation part")?.clone();
    text.insert(pres_path.into(), strip_presentation(&pres)?);
    let rels = text.get(&pres_rels_path).context("missing presentation relationships")?.clone();
    text.insert(pres_rels_path.clone(), drop_rels(&rels, |kind| kind.ends_with("/slide"))?);
    // The thumbnail is a picture of the master's first slide (content).
    if let Some(root) = text.get("_rels/.rels").cloned() {
        text.insert("_rels/.rels".into(), drop_rels(&root, |kind| kind.ends_with("/thumbnail"))?);
    }
    // Document properties describe the new deck, not the master's example
    // deck (whose slide titles and authors would otherwise leak through).
    text.insert("docProps/core.xml".into(), crate::core_props(title));
    text.insert("docProps/app.xml".into(), crate::app_props());
    ensure_root_rel(&mut text, "docProps/core.xml", "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties")?;
    ensure_root_rel(&mut text, "docProps/app.xml", "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties")?;

    let mut media: Vec<(String, Vec<u8>)> = Vec::new();
    let mut sld_ids = String::new();
    let mut slide_rels = String::new();
    let mut overrides = String::new();
    for (i, (slide, target)) in slides.iter().zip(&targets).enumerate() {
        let n = i + 1;
        let path = format!("ppt/slides/slide{n}.xml");
        let (xml, rels) = crate::deck::build_slide(slide, target, &mut media);
        text.insert(path.clone(), xml);
        text.insert(format!("ppt/slides/_rels/slide{n}.xml.rels"), rels);
        sld_ids.push_str(&format!("<p:sldId id=\"{}\" r:id=\"rIdSldrSlide{n}\"/>", 255 + n));
        slide_rels.push_str(&format!("<Relationship Id=\"rIdSldrSlide{n}\" Type=\"{R}/slide\" Target=\"slides/slide{n}.xml\"/>"));
        overrides.push_str(&format!("<Override PartName=\"/{path}\" ContentType=\"{SLIDE_CT}\"/>"));
    }
    let pres = text[pres_path].clone();
    let at = pres.find("<p:sldSz").context("presentation has no slide size")?;
    text.insert(pres_path.into(), format!("{}<p:sldIdLst>{sld_ids}</p:sldIdLst>{}", &pres[..at], &pres[at..]));
    let rels = text[&pres_rels_path].clone();
    text.insert(pres_rels_path, rels.replace("</Relationships>", &format!("{slide_rels}</Relationships>")));

    // Content types: drop the master's slide overrides, add ours and any
    // media extension the master did not declare.
    let ct = text.get("[Content_Types].xml").context("missing content types")?.clone();
    let mut ct = drop_overrides(&ct, |name| name.starts_with("/ppt/slides/") || name.starts_with("/ppt/notesSlides/"))?;
    let defaults = ct_defaults(&ct)?;
    let mut extra = String::new();
    for ext in media.iter().filter_map(|(p, _)| p.rsplit('.').next()).collect::<BTreeSet<_>>() {
        if !defaults.contains(&ext.to_ascii_lowercase()) {
            extra.push_str(&format!("<Default Extension=\"{ext}\" ContentType=\"{}\"/>", media_type(ext)));
        }
    }
    ct = ct.replace("</Types>", &format!("{extra}{overrides}</Types>"));
    text.insert("[Content_Types].xml".into(), ct);

    // Identity + notes work on the part list, exactly as for native decks.
    let mut parts: Vec<(String, String)> = text.into_iter().collect();
    crate::identity::attach(&mut parts, slides)?;
    crate::notes::attach(&mut parts, slides)?;
    for (path, bytes) in media {
        binary.insert(path, bytes);
    }
    // A master can carry relationships to parts it no longer contains (common
    // after hand-editing); PowerPoint would offer to repair the result. Drop
    // them from the copy and say so.
    let dangling = drop_dangling(&mut parts, &binary)?;
    if dangling > 0 {
        report.record(None, "master", "relationships", "master_dangling_relationships", D::Converted,
            &format!("{dangling} relationship(s) in the master pointed at missing parts and were left out of the copy"));
    }

    // Keep only what the package still reaches (the master's slides, their
    // notes and media go), and the content types that still apply.
    let (parts, binary) = prune(parts, binary)?;
    let mut parts = parts;
    parts.sort_by(|a, b| (a.0 != "[Content_Types].xml", &a.0).cmp(&(b.0 != "[Content_Types].xml", &b.0)));
    let bytes = crate::zip_mixed(&parts, &binary.into_iter().collect::<Vec<_>>())?;
    Package::read(&bytes)?.validate().context("template-backed package failed validation")?;

    let digest = format!("{:x}", Sha256::digest(master_bytes));
    report.record(None, "master", "package", "master_template", D::Converted,
        &format!("Slides placed on the master's own layouts (slide master {mi}); its theme, layouts and artwork are kept as-is, not redrawn. Master sha256 {digest}"));
    if inv.slides > 0 {
        report.record(None, "master", "slides", "master_slides_dropped", D::Converted,
            &format!("The master's {} own slide(s) are not carried over", inv.slides));
    }
    Ok(Conversion { value: bytes, report })
}

/// `ppt/slideLayouts/x.xml` → `../slideLayouts/x.xml` (seen from `ppt/slides/`).
fn relative_to_slides(part: &str) -> Result<String> {
    let rest = part.strip_prefix("ppt/").with_context(|| format!("layout outside ppt/: {part}"))?;
    Ok(format!("../{rest}"))
}

/// Remove the slide list and everything that points at slide ids (custom
/// shows, sections) from `presentation.xml`.
fn strip_presentation(xml: &str) -> Result<String> {
    let doc = Document::parse(xml)?;
    let mut cut: Vec<std::ops::Range<usize>> = doc
        .descendants()
        .filter(|n| {
            n.has_tag_name((P, "sldIdLst"))
                || n.has_tag_name((P, "custShowLst"))
                || (n.has_tag_name((P, "ext")) && n.attribute("uri") == Some(SECTIONS_EXT))
        })
        .map(|n| n.range())
        .collect();
    cut.sort_by_key(|r| std::cmp::Reverse(r.start));
    let mut out = xml.to_string();
    for r in cut {
        out.replace_range(r, "");
    }
    // An extension list emptied by the cut above would be invalid.
    Ok(out.replace("<p:extLst></p:extLst>", ""))
}

/// Remove relationships whose type matches `drop`.
fn drop_rels(xml: &str, drop: impl Fn(&str) -> bool) -> Result<String> {
    let doc = Document::parse(xml)?;
    let mut cut: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name((REL_NS, "Relationship")) && n.attribute("Type").is_some_and(&drop))
        .map(|n| n.range())
        .collect();
    cut.sort_by_key(|r| std::cmp::Reverse(r.start));
    let mut out = xml.to_string();
    for r in cut {
        out.replace_range(r, "");
    }
    Ok(out)
}

fn ensure_root_rel(text: &mut BTreeMap<String, String>, target: &str, kind: &str) -> Result<()> {
    let root = text.get("_rels/.rels").context("missing package relationships")?.clone();
    if !root.contains(&format!("Target=\"{target}\"")) && !root.contains(&format!("Target=\"/{target}\"")) {
        let id = format!("rIdSldr{}", target.rsplit('/').next().unwrap_or("x").trim_end_matches(".xml"));
        text.insert("_rels/.rels".into(), root.replace("</Relationships>", &format!("<Relationship Id=\"{id}\" Type=\"{kind}\" Target=\"{target}\"/></Relationships>")));
    }
    let ct = text.get("[Content_Types].xml").context("missing content types")?.clone();
    if !ct.contains(&format!("PartName=\"/{target}\"")) {
        let typ = if target.ends_with("core.xml") {
            "application/vnd.openxmlformats-package.core-properties+xml"
        } else {
            "application/vnd.openxmlformats-officedocument.extended-properties+xml"
        };
        text.insert("[Content_Types].xml".into(), ct.replace("</Types>", &format!("<Override PartName=\"/{target}\" ContentType=\"{typ}\"/></Types>")));
    }
    Ok(())
}

/// Remove internal relationships whose target part does not exist. Returns
/// how many were removed.
fn drop_dangling(parts: &mut [(String, String)], binary: &BTreeMap<String, Vec<u8>>) -> Result<usize> {
    let names: BTreeSet<String> = parts.iter().map(|(p, _)| p.clone()).chain(binary.keys().cloned()).collect();
    let mut removed = 0;
    for (path, xml) in parts.iter_mut().filter(|(p, _)| p.ends_with(".rels")) {
        let owner = if path == "_rels/.rels" {
            String::new()
        } else {
            let (dir, name) = path.rsplit_once("/_rels/").context("invalid relationship part path")?;
            format!("{dir}/{}", name.trim_end_matches(".rels"))
        };
        let doc = Document::parse(xml).with_context(|| format!("malformed {path}"))?;
        let mut cut: Vec<_> = Vec::new();
        for rel in doc.descendants().filter(|n| n.has_tag_name((REL_NS, "Relationship"))) {
            if rel.attribute("TargetMode") == Some("External") {
                continue;
            }
            let Some(target) = rel.attribute("Target") else { continue };
            let resolved = if owner.is_empty() { target.trim_start_matches('/').to_string() } else { resolve(&owner, target)? };
            if !names.contains(&resolved) {
                cut.push(rel.range());
            }
        }
        removed += cut.len();
        cut.sort_by_key(|r| std::cmp::Reverse(r.start));
        for r in cut {
            xml.replace_range(r, "");
        }
    }
    Ok(removed)
}

fn drop_overrides(xml: &str, drop: impl Fn(&str) -> bool) -> Result<String> {
    let doc = Document::parse(xml)?;
    let mut cut: Vec<_> = doc
        .descendants()
        .filter(|n| n.tag_name().name() == "Override" && n.attribute("PartName").is_some_and(&drop))
        .map(|n| n.range())
        .collect();
    cut.sort_by_key(|r| std::cmp::Reverse(r.start));
    let mut out = xml.to_string();
    for r in cut {
        out.replace_range(r, "");
    }
    Ok(out)
}

fn ct_defaults(xml: &str) -> Result<BTreeSet<String>> {
    let doc = Document::parse(xml)?;
    Ok(doc
        .descendants()
        .filter(|n| n.tag_name().name() == "Default")
        .filter_map(|n| n.attribute("Extension"))
        .map(str::to_ascii_lowercase)
        .collect())
}

fn media_type(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        _ => "application/octet-stream",
    }
}

/// Drop parts no relationship reaches any more (starting from the package
/// root) together with their content-type overrides.
/// XML parts (by name) and binary parts of a package being assembled.
type Parts = (Vec<(String, String)>, BTreeMap<String, Vec<u8>>);

fn prune(parts: Vec<(String, String)>, binary: BTreeMap<String, Vec<u8>>) -> Result<Parts> {
    let text: BTreeMap<String, String> = parts.into_iter().collect();
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut queue = vec![String::new()];
    while let Some(owner) = queue.pop() {
        let rels = if owner.is_empty() { "_rels/.rels".to_string() } else { rels_path(&owner) };
        let Some(xml) = text.get(&rels) else { continue };
        reached.insert(rels.clone());
        let doc = Document::parse(xml).with_context(|| format!("malformed {rels}"))?;
        for rel in doc.descendants().filter(|n| n.has_tag_name((REL_NS, "Relationship"))) {
            if rel.attribute("TargetMode") == Some("External") {
                continue;
            }
            let Some(target) = rel.attribute("Target") else { continue };
            let base = if owner.is_empty() { "x" } else { owner.as_str() };
            let path = if owner.is_empty() { target.trim_start_matches('/').to_string() } else { resolve(base, target)? };
            if reached.insert(path.clone()) {
                queue.push(path);
            }
        }
    }
    reached.insert("[Content_Types].xml".into());
    let ct = drop_overrides(&text["[Content_Types].xml"], |name| !reached.contains(name.trim_start_matches('/')))?;
    let mut kept: Vec<(String, String)> = text.into_iter().filter(|(p, _)| reached.contains(p)).collect();
    if let Some(entry) = kept.iter_mut().find(|(p, _)| p == "[Content_Types].xml") {
        entry.1 = ct;
    }
    let binary = binary.into_iter().filter(|(p, _)| reached.contains(p)).collect();
    Ok((kept, binary))
}

#[cfg(test)]
mod tests {
    //! A synthetic master (never a real organisation's): two layouts, its own
    //! logo, a 4:3 slide, an example slide with "private" content, a foreign
    //! custom-XML item, sections, a thumbnail and a dangling relationship.
    use super::*;
    use crate::{import_with_report, ZoneContent};
    use sldr_renderer::{LayoutRegistry, Zone};
    use std::io::Write;

    const PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
        0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
        0x54, 0x78, 0x9C, 0x63, 0x60, 0x00, 0x02, 0x00, 0x00, 0x05, 0x00, 0x01, 0xE2, 0x26, 0x05, 0x9B, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    const NS: &str = r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#;
    const RT: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

    fn ph(id: u32, name: &str, attrs: &str, xfrm: bool) -> String {
        let geo = if xfrm { r#"<a:xfrm><a:off x="457200" y="1600200"/><a:ext cx="8229600" cy="4525963"/></a:xfrm>"# } else { "" };
        format!(r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr/><p:nvPr><p:ph {attrs}/></p:nvPr></p:nvSpPr><p:spPr>{geo}</p:spPr><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>{name}</a:t></a:r></a:p></p:txBody></p:sp>"#)
    }

    fn layout(name: &str, shapes: &str) -> String {
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout {NS} preserve="1"><p:cSld name="{name}"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld></p:sldLayout>"#)
    }

    fn rels(items: &[(&str, &str, &str)]) -> String {
        let body: String = items.iter().map(|(id, kind, target)| format!(r#"<Relationship Id="{id}" Type="{RT}/{kind}" Target="{target}"/>"#)).collect();
        format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{body}</Relationships>"#)
    }

    /// A small 4:3 master package with an example slide.
    fn synthetic_master() -> Vec<u8> {
        let body_layout = layout(" Body Layout ", &[
            ph(2, "Title", r#"type="title""#, false),
            ph(3, "Subtitle", r#"type="body" sz="quarter" idx="13""#, true),
            ph(4, "Content", r#"idx="24""#, true),
            ph(5, "Footer", r#"type="ftr" sz="quarter" idx="10""#, false),
            ph(6, "Number", r#"type="sldNum" sz="quarter" idx="11""#, false),
        ].concat());
        let section_layout = layout("Section", &[ph(2, "Title", r#"type="title""#, false), ph(3, "Chapter", r#"type="body" idx="25""#, true)].concat());
        let master = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
<p:pic><p:nvPicPr><p:cNvPr id="2" name="Org logo"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId9"/></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/></a:xfrm></p:spPr></p:pic>
{}</p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/><p:sldLayoutId id="2147483650" r:id="rId2"/></p:sldLayoutIdLst></p:sldMaster>"#,
            ph(3, "Title", r#"type="title""#, true));
        let theme = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Org Theme"><a:themeElements><a:fontScheme name="Org"><a:majorFont><a:latin typeface="Org Sans"/></a:majorFont><a:minorFont><a:latin typeface="Org Text"/></a:minorFont></a:fontScheme></a:themeElements></a:theme>"#;
        let slide = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {NS}><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{}</p:spTree></p:cSld></p:sld>"#,
            ph(2, "Title", r#"type="title""#, false).replace(">Title<", ">Secret example content<"));
        let pres = format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation {NS}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst><p:sldIdLst><p:sldId id="256" r:id="rId2"/></p:sldIdLst><p:sldSz cx="9144000" cy="6858000" type="screen4x3"/><p:notesSz cx="6858000" cy="9144000"/><p:extLst><p:ext uri="{SECTIONS_EXT}"><p14:sectionLst xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main"><p14:section name="Intro" id="{{00000000-0000-0000-0000-000000000001}}"><p14:sldIdLst><p14:sldId id="256"/></p14:sldIdLst></p14:section></p14:sectionLst></p:ext></p:extLst></p:presentation>"#);
        let ct = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="jpeg" ContentType="image/jpeg"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/slideLayouts/slideLayout2.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
<Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
<Override PartName="/customXml/itemProps1.xml" ContentType="application/vnd.openxmlformats-officedocument.customXmlProperties+xml"/>
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/></Types>"#;
        let parts: Vec<(String, Vec<u8>)> = vec![
            ("[Content_Types].xml".into(), ct.into()),
            ("_rels/.rels".into(), format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{RT}/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="{RT}/extended-properties" Target="docProps/app.xml"/><Relationship Id="rId4" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/thumbnail" Target="docProps/thumbnail.jpeg"/></Relationships>"#).into_bytes()),
            ("docProps/core.xml".into(), br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:creator>Jane Example</dc:creator></cp:coreProperties>"#.to_vec()),
            ("docProps/app.xml".into(), br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><TitlesOfParts>Secret example content</TitlesOfParts></Properties>"#.to_vec()),
            ("docProps/thumbnail.jpeg".into(), b"\xFF\xD8 thumbnail of the example slide".to_vec()),
            ("ppt/presentation.xml".into(), pres.into_bytes()),
            ("ppt/_rels/presentation.xml.rels".into(), rels(&[("rId1", "slideMaster", "slideMasters/slideMaster1.xml"), ("rId2", "slide", "slides/slide1.xml"),
                ("rId3", "theme", "theme/theme1.xml"), ("rId4", "customXml", "../customXml/item1.xml"), ("rId5", "customXml", "../customXml/item9.xml")]).into_bytes()),
            ("ppt/slideMasters/slideMaster1.xml".into(), master.into_bytes()),
            ("ppt/slideMasters/_rels/slideMaster1.xml.rels".into(), rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                ("rId2", "slideLayout", "../slideLayouts/slideLayout2.xml"), ("rId3", "theme", "../theme/theme1.xml"), ("rId9", "image", "../media/image1.png")]).into_bytes()),
            ("ppt/slideLayouts/slideLayout1.xml".into(), body_layout.into_bytes()),
            ("ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(), rels(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]).into_bytes()),
            ("ppt/slideLayouts/slideLayout2.xml".into(), section_layout.into_bytes()),
            ("ppt/slideLayouts/_rels/slideLayout2.xml.rels".into(), rels(&[("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]).into_bytes()),
            ("ppt/theme/theme1.xml".into(), theme.into()),
            ("ppt/slides/slide1.xml".into(), slide.into_bytes()),
            ("ppt/slides/_rels/slide1.xml.rels".into(), rels(&[("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"), ("rId2", "image", "../media/image2.png")]).into_bytes()),
            ("ppt/media/image1.png".into(), PNG.to_vec()),
            ("ppt/media/image2.png".into(), [PNG, b"example"].concat()),
            ("customXml/item1.xml".into(), br#"<?xml version="1.0" encoding="UTF-8"?><b:Sources xmlns:b="http://schemas.openxmlformats.org/officeDocument/2006/bibliography"/>"#.to_vec()),
            ("customXml/itemProps1.xml".into(), br#"<?xml version="1.0" encoding="UTF-8" standalone="no"?><ds:datastoreItem ds:itemID="{11111111-1111-1111-1111-111111111111}" xmlns:ds="http://schemas.openxmlformats.org/officeDocument/2006/customXml"/>"#.to_vec()),
            ("customXml/_rels/item1.xml.rels".into(), rels(&[("rId1", "customXmlProps", "itemProps1.xml")]).into_bytes()),
        ];
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            for (name, bytes) in parts {
                zip.start_file(name, zip::write::FileOptions::<()>::default()).unwrap();
                zip.write_all(&bytes).unwrap();
            }
            zip.finish().unwrap();
        }
        buf
    }

    fn map() -> MasterMap {
        let mut m = MasterMap { slide_number: true, ..Default::default() };
        m.layouts.insert("framed".into(), LayoutMapping { layout: "Body Layout".into(),
            zones: [("headline", 0), ("subheadline", 13), ("content", 24), ("footer", 10)].map(|(k, v)| (k.to_string(), v)).into() });
        m.layouts.insert("framed-section".into(), LayoutMapping { layout: "Section".into(),
            zones: [("headline", 0), ("subheadline", 25)].map(|(k, v)| (k.to_string(), v)).into() });
        m.positions.insert("source".into(), Position { x: 5.0, y: 90.0, w: 40.0, h: 4.0 });
        m
    }

    fn read(bytes: &[u8], part: &str) -> String {
        let pkg = Package::read(bytes).unwrap();
        pkg.text(part).unwrap().to_string()
    }

    fn framed_with_source() -> sldr_renderer::LayoutDef {
        let mut def = LayoutRegistry::builtin().get("framed").unwrap().clone();
        def.zones.push(Zone { name: "source".into(), ph: Some("body".into()), idx: Some(9),
            rep: ZoneRep::PlaceholderText, x: 4.4, y: 88.0, w: 86.0, h: 4.0 });
        def
    }

    fn deck<'a>(framed: &'a sldr_renderer::LayoutDef, section: &'a sldr_renderer::LayoutDef) -> Vec<SlideInput<'a>> {
        vec![
            SlideInput { layout: section, details: Default::default(), fields: vec![
                ("headline".into(), ZoneContent::Text("Part one".into())),
                ("subheadline".into(), ZoneContent::Text("01".into())),
            ] },
            SlideInput { layout: framed, details: crate::SlideDetails { source_id: Some("s/two.md".into()), flavor_owned: vec!["footer".into()], ..Default::default() }, fields: vec![
                ("headline".into(), ZoneContent::Text("Findings".into())),
                ("subheadline".into(), ZoneContent::Text("What we saw".into())),
                ("content".into(), ZoneContent::Markdown("- first\n- second".into())),
                ("footer".into(), ZoneContent::Text("ORG".into())),
                ("source".into(), ZoneContent::Link { text: "Source: Example".into(), url: "https://example.org/a".into() }),
            ] },
        ]
    }

    #[test]
    fn inventory_lists_layouts_and_placeholders() {
        let inv = inspect(&synthetic_master()).unwrap();
        assert_eq!(inv.slide_size, (9_144_000, 6_858_000));
        assert_eq!(inv.slides, 1);
        let m = &inv.masters[0];
        assert_eq!(m.theme.as_deref(), Some("Org Theme"));
        assert_eq!(m.heading_font.as_deref(), Some("Org Sans"));
        let names: Vec<_> = m.layouts.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Body Layout", "Section"], "names are trimmed, in master order");
        let body: Vec<_> = m.layouts[0].placeholders.iter().map(|p| (p.kind.as_str(), p.idx)).collect();
        assert_eq!(body, [("title", 0), ("body", 13), ("obj", 24), ("ftr", 10), ("sldNum", 11)]);
        let skeleton = map_skeleton(&inv, "org.pptx");
        assert!(skeleton.contains("master = \"org.pptx\"") && skeleton.contains("\"Body Layout\": title 0, body 13, obj 24, ftr 10, sldNum 11"));
        assert!(!skeleton.lines().any(|l| l.starts_with("[layouts.")), "the skeleton maps nothing by itself");
    }

    #[test]
    fn slides_land_on_the_master_layouts_and_placeholders() {
        let (framed, section) = (framed_with_source(), LayoutRegistry::builtin().get("framed-section").unwrap().clone());
        let input = synthetic_master();
        let before = input.clone();
        let out = build_deck_on_master(&input, &map(), "Deck", &deck(&framed, &section)).unwrap();
        assert_eq!(input, before, "the master bytes are never modified");
        let bytes = out.value;

        // Layout relationships point at the master's own layouts.
        assert!(read(&bytes, "ppt/slides/_rels/slide1.xml.rels").contains("Target=\"../slideLayouts/slideLayout2.xml\""));
        assert!(read(&bytes, "ppt/slides/_rels/slide2.xml.rels").contains("Target=\"../slideLayouts/slideLayout1.xml\""));
        let s2 = read(&bytes, "ppt/slides/slide2.xml");
        assert!(s2.contains(r#"<p:ph type="title"/>"#), "title keeps no idx");
        assert!(s2.contains(r#"<p:ph type="body" idx="13"/>"#) && s2.contains(r#"<p:ph idx="24"/>"#) && s2.contains(r#"<p:ph type="ftr" idx="10"/>"#));
        assert!(s2.contains(r#"<p:ph type="sldNum" idx="11"/>"#) && s2.contains(r#"type="slidenum"><a:rPr lang="en"/><a:t>2</a:t>"#));
        // No placeholder for the source on the master: a text box at the mapped box, scaled to 4:3.
        assert!(s2.contains(r#"txBox="1""#) && s2.contains(&format!("<a:off x=\"{}\" y=\"{}\"/>", 457_200, 6_172_200)));
        assert!(!s2.contains("sz=\"quarter\""), "placeholder size hints are not copied");
        assert!(read(&bytes, "ppt/slides/slide1.xml").contains(r#"<p:ph type="body" idx="25"/>"#));

        // The master's example slide, its media, sections and document metadata are gone.
        let pkg = Package::read(&bytes).unwrap();
        assert!(!pkg.parts.contains_key("ppt/media/image2.png") && !pkg.parts.contains_key("docProps/thumbnail.jpeg"));
        assert!(pkg.parts.contains_key("ppt/media/image1.png"), "master artwork is kept");
        let all: String = pkg.parts.values().map(|b| String::from_utf8_lossy(b).into_owned()).collect();
        assert!(!all.contains("Secret example content") && !all.contains("Jane Example"));
        let pres = read(&bytes, "ppt/presentation.xml");
        assert!(!pres.contains("sectionLst") && pres.contains(r#"<p:sldSz cx="9144000" cy="6858000""#));
        assert_eq!(pres.matches("<p:sldId ").count(), 2);
        // The foreign custom-XML item stays; the sldr manifest takes the next free one.
        assert!(pkg.parts.contains_key("customXml/item1.xml") && read(&bytes, "customXml/item2.xml").contains("sldr:manifest"));
        assert!(out.report.findings.iter().any(|f| f.feature == "master_dangling_relationships"));
        assert!(out.report.findings.iter().any(|f| f.feature == "master_template" && f.remedy.contains("sha256")));
    }

    #[test]
    fn export_is_deterministic_and_round_trips_strictly() {
        let (framed, section) = (framed_with_source(), LayoutRegistry::builtin().get("framed-section").unwrap().clone());
        let a = build_deck_on_master(&synthetic_master(), &map(), "Deck", &deck(&framed, &section)).unwrap().value;
        let b = build_deck_on_master(&synthetic_master(), &map(), "Deck", &deck(&framed, &section)).unwrap().value;
        assert_eq!(a, b);

        let imported = import_with_report(&a).unwrap();
        imported.report.enforce(false).expect("an unedited master-backed deck imports under --strict");
        let s = &imported.value[1];
        assert_eq!((s.layout.as_str(), s.title.as_deref(), s.subtitle.as_deref()), ("framed", Some("Findings"), Some("What we saw")));
        assert!(s.body.contains("- first") && s.body.contains("- second"));
        assert_eq!((s.source.as_deref(), s.source_url.as_deref()), (Some("Example"), Some("https://example.org/a")));
        assert_eq!(s.footer, None, "the flavor-owned footer is not slide content");
        let number = s.zones.iter().find(|z| z.zone == crate::SLIDE_NUMBER_ZONE).unwrap();
        assert_eq!(number.owner, "flavor");
    }

    #[test]
    fn a_map_that_does_not_fit_fails_before_writing() {
        let (framed, section) = (framed_with_source(), LayoutRegistry::builtin().get("framed-section").unwrap().clone());
        let image = LayoutRegistry::builtin().get("framed-image").unwrap().clone();
        let mut slides = deck(&framed, &section);
        slides.push(SlideInput { layout: &image, details: Default::default(), fields: vec![] });
        let err = format!("{:#}", build_deck_on_master(&synthetic_master(), &map(), "Deck", &slides).unwrap_err());
        assert!(err.contains("\"framed-image\" is not mapped"), "{err}");

        let mut bad = map();
        bad.layouts.get_mut("framed").unwrap().zones.insert("content".into(), 99);
        bad.layouts.get_mut("framed-section").unwrap().layout = "Nope".into();
        bad.layouts.get_mut("framed-section").unwrap().zones.insert("typo".into(), 0);
        let err = format!("{:#}", build_deck_on_master(&synthetic_master(), &bad, "Deck", &deck(&framed, &section)).unwrap_err());
        assert!(err.contains("no placeholder idx 99") && err.contains("obj 24"), "{err}");
        assert!(err.contains("\"Nope\" does not exist") && err.contains("\"Body Layout\""), "{err}");
        assert!(err.contains("maps zone \"typo\""), "{err}");

        let mut fallback = map();
        fallback.fallback = Some("Section".into());
        assert!(build_deck_on_master(&synthetic_master(), &fallback, "Deck", &slides).is_ok(), "an explicit fallback covers unmapped layouts");
        let wrong_master = MasterMap { slide_master: Some(2), ..map() };
        assert!(format!("{:#}", build_deck_on_master(&synthetic_master(), &wrong_master, "Deck", &slides).unwrap_err()).contains("1 slide master"));
    }
}
