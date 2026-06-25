//! Native OOXML PowerPoint export/import for sldr — a **satellite** crate
//! (ADR-0001: derives from the model, never colonizes core formats).
//!
//! Today this hosts the **template** generator (trx-4s9s.3); the filled-deck
//! generator (trx-4s9s.4) and the round-trip import parser (trx-4s9s.5) will
//! live here too, sharing the part-builders below.
//!
//! This is the real-OOXML path, distinct from the lossy screenshot writer in
//! `sldr_renderer::pptx`. It emits a `.pptx` that is theme + slideMaster +
//! slideLayouts only — **empty of slides** — so an org user can author new
//! branded slides directly in PowerPoint and round-trip them back later
//! (Phase 2, trx-4s9s.5). Filled decks are Phase 1b (trx-4s9s.4); they reuse
//! every part-builder here and add `ppt/slides/`.
//!
//! The mapping is deterministic, not heuristic (ADR-0008):
//!
//! | sldr                         | PPTX                                  |
//! |------------------------------|---------------------------------------|
//! | flavor colors                | theme `clrScheme`                     |
//! | flavor fonts                 | theme `fontScheme`                    |
//! | flavor background            | slideMaster background fill           |
//! | layout zones (`Zone`)        | slideLayout placeholders (in EMU)     |
//! | zone `x/y/w/h` (% of box)    | placeholder `xfrm` off/ext            |
//!
//! Only zones whose representation is [`ZoneRep::PlaceholderText`] become
//! template placeholders — a *template* carries empty editable frames, not
//! pictures or baked rasters (those are filled-deck concerns). A layout with
//! no such zones is skipped (and reported by the caller), never silently
//! emitted blank.
//!
//! The OOXML shape is the one validated to open in real PowerPoint in
//! `docs/pptx-spike` — same part set, relationships, and placeholder taxonomy.

use std::io::Write;

use anyhow::{bail, Result};

use sldr_renderer::{LayoutDef, Zone, ZoneRep};

mod deck;
mod mdooxml;

pub use deck::{build_deck, SlideInput, ZoneContent};

/// 16:9 slide box in EMU (English Metric Units). `screen16x9`.
pub(crate) const SLIDE_W_EMU: i64 = 12_192_000;
pub(crate) const SLIDE_H_EMU: i64 = 6_858_000;

/// Percent of the slide box → EMU on each axis.
pub(crate) fn emu_x(pct: f64) -> i64 {
    (SLIDE_W_EMU as f64 * pct / 100.0).round() as i64
}
pub(crate) fn emu_y(pct: f64) -> i64 {
    (SLIDE_H_EMU as f64 * pct / 100.0).round() as i64
}

/// Resolved theme colors + fonts for the package. All colors are 6-digit
/// uppercase hex **without** a leading `#` (OOXML `srgbClr val=`). Derived
/// from a flavor by [`Theme::from_parts`]; the caller maps `flavor.colors`
/// and `flavor.typography` into these slots.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Theme name (shown in PowerPoint's theme gallery).
    pub name: String,
    /// `dk1` — slide background / dark base.
    pub dk1: String,
    /// `lt1` — primary text on the dark base.
    pub lt1: String,
    /// `accent1` — primary brand accent.
    pub accent1: String,
    /// `accent2` — secondary accent.
    pub accent2: String,
    /// `lt2` — subtle surface / dim.
    pub lt2: String,
    /// `accent4` — muted (dim text, borders).
    pub accent4: String,
    /// `+mj-lt` major (heading) latin typeface.
    pub major_font: String,
    /// `+mn-lt` minor (body) latin typeface.
    pub minor_font: String,
}

