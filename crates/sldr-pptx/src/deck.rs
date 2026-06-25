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
        // Exportable if it declares any representable zone — an editable text
        // placeholder OR a picture (a picture-only image layout is fine).
        let eligible = slide.layout.zones.iter().any(|z| {
            (z.rep == ZoneRep::PlaceholderText && z.ph.is_some()) || z.rep == ZoneRep::Picture
        });
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

    // Binary media (pictures), collected across all slides → ppt/media/.
    let mut media: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let n1 = i + 1;
        let li = layout_index[slide.layout.name.as_str()];
        let (xml, rels) = build_slide(slide, li, &mut media);
        parts.push((format!("ppt/slides/slide{n1}.xml"), xml));
        parts.push((format!("ppt/slides/_rels/slide{n1}.xml.rels"), rels));
    }

    crate::zip_mixed(&parts, &media)
}

/// Build one slide's XML + rels. Iterates the layout's zones: `placeholder-text`
/// zones become filled `<p:sp>` placeholders (geometry inherited from the
/// layout); `picture` zones whose content is a [`ZoneContent::Picture`] become
/// positioned, embedded `<p:pic>` (geometry from the zone). `media` accumulates
/// the image parts across the whole deck; the returned rels reference them.
fn build_slide(
    slide: &SlideInput,
    layout_index: usize,
    media: &mut Vec<(String, Vec<u8>)>,
) -> (String, String) {
    let lookup: HashMap<&str, &ZoneContent> = slide
        .fields
        .iter()
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let mut shapes = String::new();
    let mut image_rels = String::new();
    let mut next_id = 2; // id 1 is the group shape
    let mut next_rel = 2; // rId1 is the slideLayout

    for zone in &slide.layout.zones {
        let content = lookup.get(zone.name.as_str()).copied();

        // A picture (an `image` zone, or a baked diagram landing in a text
        // zone) wins regardless of the zone's declared rep — it becomes a
        // positioned <p:pic> at the zone's box.
        if let Some(ZoneContent::Picture { bytes, ext, fit }) = content {
            let media_n = media.len() + 1;
            let media_path = format!("ppt/media/image{media_n}.{ext}");
            media.push((media_path, bytes.clone()));

            let rel = next_rel;
            next_rel += 1;
            image_rels.push_str(&format!(
                "<Relationship Id=\"rId{rel}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/image{media_n}.{ext}\"/>"
            ));

            // Zone box in EMU; aspect-fit (centered) when intrinsic dims are
            // given so a diagram isn't stretched, else fill the box.
            let (zx, zy, zw, zh) = (
                crate::emu_x(zone.x),
                crate::emu_y(zone.y),
                crate::emu_x(zone.w),
                crate::emu_y(zone.h),
            );
            let (x, y, cx, cy) = match fit {
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

        // Otherwise: a placeholder-text zone becomes a filled text placeholder.
        if zone.rep == crate::ZoneRep::PlaceholderText && zone.ph.is_some() {
            let id = next_id;
            next_id += 1;
            let ph = zone.ph.as_deref().unwrap_or("body");
            let idx_attr = match zone.idx {
                Some(idx) => format!(" idx=\"{idx}\""),
                None => String::new(),
            };
            let label = crate::xml_escape(&crate::title_case(&zone.name));
            let paragraphs = match content {
                Some(ZoneContent::Text(t)) => mdooxml::plain_paragraph(t),
                Some(ZoneContent::Markdown(m)) => mdooxml::to_paragraphs(m).join(""),
                _ => mdooxml::plain_paragraph(""),
            };
            shapes.push_str(&format!(
                r#"<p:sp><p:nvSpPr><p:cNvPr id="{id}" name="{label}"/><p:cNvSpPr><a:spLocks noGrp="1"/></p:cNvSpPr><p:nvPr><p:ph type="{ph}"{idx_attr}/></p:nvPr></p:nvSpPr>
<p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{paragraphs}</p:txBody></p:sp>"#
            ));
        }
        // (picture zone with no picture content → nothing emitted)
    }

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
{shapes}
</p:spTree></p:cSld></p:sld>"#
    );

    let rels = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout{layout_index}.xml"/>
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
    fn test_picture_zone_embeds_media_and_pic() {
        let reg = LayoutRegistry::builtin();
        let image_left = reg.get("image-left").unwrap();
        let slides = vec![SlideInput {
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
        let slides = vec![SlideInput {
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
    fn test_layout_without_zones_fails_loud() {
        let reg = LayoutRegistry::builtin();
        let collage = reg.get("image-grid").unwrap(); // multi-image, no zones yet
        let slides = vec![SlideInput {
            layout: collage,
            fields: vec![("content".into(), ZoneContent::Markdown("hi".into()))],
        }];
        let err = build_deck(&theme(), "Deck", &slides).unwrap_err().to_string();
        assert!(err.contains("image-grid"));
        assert!(err.contains("--flatten"));
    }
}
