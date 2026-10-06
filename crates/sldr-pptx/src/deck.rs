//! Filled **deck** generation (trx-4s9s.4): markdown slides → an *editable*
//! PowerPoint, not a screenshot. Reuses the template machinery (theme, master,
//! slideLayouts) and adds `ppt/slides/` — each slide reuses its layout's
//! placeholders, filled with the slide's chrome and body as native text.
//!
//! Scope of this module: the `placeholder-text` half (chrome + body, with
//! square bullets). Picture and bake zones are layered on separately
//! (trx-4s9s.4 picture/bake stage). A slide whose layout declares no
//! placeholder-text zones can't be represented natively — `build_deck` fails
//! loud naming it, rather than emitting a blank slide.

use std::collections::HashMap;

use anyhow::{bail, Result};

use sldr_renderer::LayoutDef;

use crate::{mdooxml, ZoneRep};

/// Content destined for one layout zone of a slide, keyed by the zone's name.
pub enum ZoneContent {
    /// Plain single-line chrome (headline, footer, rendered source) — one
    /// bullet-less paragraph.
    Text(String),
    /// A markdown body segment (content / left / right / heading) — converted
    /// to bulleted/plain OOXML paragraphs.
    Markdown(String),
    /// Plain attribution text with a real external hyperlink (never fetched).
    Link { text: String, url: String },
    /// A raster image embedded as a positioned `<p:pic>`. The caller resolves
    /// the bytes; `ext` is the media extension (`png` / `jpeg` / `gif`).
    /// `fit` carries the image's intrinsic `(width, height)` in pixels when the
    /// picture should be aspect-fit (centered) inside its zone — used for baked
    /// diagrams, which must not stretch. `None` fills the zone box (the column
    /// images in image-left/right, which are meant to fill).
    Picture {
        bytes: Vec<u8>,
        ext: String,
        fit: Option<(u32, u32)>,
    },
    /// A video embedded as a movie (`mp4` / `webm` / `mov` / `m4v`) at the
    /// zone box, shown as its poster frame until played. Without a poster a
    /// blank frame is used so the slide is never empty.
    Video {
        bytes: Vec<u8>,
        ext: String,
        poster: Option<(Vec<u8>, String)>,
    },
}

/// Extensions the writer embeds as movies, with their content types.
pub const VIDEO_TYPES: [(&str, &str); 4] =
    [("mp4", "video/mp4"), ("m4v", "video/mp4"), ("webm", "video/webm"), ("mov", "video/quicktime")];

/// A 1×1 transparent PNG: the poster of a video that has none.
const BLANK_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41,
    0x54, 0x78, 0x9C, 0x63, 0x60, 0x00, 0x02, 0x00, 0x00, 0x05, 0x00, 0x01, 0xE2, 0x26, 0x05, 0x9B, 0x00, 0x00, 0x00, 0x00,
    0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
];

/// One slide to export: the layout it uses and the content for each zone.
/// The caller (CLI) decides which zone gets which content and whether it is
/// chrome text or a markdown body — keeping this crate agnostic about the
/// frontmatter↔zone naming convention.
pub struct SlideInput<'a> {
    pub layout: &'a LayoutDef,
    /// `(zone_name, content)` pairs. Zones without an entry render empty.
    pub fields: Vec<(String, ZoneContent)>,
    /// Source ownership and per-slide notes; never an absolute file path.
    pub details: SlideDetails,
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SlideDetails {
    pub source_id: Option<String>,
    pub step: usize,
    pub notes: Option<String>,
    pub language: Option<String>,
    /// Zone names whose content came from the flavor (e.g. a flavor footer),
    /// not the slide. Recorded so import never copies them into the slide.
    #[serde(default)]
    pub flavor_owned: Vec<String>,
    /// Zone names exported as a picture of the rendered layout (`rep=bake`):
    /// a diagram the slide's markdown generates, not content of its own.
    /// Recorded so import never writes that picture back into the slide.
    #[serde(default)]
    pub rendered: Vec<String>,
}