impl Theme {
    /// Build a theme from already-resolved hex colors and font names. Each
    /// color is normalized to bare 6-digit uppercase hex; anything that is not
    /// a clean hex triple falls back to the matching default so the package is
    /// always valid OOXML (never an empty `val=`).
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        name: impl Into<String>,
        dk1: Option<&str>,
        lt1: Option<&str>,
        accent1: Option<&str>,
        accent2: Option<&str>,
        lt2: Option<&str>,
        accent4: Option<&str>,
        major_font: Option<&str>,
        minor_font: Option<&str>,
    ) -> Self {
        Theme {
            name: name.into(),
            dk1: norm_hex(dk1).unwrap_or_else(|| "0F172A".into()),
            lt1: norm_hex(lt1).unwrap_or_else(|| "FFFFFF".into()),
            accent1: norm_hex(accent1).unwrap_or_else(|| "3B82F6".into()),
            accent2: norm_hex(accent2).unwrap_or_else(|| "F59E0B".into()),
            lt2: norm_hex(lt2).unwrap_or_else(|| "E2E8F0".into()),
            accent4: norm_hex(accent4).unwrap_or_else(|| "94A3B8".into()),
            major_font: clean_font(major_font),
            minor_font: clean_font(minor_font),
        }
    }

    /// Map a sldr flavor's colors + typography into a PPTX theme. The flavor
    /// is the single source of brand truth (ADR-0001) — this is a pure
    /// projection of it, no theme colors invented here beyond the safe
    /// fallbacks in [`Theme::from_parts`].
    pub fn from_flavor(flavor: &sldr_core::flavor::Flavor) -> Self {
        let c = &flavor.colors;
        let t = &flavor.typography;
        let name = flavor
            .display_name
            .as_deref()
            .unwrap_or(&flavor.name)
            .to_string();
        Theme::from_parts(
            name,
            c.background.as_deref(),
            c.text.as_deref(),
            c.accent.or_ref(&c.primary),
            c.secondary.or_ref(&c.accent),
            c.surface.as_deref(),
            c.text_dim.or_ref(&c.muted),
            t.heading_font.as_deref(),
            t.body_font.as_deref(),
        )
    }
}

/// Tiny helper: `self` if `Some`, else fall back to another `Option`'s value.
/// Keeps the flavor→theme mapping readable (accent ?? primary, …).
trait OrRef {
    fn or_ref<'a>(&'a self, other: &'a Option<String>) -> Option<&'a str>;
}
impl OrRef for Option<String> {
    fn or_ref<'a>(&'a self, other: &'a Option<String>) -> Option<&'a str> {
        self.as_deref().or(other.as_deref())
    }
}

/// A layout selected for a slideLayout: its name, the PPTX layout `type`
/// token, and the placeholder-text zones it contributes (already filtered).
/// Shared by template and deck generation.
pub(crate) struct TemplateLayout<'a> {
    pub(crate) name: &'a str,
    pub(crate) typ: &'static str,
    pub(crate) placeholders: Vec<&'a Zone>,
}

/// Turn resolved layout defs into [`TemplateLayout`]s, keeping only their
/// placeholder-text zones and dropping any layout left with none.
pub(crate) fn to_template_layouts<'a>(layouts: &[&'a LayoutDef]) -> Vec<TemplateLayout<'a>> {
    layouts
        .iter()
        .map(|def| {
            let placeholders: Vec<&Zone> = def
                .zones
                .iter()
                .filter(|z| z.rep == ZoneRep::PlaceholderText && z.ph.is_some())
                .collect();
            TemplateLayout {
                name: def.name.as_str(),
                typ: layout_type(&placeholders),
                placeholders,
            }
        })
        .filter(|t| !t.placeholders.is_empty())
        .collect()
}

/// Pick the layouts that can become template slideLayouts: those declaring at
/// least one `placeholder-text` zone. Returns them in stable name order, plus
/// the names of zone-bearing layouts that were *not* template-eligible (their
/// zones are all picture/shape/bake — filled-deck only) so the caller can
/// report honestly rather than dropping silently.
pub fn select_layouts<'a, I>(defs: I) -> (Vec<&'a LayoutDef>, Vec<&'a str>)
where
    I: IntoIterator<Item = &'a LayoutDef>,
{
    let mut eligible = Vec::new();
    let mut skipped = Vec::new();
    for def in defs {
        if def.zones.is_empty() {
            continue;
        }
        if def
            .zones
            .iter()
            .any(|z| z.rep == ZoneRep::PlaceholderText && z.ph.is_some())
        {
            eligible.push(def);
        } else {
            skipped.push(def.name.as_str());
        }
    }
    eligible.sort_by(|a, b| a.name.cmp(&b.name));
    skipped.sort_unstable();
    (eligible, skipped)
}

