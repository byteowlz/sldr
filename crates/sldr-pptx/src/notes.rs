//! Speaker notes as native notesSlide parts (export) and extraction (import).
//! Notes are plain-markdown text; they never fetch or embed external content.
use anyhow::{bail, Context, Result};
use roxmltree::Node;
use crate::{package::{Package, A, P, R}, SlideInput, mdooxml};

const NOTES_CT: &str = "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";

/// Emit a notesSlide part for every slide carrying notes, wire the slide's
/// relationship to it, add a back-reference rel, and register the content type.
pub(crate) fn attach(parts: &mut Vec<(String, String)>, slides: &[SlideInput]) -> Result<()> {
    let mut overrides = String::new();
    let mut added = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let Some(notes) = slide.details.notes.as_ref().filter(|n| !n.trim().is_empty()) else { continue; };
        let n = i + 1;
        let notes_path = format!("ppt/notesSlides/notesSlide{n}.xml");
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:notes xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
<p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>
<p:sp><p:nvSpPr><p:cNvPr id="2" name="Slide Image Placeholder 2"/><p:cNvSpPr/><p:nvPr><p:ph type="sldImg"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/></p:txBody></p:sp>
<p:sp><p:nvSpPr><p:cNvPr id="3" name="Notes Placeholder 3"/><p:cNvSpPr/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr><p:spPr/><p:txBody><a:bodyPr/><a:lstStyle/>{}</p:txBody></p:sp>
</p:spTree></p:cSld><p:clrMapOvr><a:overrideClrMapping/></p:clrMapOvr></p:notes>"#,
            mdooxml::notes_paragraphs(notes).join("")
        );
        // Slide rels get a notesSlide relationship (rId is unique per slide).
        let slide_rels = parts.iter_mut().find(|(p, _)| p == &format!("ppt/slides/_rels/slide{n}.xml.rels"))
            .context("missing slide relationships")?;
        slide_rels.1 = slide_rels.1.replace("</Relationships>",
            &format!("<Relationship Id=\"rIdN{n}\" Type=\"{R}/notesSlide\" Target=\"../notesSlides/notesSlide{n}.xml\"/></Relationships>"));
        added.push((notes_path.clone(), xml));
        added.push((format!("ppt/notesSlides/_rels/notesSlide{n}.xml.rels"),
            format!(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="{R}/slide" Target="../slides/slide{n}.xml"/>
</Relationships>"#)));
        overrides.push_str(&format!("<Override PartName=\"/ppt/notesSlides/notesSlide{n}.xml\" ContentType=\"{NOTES_CT}\"/>"));
    }
    if !added.is_empty() {
        let ct = parts.iter_mut().find(|(p, _)| p == "[Content_Types].xml").context("missing content types")?;
        ct.1 = ct.1.replace("</Types>", &format!("{overrides}</Types>"));
        parts.extend(added);
    }
    Ok(())
}

/// Extract the notes text for a slide from its rels, or `None` when no
/// notesSlide exists. Notes with unsupported external references are surfaced
/// by the caller's report; the text itself is inert Markdown.
pub(crate) fn extract(package: &Package, slide_rels: &[crate::package::Relationship]) -> Result<Option<String>> {
    let Some(rel) = slide_rels.iter().find(|r| r.kind == format!("{R}/notesSlide") && !r.external) else {
        return Ok(None);
    };
    let doc = package.xml(&rel.target)?;
    // The body placeholder lives in a <p:sp>; prefer ph type="body" over sldImg.
    let body: Option<Node<'_, '_>> = doc.descendants()
        .filter(|n| n.has_tag_name((P, "ph")) && n.attribute("type").is_none_or(|t| t != "sldImg"))
        .find_map(|ph| ph.ancestors().find(|n| n.has_tag_name((P, "sp"))))
        .and_then(|sp| sp.descendants().find(|n| n.has_tag_name((P, "txBody"))));
    let Some(body) = body else { bail!("notes slide has no notes body"); };
    let mut out = String::new();
    for para in body.descendants().filter(|n| n.has_tag_name((A, "p"))) {
        let runs = para.children().filter(|n| n.has_tag_name((A, "r")))
            .map(|r| r.children().filter(|n| n.has_tag_name((A, "t"))).filter_map(|n| n.text()).collect::<String>())
            .collect::<Vec<_>>();
        if runs.is_empty() { continue; }
        if !out.is_empty() { out.push('\n'); }
        out.push_str(&runs.join(""));
    }
    let value = out.trim();
    Ok(if value.is_empty() { None } else { Some(value.to_owned()) })
}