/// Generate an editable deck `.pptx` from `slides`. `title` becomes the
/// document title. Every slide's layout must declare at least one
/// `placeholder-text` zone; otherwise this fails loud (use `--flatten` for the
/// screenshot path, or annotate the layout with zones).
pub fn build_deck(theme: &crate::Theme, title: &str, slides: &[SlideInput]) -> Result<Vec<u8>> {
    let report = crate::preflight::deck(slides);
    report.enforce(false)?;
    build_deck_bytes(theme, title, slides)
}

/// Generate with explicit diagnostics. Call `report.enforce` before publishing;
/// conflicting/unsafe inputs are rejected even when lossy output is requested.
pub fn build_deck_with_report(theme: &crate::Theme, title: &str, slides: &[SlideInput]) -> Result<crate::Conversion<Vec<u8>>> {
    let report = crate::preflight::deck(slides);
    report.enforce(true)?;
    let value = build_deck_bytes(theme, title, slides)?;
    Ok(crate::Conversion { value, report })
}

fn build_deck_bytes(theme: &crate::Theme, title: &str, slides: &[SlideInput]) -> Result<Vec<u8>> {
    if slides.is_empty() {
        bail!("PPTX deck needs at least one slide");
    }

    // Distinct layouts in first-seen order; every slide layout must be
    // placeholder-eligible or we can't represent it natively.
    let mut distinct: Vec<&LayoutDef> = Vec::new();
    let mut not_eligible: Vec<&str> = Vec::new();
    for slide in slides {
        // Exportable if it declares any representable zone — an editable text
        // placeholder OR a picture (a picture-only image layout is fine).
        // Exportable if it declares any representable zone: a text
        // placeholder, a free text box, a picture, or a baked region.
        let eligible = slide.layout.zones.iter().any(|z| matches!(z.rep, ZoneRep::PlaceholderText | ZoneRep::Picture | ZoneRep::Bake));
        if !eligible {
            if !not_eligible.contains(&slide.layout.name.as_str()) {
                not_eligible.push(slide.layout.name.as_str());
            }
            continue;
        }
        if !distinct.iter().any(|d| d.name == slide.layout.name) {
            distinct.push(slide.layout);
        }
    }

    if !not_eligible.is_empty() {
        not_eligible.sort_unstable();
        bail!(
            "These layouts have no PPTX placeholder zones, so their slides can't \
             export as editable PowerPoint: {}. Annotate them with \
             `<!-- sldr:zone … rep=placeholder-text … -->`, or export with \
             `--flatten` (screenshot mode).",
            not_eligible.join(", ")
        );
    }

    let layouts = crate::to_template_layouts(&distinct);
    // name → 1-based slideLayout index.
    let layout_index: HashMap<&str, usize> = layouts
        .iter()
        .enumerate()
        .map(|(i, l)| (l.name, i + 1))
        .collect();

    let mut parts: Vec<(String, String)> = Vec::new();
    let n_layouts = layouts.len();
    let n_slides = slides.len();

    parts.push((
        "[Content_Types].xml".into(),
        crate::content_types(n_layouts, n_slides),
    ));
    parts.push(("_rels/.rels".into(), crate::root_rels()));
    parts.push(("docProps/core.xml".into(), crate::core_props(title)));
    parts.push(("docProps/app.xml".into(), crate::app_props()));
    parts.push((
        "ppt/_rels/presentation.xml.rels".into(),
        crate::presentation_rels(n_slides),
    ));
    parts.push(("ppt/presentation.xml".into(), crate::presentation_xml(n_slides)));
    parts.push(("ppt/presProps.xml".into(), crate::pres_props()));
    parts.push(("ppt/theme/theme1.xml".into(), crate::theme_xml(theme)));
    parts.push((
        "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
        crate::slide_master_rels(n_layouts, &theme.brand),
    ));
    parts.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        crate::slide_master_xml(n_layouts, &theme.brand),
    ));

    for (i, layout) in layouts.iter().enumerate() {
        let n1 = i + 1;
        parts.push((
            format!("ppt/slideLayouts/slideLayout{n1}.xml"),
            crate::slide_layout_xml(layout, &theme.brand),
        ));
        parts.push((
            format!("ppt/slideLayouts/_rels/slideLayout{n1}.xml.rels"),
            crate::slide_layout_rels(layout, &theme.brand),
        ));
    }

    // Binary media: brand artwork first (fixed names), then the pictures
    // collected across all slides → ppt/media/.
    let brand_media = crate::brand_media(&theme.brand, &layouts);
    let mut media: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let n1 = i + 1;
        let li = layout_index[slide.layout.name.as_str()];
        let (xml, rels) = build_slide(slide, &Target::native(li), &mut media);
        parts.push((format!("ppt/slides/slide{n1}.xml"), xml));
        parts.push((format!("ppt/slides/_rels/slide{n1}.xml.rels"), rels));
    }

    crate::identity::attach(&mut parts, slides)?;
    crate::notes::attach(&mut parts, slides)?;
    media.extend(brand_media);
    crate::zip_mixed(&parts, &media)
}