/// Generate the template `.pptx` bytes for `theme` over `layouts`. Each layout
/// must carry at least one placeholder-text zone (use [`select_layouts`] to
/// filter first). Fails loudly if `layouts` is empty — a template with no
/// slideLayouts is never what the caller wanted.
pub fn build_template(theme: &Theme, layouts: &[&LayoutDef]) -> Result<Vec<u8>> {
    if layouts.is_empty() {
        bail!("PPTX template needs at least one layout with placeholder zones");
    }

    let selected = to_template_layouts(layouts);

    if selected.is_empty() {
        bail!("none of the given layouts declare placeholder-text zones");
    }

    let mut parts: Vec<(String, String)> = Vec::new();
    let n = selected.len();

    parts.push(("[Content_Types].xml".into(), content_types(n, 0)));
    parts.push(("_rels/.rels".into(), root_rels()));
    parts.push((
        "docProps/core.xml".into(),
        core_props(&format!("{} template", theme.name)),
    ));
    parts.push(("docProps/app.xml".into(), app_props()));
    parts.push((
        "ppt/_rels/presentation.xml.rels".into(),
        presentation_rels(0),
    ));
    parts.push(("ppt/presentation.xml".into(), presentation_xml(0)));
    parts.push(("ppt/presProps.xml".into(), pres_props()));
    parts.push(("ppt/theme/theme1.xml".into(), theme_xml(theme)));
    parts.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        slide_master_rels(n),
    ));
    parts.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        slide_master_xml(n),
    ));

    for (i, layout) in selected.iter().enumerate() {
        let n1 = i + 1;
        parts.push((
            format!("ppt/slideLayouts/slideLayout{n1}.xml"),
            slide_layout_xml(layout),
        ));
        parts.push((
            format!("ppt/slideLayouts/_rels/slideLayout{n1}.xml.rels"),
            slide_layout_rels(),
        ));
    }

    zip_parts(&parts)
}

/// PPTX slideLayout `type` token: `twoObj` when ≥2 body placeholders sit
/// beside the title, else the generic `obj`.
pub(crate) fn layout_type(placeholders: &[&Zone]) -> &'static str {
    let body_count = placeholders
        .iter()
        .filter(|z| z.ph.as_deref() == Some("body"))
        .count();
    if body_count >= 2 {
        "twoObj"
    } else {
        "obj"
    }
}

// ---- part builders (ported from docs/pptx-spike/gen.py) ----------------

pub(crate) fn content_types(layout_count: usize, slide_count: usize) -> String {
    let mut overrides = String::new();
    for n in 1..=layout_count {
        overrides.push_str(&format!(
            "<Override PartName=\"/ppt/slideLayouts/slideLayout{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>"
        ));
    }
    for n in 1..=slide_count {
        overrides.push_str(&format!(
            "<Override PartName=\"/ppt/slides/slide{n}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Default Extension="png" ContentType="image/png"/>
<Default Extension="jpeg" ContentType="image/jpeg"/>
<Default Extension="jpg" ContentType="image/jpeg"/>
<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
<Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
{overrides}
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#
    )
}

pub(crate) fn root_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#
        .into()
}

pub(crate) fn core_props(title: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{}</dc:title><dc:creator>sldr</dc:creator></cp:coreProperties>"#,
        xml_escape(title)
    )
}

pub(crate) fn app_props() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Application>sldr</Application></Properties>"#
        .into()
}

