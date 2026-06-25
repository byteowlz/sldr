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
}

/// One slide to export: the layout it uses and the content for each zone.
/// The caller (CLI) decides which zone gets which content and whether it is
/// chrome text or a markdown body — keeping this crate agnostic about the
/// frontmatter↔zone naming convention.
pub struct SlideInput<'a> {
    pub layout: &'a LayoutDef,
    /// `(zone_name, content)` pairs. Zones without an entry render empty.
    pub fields: Vec<(String, ZoneContent)>,
}

/// Generate an editable deck `.pptx` from `slides`. `title` becomes the
/// document title. Every slide's layout must declare at least one
/// `placeholder-text` zone; otherwise this fails loud (use `--flatten` for the
/// screenshot path, or annotate the layout with zones).
pub fn build_deck(theme: &crate::Theme, title: &str, slides: &[SlideInput]) -> Result<Vec<u8>> {
    if slides.is_empty() {
        bail!("PPTX deck needs at least one slide");
    }

    // Distinct layouts in first-seen order; every slide layout must be
    // placeholder-eligible or we can't represent it natively.
    let mut distinct: Vec<&LayoutDef> = Vec::new();
    let mut not_eligible: Vec<&str> = Vec::new();
    for slide in slides {
        let eligible = slide
            .layout
            .zones
            .iter()
            .any(|z| z.rep == ZoneRep::PlaceholderText && z.ph.is_some());
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
        crate::slide_master_rels(n_layouts),
    ));
    parts.push((
        "ppt/slideMasters/slideMaster1.xml".into(),
        crate::slide_master_xml(n_layouts),
    ));

    for (i, layout) in layouts.iter().enumerate() {
        let n1 = i + 1;
        parts.push((
            format!("ppt/slideLayouts/slideLayout{n1}.xml"),
            crate::slide_layout_xml(layout),
        ));
        parts.push((
            format!("ppt/slideLayouts/_rels/slideLayout{n1}.xml.rels"),
            crate::slide_layout_rels(),
        ));
    }

    for (i, slide) in slides.iter().enumerate() {
        let n1 = i + 1;
        let li = layout_index[slide.layout.name.as_str()];
        let tl = &layouts[li - 1];
        parts.push((format!("ppt/slides/slide{n1}.xml"), slide_xml(tl, slide)));
        parts.push((
            format!("ppt/slides/_rels/slide{n1}.xml.rels"),
            slide_rels(li),
        ));
    }

    crate::zip_parts(&parts)
}

/// One slide: a `<p:sp>` per placeholder zone of its layout, geometry
/// inherited from the layout (empty `<p:spPr/>`), text filled from `fields`.
fn slide_xml(layout: &crate::TemplateLayout, slide: &SlideInput) -> String {
    let lookup: HashMap<&str, &ZoneContent> = slide
        .fields
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let mut sps = String::new();
    for (i, zone) in layout.placeholders.iter().enumerate() {
        let id = i + 2; // id 1 is the group shape
        let ph = zone.ph.as_deref().unwrap_or("body");
        let idx_attr = match zone.idx {
            Some(idx) => format!(" idx=\"{idx}\""),
            None => String::new(),
        };
        let label = crate::xml_escape(&crate::title_case(&zone.name));

        let paragraphs = match lookup.get(zone.name.as_str()) {
            Some(ZoneContent::Text(t)) => mdooxml::plain_paragraph(t),
            Some(ZoneContent::Markdown(m)) => mdooxml::to_paragraphs(m).join(""),
            None => mdooxml::plain_paragraph(""),
        };

        sps.push_str(&format!(
            r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{ph}"{idx_attr}/></p:nvPr></p:nvSpPr>
<p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
        ));
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
{sps}
</p:spTree></p:cSld></p:sld>"#
    )
}

fn slide_rels(layout_index: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout{layout_index}.xml"/>
</Relationships>"#
    )
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
        let slides = vec![SlideInput {
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
        assert!(slide.contains("buChar char=\"&#9642;\"")); // square bullets
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
            SlideInput {
                layout: framed,
                fields: vec![("headline".into(), ZoneContent::Text("A".into()))],
            },
            SlideInput {
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
    fn test_layout_without_zones_fails_loud() {
        let reg = LayoutRegistry::builtin();
        let default = reg.get("default").unwrap(); // no zones
        let slides = vec![SlideInput {
            layout: default,
            fields: vec![("content".into(), ZoneContent::Markdown("hi".into()))],
        }];
        let err = build_deck(&theme(), "Deck", &slides).unwrap_err().to_string();
        assert!(err.contains("default"));
        assert!(err.contains("--flatten"));
    }
}