/// Where a slide lands: its slideLayout, the slide size, how its text zones
/// become shapes, and the media naming. Native export uses sldr's own
/// generated layouts ([`Target::native`]); template-backed export
/// (`crate::master`) points at a real master's layouts and placeholders.
pub(crate) struct Target {
    /// Relationship target of the slideLayout, relative to `ppt/slides/`.
    pub layout_target: String,
    /// Slide size in EMU; zone percentages scale to it.
    pub size: (i64, i64),
    /// Prefix for media part names, so new media never collides with a
    /// master's own `ppt/media/imageN`.
    pub media_prefix: &'static str,
    /// Per text zone: the placeholder (or free text box) it becomes. Empty =
    /// the zone's own `ph`/`idx` (native export).
    pub zones: HashMap<String, ZoneTarget>,
    /// `(type, idx, number)` of a slide-number placeholder to fill.
    pub slide_number: Option<(String, u32, usize)>,
}

/// How one text zone is represented on a master-backed slide.
pub(crate) enum ZoneTarget {
    /// The master layout's placeholder: geometry and text style inherited.
    Placeholder { typ: Option<String>, idx: Option<u32> },
    /// No counterpart on the master: a text box at this box (% of slide).
    TextBox { x: f64, y: f64, w: f64, h: f64 },
}

impl Target {
    pub(crate) fn native(layout_index: usize) -> Self {
        Target {
            layout_target: format!("../slideLayouts/slideLayout{layout_index}.xml"),
            size: (crate::SLIDE_W_EMU, crate::SLIDE_H_EMU),
            media_prefix: "",
            zones: HashMap::new(),
            slide_number: None,
        }
    }
}