/// Presentation relationships. `rId1`=master, `rId2`=theme, `rId3`=presProps,
/// then `rId(4+i)`=slide{i+1}. `slide_count` 0 → a template (no slides).
pub(crate) fn presentation_rels(slide_count: usize) -> String {
    let mut slides = String::new();
    for i in 0..slide_count {
        let rid = 4 + i;
        let n = i + 1;
        slides.push_str(&format!(
            "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{n}.xml\"/>"
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>
<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps" Target="presProps.xml"/>
{slides}
</Relationships>"#
    )
}

pub(crate) fn presentation_xml(slide_count: usize) -> String {
    let mut sld_ids = String::new();
    for i in 0..slide_count {
        let id = 256 + i;
        let rid = 4 + i;
        sld_ids.push_str(&format!("<p:sldId id=\"{id}\" r:id=\"rId{rid}\"/>"));
    }
    let sld_id_lst = if slide_count == 0 {
        String::new()
    } else {
        format!("<p:sldIdLst>{sld_ids}</p:sldIdLst>")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>
{sld_id_lst}
<p:sldSz cx="{SLIDE_W_EMU}" cy="{SLIDE_H_EMU}" type="screen16x9"/>
<p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>"#
    )
}

pub(crate) fn pres_props() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentationPr xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>"#
        .into()
}

pub(crate) fn theme_xml(t: &Theme) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="{name}">
<a:themeElements>
<a:clrScheme name="sldr">
<a:dk1><a:srgbClr val="{dk1}"/></a:dk1><a:lt1><a:srgbClr val="{lt1}"/></a:lt1>
<a:dk2><a:srgbClr val="{dk1}"/></a:dk2><a:lt2><a:srgbClr val="{lt2}"/></a:lt2>
<a:accent1><a:srgbClr val="{accent1}"/></a:accent1><a:accent2><a:srgbClr val="{accent2}"/></a:accent2>
<a:accent3><a:srgbClr val="{lt2}"/></a:accent3><a:accent4><a:srgbClr val="{accent4}"/></a:accent4>
<a:accent5><a:srgbClr val="{accent1}"/></a:accent5><a:accent6><a:srgbClr val="{accent2}"/></a:accent6>
<a:hlink><a:srgbClr val="{accent1}"/></a:hlink><a:folHlink><a:srgbClr val="{accent4}"/></a:folHlink>
</a:clrScheme>
<a:fontScheme name="sldr">
<a:majorFont><a:latin typeface="{major}"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>
<a:minorFont><a:latin typeface="{minor}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont>
</a:fontScheme>
<a:fmtScheme name="sldr">
<a:fillStyleLst>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
</a:fillStyleLst>
<a:lnStyleLst>
<a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
<a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>
</a:lnStyleLst>
<a:effectStyleLst>
<a:effectStyle><a:effectLst/></a:effectStyle>
<a:effectStyle><a:effectLst/></a:effectStyle>
<a:effectStyle><a:effectLst/></a:effectStyle>
</a:effectStyleLst>
<a:bgFillStyleLst>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
</a:bgFillStyleLst>
</a:fmtScheme>
</a:themeElements>
</a:theme>"#,
        name = xml_escape(&t.name),
        dk1 = t.dk1,
        lt1 = t.lt1,
        lt2 = t.lt2,
        accent1 = t.accent1,
        accent2 = t.accent2,
        accent4 = t.accent4,
        major = xml_escape(&t.major_font),
        minor = xml_escape(&t.minor_font),
    )
}

pub(crate) fn slide_master_rels(layout_count: usize) -> String {
    let mut rels = String::new();
    for n in 1..=layout_count {
        rels.push_str(&format!(
            "<Relationship Id=\"rId{n}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout{n}.xml\"/>"
        ));
    }
    let theme_id = layout_count + 1;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
{rels}
<Relationship Id="rId{theme_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#
    )
}

pub(crate) fn slide_master_xml(layout_count: usize) -> String {
    let mut layout_ids = String::new();
    for n in 1..=layout_count {
        let id = 2147483648u64 + n as u64;
        layout_ids.push_str(&format!("<p:sldLayoutId id=\"{id}\" r:id=\"rId{n}\"/>"));
    }
    // Master title placeholder + dk1 background. Body style declares the
    // project-default SQUARE bullet (U+25AA) so authored bullets render square.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:bg><p:bgPr><a:solidFill><a:schemeClr val="dk1"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>
<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Title Placeholder"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{title_x}" y="{title_y}"/><a:ext cx="{title_cx}" cy="{title_cy}"/></a:xfrm></p:spPr>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>Master title</a:t></a:r></a:p></p:txBody></p:sp>
</p:spTree></p:cSld>
<p:clrMap bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>
<p:sldLayoutIdLst>{layout_ids}</p:sldLayoutIdLst>
<p:txStyles>
<p:titleStyle><a:lvl1pPr><a:defRPr sz="2800" b="1"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mj-lt"/></a:defRPr></a:lvl1pPr></p:titleStyle>
<p:bodyStyle><a:lvl1pPr marL="285750" indent="-285750"><a:buFont typeface="Arial"/><a:buChar char="&#9642;"/><a:defRPr sz="1800"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill><a:latin typeface="+mn-lt"/></a:defRPr></a:lvl1pPr></p:bodyStyle>
<p:otherStyle><a:defPPr><a:defRPr lang="en-US"/></a:defPPr></p:otherStyle>
</p:txStyles>
</p:sldMaster>"#,
        title_x = emu_x(4.4),
        title_y = emu_y(6.3),
        title_cx = emu_x(70.0),
        title_cy = emu_y(12.0),
    )
}

pub(crate) fn slide_layout_xml(layout: &TemplateLayout) -> String {
    let mut sps = String::new();
    for (i, zone) in layout.placeholders.iter().enumerate() {
        let id = i + 2; // id 1 is the group shape
        let ph = zone.ph.as_deref().unwrap_or("body");
        let idx_attr = match zone.idx {
            Some(idx) => format!(" idx=\"{idx}\""),
            None => String::new(),
        };
        let label = xml_escape(&title_case(&zone.name));
        sps.push_str(&format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{ph}"{idx_attr}/></p:nvPr></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm></p:spPr>
<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="en-US"/><a:t>{label}</a:t></a:r></a:p></p:txBody></p:sp>"#,
            x = emu_x(zone.x),
            y = emu_y(zone.y),
            cx = emu_x(zone.w),
            cy = emu_y(zone.h),
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="{typ}" preserve="1">
<p:cSld name="{name}"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
{sps}
</p:spTree></p:cSld>
<p:clrMapOvr><a:overrideClrMapping bg1="dk1" tx1="lt1" bg2="dk2" tx2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/></p:clrMapOvr>
</p:sldLayout>"#,
        typ = layout.typ,
        name = xml_escape(layout.name),
    )
}

pub(crate) fn slide_layout_rels() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#
        .into()
}

/// Pack the named XML parts into a deflated zip — a `.pptx` package.
pub(crate) fn zip_parts(parts: &[(String, String)]) -> Result<Vec<u8>> {
    zip_mixed(parts, &[])
}

/// Pack XML parts (deflated) plus binary media parts (stored — images are
/// already compressed) into a `.pptx` package.
pub(crate) fn zip_mixed(text: &[(String, String)], media: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zip = zip::ZipWriter::new(cursor);
        let deflate = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (path, content) in text {
            zip.start_file(path, deflate)?;
            zip.write_all(content.as_bytes())?;
        }
        let stored = zip::write::FileOptions::<()>::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (path, bytes) in media {
            zip.start_file(path, stored)?;
            zip.write_all(bytes)?;
        }
        zip.finish()?;
    }
    Ok(buf)
}

// ---- small helpers -----------------------------------------------------

/// Normalize a CSS color to bare 6-digit uppercase hex, or `None` if it is
/// not a plain `#rgb`/`#rrggbb` literal (rgb()/named colors aren't OOXML
/// `srgbClr` values — caller's default fills in).
fn norm_hex(color: Option<&str>) -> Option<String> {
    let c = color?.trim().trim_start_matches('#');
    let hex = match c.len() {
        6 => c.to_string(),
        3 => c.chars().flat_map(|ch| [ch, ch]).collect(),
        _ => return None,
    };
    if hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(hex.to_ascii_uppercase())
    } else {
        None
    }
}

/// Take the first family from a CSS font stack and strip quotes — OOXML wants
/// a single typeface name. Falls back to Arial.
fn clean_font(font: Option<&str>) -> String {
    font.and_then(|f| f.split(',').next())
        .map(|f| f.trim().trim_matches(['"', '\'']).to_string())
        .filter(|f| !f.is_empty())
        .unwrap_or_else(|| "Arial".into())
}

/// `headline` → `Headline`, `two-cols` → `Two-cols`. For placeholder labels.
pub(crate) fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