/// Build one slide's XML + rels. Iterates the layout's zones: `placeholder-text`
/// zones become filled `<p:sp>` placeholders (geometry inherited from the
/// layout); `picture` zones whose content is a [`ZoneContent::Picture`] become
/// positioned, embedded `<p:pic>` (geometry from the zone). `media` accumulates
/// the image parts across the whole deck; the returned rels reference them.
pub(crate) fn build_slide(
    slide: &SlideInput,
    target: &Target,
    media: &mut Vec<(String, Vec<u8>)>,
) -> (String, String) {
    let ex = |pct: f64| (target.size.0 as f64 * pct / 100.0).round() as i64;
    let ey = |pct: f64| (target.size.1 as f64 * pct / 100.0).round() as i64;
    let mp = target.media_prefix;
    let lookup: HashMap<&str, &ZoneContent> = slide
        .fields
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let mut shapes = String::new();
    let mut image_rels = String::new();
    let mut links = std::collections::BTreeMap::new();
    let mut next_id = 2; // id 1 is the group shape
    let mut next_rel = 2; // rId1 is the slideLayout
    let lang = slide.details.language.as_deref().unwrap_or("en");

    for zone in &slide.layout.zones {
        let content = lookup.get(zone.name.as_str()).copied();

        // A video: a <p:pic> whose blip is the poster frame, with the movie
        // linked (a:videoFile) and embedded (p14:media) — the form PowerPoint
        // 2010+ writes itself, playable in PowerPoint and Quick Look.
        if let Some(ZoneContent::Video { bytes, ext, poster }) = content {
            let (poster_bytes, poster_ext) = match poster {
                Some((b, e)) => (b.clone(), e.clone()),
                None => (BLANK_PNG.to_vec(), "png".to_string()),
            };
            let poster_n = media.len() + 1;
            media.push((format!("ppt/media/{mp}image{poster_n}.{poster_ext}"), poster_bytes.clone()));
            let movie_n = media.len() + 1;
            media.push((format!("ppt/media/{mp}media{movie_n}.{ext}"), bytes.clone()));
            let (rel_img, rel_video, rel_media) = (next_rel, next_rel + 1, next_rel + 2);
            next_rel += 3;
            image_rels.push_str(&format!(
                "<Relationship Id=\"rId{rel_img}\" Type=\"{R}/image\" Target=\"../media/{mp}image{poster_n}.{poster_ext}\"/>\
<Relationship Id=\"rId{rel_video}\" Type=\"{R}/video\" Target=\"../media/{mp}media{movie_n}.{ext}\"/>\
<Relationship Id=\"rId{rel_media}\" Type=\"http://schemas.microsoft.com/office/2007/relationships/media\" Target=\"../media/{mp}media{movie_n}.{ext}\"/>",
                R = crate::package::R,
            ));
            let (zx, zy, zw, zh) = (ex(zone.x), ey(zone.y), ex(zone.w), ey(zone.h));
            let (x, y, cx, cy) = match crate::imagesize::dimensions(&poster_bytes) {
                Some((iw, ih)) if iw > 1 && ih > 1 => {
                    let scale = (zw as f64 / iw as f64).min(zh as f64 / ih as f64);
                    let cx = (iw as f64 * scale).round() as i64;
                    let cy = (ih as f64 * scale).round() as i64;
                    (zx + (zw - cx) / 2, zy + (zh - cy) / 2, cx, cy)
                }
                _ => (zx, zy, zw, zh),
            };
            let id = next_id;
            next_id += 1;
            let label = crate::xml_escape(&crate::title_case(&zone.name));
            shapes.push_str(&format!(
                r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{label}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr><a:videoFile r:link="rId{rel_video}"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}"><p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="rId{rel_media}"/></p:ext></p:extLst></p:nvPr></p:nvPicPr>
<p:blipFill><a:blip r:embed="rId{rel_img}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
            ));
            continue;
        }

        // A picture (an `image` zone, or a baked diagram landing in a text
        // zone) wins regardless of the zone's declared rep — it becomes a
        // positioned <p:pic> at the zone's box.
        if let Some(ZoneContent::Picture { bytes, ext, fit }) = content {
            let media_n = media.len() + 1;
            let media_path = format!("ppt/media/{mp}image{media_n}.{ext}");
            media.push((media_path, bytes.clone()));

            let rel = next_rel;
            next_rel += 1;
            image_rels.push_str(&format!(
                "<Relationship Id=\"rId{rel}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/{mp}image{media_n}.{ext}\"/>"
            ));

            // Zone box in EMU. A picture keeps its aspect ratio, always: it
            // is fitted inside the box (centered) at the size the caller
            // gives or, failing that, the one its header declares. Only an
            // unreadable header fills the box.
            let (zx, zy, zw, zh) = (
                ex(zone.x),
                ey(zone.y),
                ex(zone.w),
                ey(zone.h),
            );
            let (x, y, cx, cy) = match fit.or_else(|| crate::imagesize::dimensions(bytes)).as_ref() {
                Some((iw, ih)) if *iw > 0 && *ih > 0 => {
                    let scale = (zw as f64 / *iw as f64).min(zh as f64 / *ih as f64);
                    let cx = (*iw as f64 * scale).round() as i64;
                    let cy = (*ih as f64 * scale).round() as i64;
                    (zx + (zw - cx) / 2, zy + (zh - cy) / 2, cx, cy)
                }
                _ => (zx, zy, zw, zh),
            };

            let id = next_id;
            next_id += 1;
            let label = crate::xml_escape(&crate::title_case(&zone.name));
            shapes.push_str(&format!(
                r#"<p:pic><p:nvPicPr><p:cNvPr id="{id}" name="{label}"/><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>
<p:blipFill><a:blip r:embed="rId{rel}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#
            ));
            continue;
        }

        // A text zone without a placeholder type (a freeform block) is a free
        // text box at its own geometry, styled like body text.
        if zone.rep == crate::ZoneRep::PlaceholderText && zone.ph.is_none() {
            let Some(content) = content else { continue };
            let id = next_id;
            next_id += 1;
            let label = crate::xml_escape(&crate::title_case(&zone.name));
            let paragraphs = match content {
                ZoneContent::Text(t) => mdooxml::plain_paragraph(t, lang),
                ZoneContent::Markdown(m) => {
                    let (paras, found) = mdooxml::to_paragraphs_with_links(m, lang);
                    links.extend(found);
                    paras.join("")
                }
                ZoneContent::Link { text, url } => {
                    links.insert(mdooxml::link_id(url), url.clone());
                    mdooxml::linked_paragraph(text, url, lang)
                }
                ZoneContent::Picture { .. } | ZoneContent::Video { .. } => unreachable!("pictures are handled above"),
            };
            shapes.push_str(&format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
<p:txBody><a:bodyPr wrap="square"><a:normAutofit/></a:bodyPr>{lst}{paragraphs}</p:txBody></p:sp>"#,
                x = ex(zone.x), y = ey(zone.y), cx = ex(zone.w), cy = ey(zone.h),
                lst = crate::TEXT_BOX_LST,
            ));
            continue;
        }

        // Otherwise: a placeholder-text zone becomes a filled text placeholder.
        if zone.rep == crate::ZoneRep::PlaceholderText && zone.ph.is_some() {
            let label = crate::xml_escape(&crate::title_case(&zone.name));
            let paragraphs = match content {
                Some(ZoneContent::Text(t)) => mdooxml::plain_paragraph(t, lang),
                Some(ZoneContent::Markdown(m)) => {
                    let (paras, found) = mdooxml::to_paragraphs_with_links(m, lang);
                    links.extend(found);
                    paras.join("")
                }
                Some(ZoneContent::Link { text, url }) => {
                    links.insert(mdooxml::link_id(url), url.clone());
                    mdooxml::linked_paragraph(text, url, lang)
                },
                _ => mdooxml::plain_paragraph("", lang),
            };
            // The placeholder it becomes: the zone's own (native export) or the
            // mapped master placeholder; a zone with no master counterpart is a
            // text box at its box, emitted only when it has content.
            let (typ, idx) = match target.zones.get(&zone.name) {
                Some(ZoneTarget::TextBox { x, y, w, h }) => {
                    if content.is_none() { continue; }
                    let id = next_id;
                    next_id += 1;
                    let lst = match zone.name.as_str() {
                        "footer" => crate::chrome_lst("1200"),
                        "source" => crate::chrome_lst("1050"),
                        _ => crate::TEXT_BOX_LST.to_string(),
                    };
                    shapes.push_str(&format!(
                        r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
<p:spPr><a:xfrm><a:off x="{x}" y="{y}"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
<p:txBody><a:bodyPr wrap="square" anchor="t"/>{lst}{paragraphs}</p:txBody></p:sp>"#,
                        x = ex(*x), y = ey(*y), cx = ex(*w), cy = ey(*h),
                    ));
                    continue;
                }
                Some(ZoneTarget::Placeholder { typ, idx }) => (typ.clone(), *idx),
                None => (Some(zone.ph.clone().unwrap_or_else(|| "body".into())), zone.idx),
            };
            let id = next_id;
            next_id += 1;
            let type_attr = typ.map(|t| format!(" type=\"{t}\"")).unwrap_or_default();
            let idx_attr = idx.map(|i| format!(" idx=\"{i}\"")).unwrap_or_default();
            shapes.push_str(&format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph{type_attr}{idx_attr}/></p:nvPr></p:nvSpPr>
<p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
            ));
        }
        // (picture zone with no picture content → nothing emitted)
    }

    // A master's slide-number placeholder, holding the slide's own number so a
    // PowerPoint save (which refreshes the field) is not mistaken for an edit.
    if let Some((typ, idx, number)) = &target.slide_number {
        let id = next_id;
        shapes.push_str(&format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{name}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{typ}" idx="{idx}"/></p:nvPr></p:nvSpPr>
<p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:fld id="{{B6F15528-21DE-4FAA-801E-634DDDAF4B2B}}" type="slidenum"><a:rPr lang="{lang}"/><a:t>{number}</a:t></a:fld></a:p></p:txBody></p:sp>"#,
            name = crate::title_case(crate::SLIDE_NUMBER_ZONE),
        ));
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
{shapes}
</p:spTree></p:cSld></p:sld>"#
    );

    let layout_target = &target.layout_target;
    for (id, url) in links {
        image_rels.push_str(&format!("<Relationship Id=\"{id}\" Type=\"{}/hyperlink\" Target=\"{}\" TargetMode=\"External\"/>", crate::package::R, crate::xml_escape(&url)));
    }
    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="{layout_target}"/>
{image_rels}
</Relationships>"#
    );

    (xml, rels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use sldr_renderer::LayoutRegistry;

    fn theme() -> Theme {
        Theme::from_parts(
            "demo", Some("#0F172A"), Some("#FFF"), Some("#3B82F6"), Some("#F59E0B"),
            Some("#E2E8F0"), Some("#94A3B8"), Some("Inter"), Some("Inter"),
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
    fn test_build_deck_framed_slide() {
        let reg = LayoutRegistry::builtin();
        let framed = reg.get("framed").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: framed,
            fields: vec![
                ("headline".into(), ZoneContent::Text("My Title".into())),
                ("content".into(), ZoneContent::Markdown("- one\n- two".into())),
                ("footer".into(), ZoneContent::Text("ACME".into())),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"ppt/slides/slide1.xml".to_string()));
        assert!(names.contains(&"ppt/slides/_rels/slide1.xml.rels".to_string()));

        let slide = read_part(&bytes, "ppt/slides/slide1.xml");
        assert!(slide.contains("<a:t>My Title</a:t>"));
        assert!(slide.contains("<a:t>one</a:t>"));
        assert!(slide.contains("buChar char=\"&#167;\"")); // square bullets
        assert!(slide.contains("<a:t>ACME</a:t>"));
        assert!(slide.contains(r#"type="title""#));
        assert!(slide.contains(r#"type="body" idx="1""#));

        // presentation wires one slide.
        let pres = read_part(&bytes, "ppt/presentation.xml");
        assert!(pres.contains("<p:sldIdLst>"));
        assert!(pres.contains("r:id=\"rId4\""));
    }

    #[test]
    fn test_two_slides_share_one_layout() {
        let reg = LayoutRegistry::builtin();
        let framed = reg.get("framed").unwrap();
        let slides = vec![
            SlideInput { details: Default::default(),
                layout: framed,
                fields: vec![("headline".into(), ZoneContent::Text("A".into()))],
            },
            SlideInput { details: Default::default(),
                layout: framed,
                fields: vec![("headline".into(), ZoneContent::Text("B".into()))],
            },
        ];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        // two slides, but only one slideLayout (shared).
        assert!(names.contains(&"ppt/slides/slide2.xml".to_string()));
        assert!(names.contains(&"ppt/slideLayouts/slideLayout1.xml".to_string()));
        assert!(!names.contains(&"ppt/slideLayouts/slideLayout2.xml".to_string()));
    }

    #[test]
    fn test_picture_zone_embeds_media_and_pic() {
        let reg = LayoutRegistry::builtin();
        let image_left = reg.get("image-left").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: image_left,
            fields: vec![
                ("content".into(), ZoneContent::Markdown("- a point".into())),
                (
                    "image".into(),
                    ZoneContent::Picture {
                        bytes: b"\x89PNG\r\n\x1a\n fake".to_vec(),
                        ext: "png".into(),
                        fit: None,
                    },
                ),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains(&"ppt/media/image1.png".to_string()));

        let slide = read_part(&bytes, "ppt/slides/slide1.xml");
        assert!(slide.contains("<p:pic>"));
        assert!(slide.contains("r:embed=\"rId2\""));
        assert!(slide.contains("<a:t>a point</a:t>")); // text placeholder too

        let rels = read_part(&bytes, "ppt/slides/_rels/slide1.xml.rels");
        assert!(rels.contains("../media/image1.png"));
        assert!(rels.contains("relationships/image"));
    }

    #[test]
    fn test_baked_picture_in_text_zone_emits_aspect_fit_pic() {
        // A diagram baked into the `content` (placeholder-text) zone of framed
        // should emit a positioned, aspect-fit <p:pic> — not a text placeholder.
        let reg = LayoutRegistry::builtin();
        let framed = reg.get("framed").unwrap();
        let slides = vec![SlideInput { details: Default::default(),
            layout: framed,
            fields: vec![
                ("headline".into(), ZoneContent::Text("Title".into())),
                (
                    "content".into(),
                    ZoneContent::Picture {
                        bytes: b"\x89PNG fake".to_vec(),
                        ext: "png".into(),
                        fit: Some((400, 100)), // wide → fit by width, centered vertically
                    },
                ),
            ],
        }];
        let bytes = build_deck(&theme(), "Deck", &slides).unwrap();
        let slide = read_part(&bytes, "ppt/slides/slide1.xml");
        assert!(slide.contains("<p:pic>"));
        assert!(slide.contains("<a:t>Title</a:t>")); // chrome still a placeholder
        // The content zone has no body idx=1 text placeholder (it's the pic now).
        assert!(!slide.contains(r#"type="body" idx="1""#));
        // Media embedded.
        let names: Vec<String> = {
            let mut z = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
            (0..z.len()).map(|i| z.by_index(i).unwrap().name().to_string()).collect()
        };
        assert!(names.contains(&"ppt/media/image1.png".to_string()));
    }

    #[test]
    fn test_picture_keeps_its_aspect_ratio_without_a_given_size() {
        // A wide clip (2120×620) in framed-image's tall image zone: its size
        // comes from the PNG header, and the placed picture has the image's
        // aspect ratio, never the zone's.
        let reg = LayoutRegistry::builtin();
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend_from_slice(&2120u32.to_be_bytes());
        png.extend_from_slice(&620u32.to_be_bytes());
        let slides = vec![SlideInput { details: Default::default(),
            layout: reg.get("framed-image").unwrap(),
            fields: vec![("image".into(), ZoneContent::Picture { bytes: png, ext: "png".into(), fit: None })],
        }];
        let slide = read_part(&build_deck(&theme(), "Deck", &slides).unwrap(), "ppt/slides/slide1.xml");
        let pic = &slide[slide.find("<p:pic>").unwrap()..];
        let attr = |name: &str| -> f64 {
            let at = pic.find(&format!("{name}=\"")).unwrap() + name.len() + 2;
            pic[at..at + pic[at..].find('"').unwrap()].parse().unwrap()
        };
        let ratio = attr("cx") / attr("cy");
        assert!((ratio - 2120.0 / 620.0).abs() < 0.01, "placed at {ratio}, image is {}", 2120.0 / 620.0);
    }

    #[test]
    fn test_ph_less_text_zone_is_a_text_box_at_its_geometry() {
        let reg = LayoutRegistry::builtin();
        let mut def = reg.get("freeform").unwrap().clone();
        def.zones.push(sldr_renderer::Zone { name: "block1".into(), ph: None, idx: None,
            rep: sldr_renderer::ZoneRep::PlaceholderText, x: 10.0, y: 20.0, w: 30.0, h: 15.0 });
        let slides = vec![SlideInput { details: Default::default(), layout: &def,
            fields: vec![("block1".into(), ZoneContent::Markdown("- free *text*".into()))] }];
        let built = crate::build_deck_with_report(&theme(), "Deck", &slides).unwrap();
        let slide = read_part(&built.value, "ppt/slides/slide1.xml");
        assert!(slide.contains("txBox=\"1\""), "{slide}");
        assert!(slide.contains(&format!("<a:off x=\"{}\" y=\"{}\"/>", crate::emu_x(10.0), crate::emu_y(20.0))));
        assert!(slide.contains("<a:t>free </a:t>") && slide.contains("i=\"1\""));
        assert!(!slide.contains("<p:ph"), "a text box is not a placeholder");
    }

    #[test]
    fn test_layout_without_zones_fails_loud() {
        let reg = LayoutRegistry::builtin();
        let collage = reg.get("image-grid").unwrap(); // multi-image, no zones yet
        let slides = vec![SlideInput { details: Default::default(),
            layout: collage,
            fields: vec![("content".into(), ZoneContent::Markdown("hi".into()))],
        }];
        let err = build_deck(&theme(), "Deck", &slides).unwrap_err().to_string();
        assert!(err.contains("image-grid"));
        assert!(err.contains("--flatten"));
    }
}