pub(crate) fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sldr_renderer::LayoutRegistry;

    fn theme() -> Theme {
        Theme::from_parts(
            "sldr demo",
            Some("#0F172A"),
            Some("#FFFFFF"),
            Some("#3B82F6"),
            Some("#F59E0B"),
            Some("#E2E8F0"),
            Some("#94A3B8"),
            Some("Inter, sans-serif"),
            Some("Inter"),
        )
    }

    fn read_part(bytes: &[u8], path: &str) -> String {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut f = zip.by_name(path).unwrap();
        let mut s = String::new();
        std::io::Read::read_to_string(&mut f, &mut s).unwrap();
        s
    }

    #[test]
    fn test_select_layouts_picks_zone_bearing() {
        let reg = LayoutRegistry::builtin();
        let defs: Vec<_> = reg
            .names()
            .into_iter()
            .map(|n| reg.get(&n).unwrap())
            .collect();
        let (eligible, _skipped) = select_layouts(defs);
        let names: Vec<&str> = eligible.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"framed"));
        assert!(names.contains(&"two-cols"));
        // default has no zones → not eligible.
        assert!(!names.contains(&"default"));
    }

    #[test]
    fn test_build_template_produces_expected_parts() {
        let reg = LayoutRegistry::builtin();
        let framed = reg.get("framed").unwrap();
        let two = reg.get("two-cols").unwrap();
        let bytes = build_template(&theme(), &[framed, two]).unwrap();

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/theme/theme1.xml",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/slideLayouts/slideLayout2.xml",
        ] {
            assert!(names.contains(&required.to_string()), "missing {required}");
        }
        // No slides in a template.
        assert!(!names.iter().any(|n| n.starts_with("ppt/slides/")));
    }

    #[test]
    fn test_theme_colors_land_in_clrscheme() {
        let bytes = build_template(
            &theme(),
            &[LayoutRegistry::builtin().get("framed").unwrap()],
        )
        .unwrap();
        let t = read_part(&bytes, "ppt/theme/theme1.xml");
        assert!(t.contains(r#"<a:dk1><a:srgbClr val="0F172A"/></a:dk1>"#));
        assert!(t.contains(r#"<a:accent1><a:srgbClr val="3B82F6"/></a:accent1>"#));
        // Font stack reduced to a single family.
        assert!(t.contains(r#"<a:latin typeface="Inter"/>"#));
        assert!(t.contains(r#"name="sldr demo""#));
    }

    #[test]
    fn test_framed_layout_has_three_placeholders_in_emu() {
        let bytes = build_template(
            &theme(),
            &[LayoutRegistry::builtin().get("framed").unwrap()],
        )
        .unwrap();
        let l = read_part(&bytes, "ppt/slideLayouts/slideLayout1.xml");
        // title + two body placeholders.
        assert_eq!(l.matches("<p:ph ").count(), 3);
        assert!(l.contains(r#"type="title""#));
        assert!(l.contains(r#"type="body" idx="1""#));
        assert!(l.contains(r#"type="body" idx="2""#));
        // headline x = 4.4% of 12192000 = 536448 EMU.
        assert!(l.contains(&format!("x=\"{}\"", emu_x(4.4))));
    }

    #[test]
    fn test_master_declares_square_bullet() {
        let bytes = build_template(
            &theme(),
            &[LayoutRegistry::builtin().get("framed").unwrap()],
        )
        .unwrap();
        let m = read_part(&bytes, "ppt/slideMasters/slideMaster1.xml");
        assert!(m.contains(r#"<a:buChar char="&#9642;"/>"#));
    }

    #[test]
    fn test_empty_layouts_fails_loud() {
        let err = build_template(&theme(), &[]).unwrap_err().to_string();
        assert!(err.contains("at least one layout"));
    }

    #[test]
    fn test_norm_hex_and_clean_font() {
        assert_eq!(norm_hex(Some("#0f172a")).as_deref(), Some("0F172A"));
        assert_eq!(norm_hex(Some("fff")).as_deref(), Some("FFFFFF"));
        assert_eq!(norm_hex(Some("rgb(1,2,3)")), None);
        assert_eq!(norm_hex(None), None);
        assert_eq!(clean_font(Some("\"Inter\", sans-serif")), "Inter");
        assert_eq!(clean_font(None), "Arial");
    }
}
