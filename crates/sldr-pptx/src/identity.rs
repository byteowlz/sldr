//! Version 1 adapter-owned provenance. Display names and Application are not IDs.
use std::collections::{BTreeMap, BTreeSet};
use anyhow::{bail, Context, Result};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::{package::{Package, P}, Disposition as D, Report, SlideInput};

pub(crate) const NS: &str = "https://sldr.dev/pptx/identity/v1";
/// Where the manifest lives: a standard Office custom-XML data-store item
/// related to the presentation part. PowerPoint preserves these on save; it
/// silently discarded the earlier root-related `customXml/sldr.xml`.
const ITEM: &str = "customXml/item1.xml";
const ITEM_PROPS: &str = "customXml/itemProps1.xml";
/// Pre-2026-09-28 packages (not preserved by PowerPoint, still readable).
const LEGACY_PART: &str = "customXml/sldr.xml";
const EXT: &str = "{3A4FAD9F-7390-4FD1-A48A-B34F786773ED}";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest { pub version: u32, pub slides: Vec<SlideRecord> }
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SlideRecord {
    pub id: String,
    pub source_id: String,
    pub step: usize,
    pub layout: String,
    /// Language the slide was exported in (absent in older packages).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub elements: BTreeMap<String, ElementRecord>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ElementRecord {
    pub zone: String,
    pub owner: String,
    pub structure_hash: String,
    pub content_hash: String,
}

pub(crate) fn hash(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

/// Change detector for a shape's text: the hash of its runs' text in order.
/// Equal at import ⇒ the zone was not edited since export.
pub(crate) fn text_hash(shape: Node<'_, '_>) -> String {
    hash(shape.descendants().filter(|n| n.has_tag_name((crate::package::A, "t"))).filter_map(|n| n.text()).collect::<String>().as_bytes())
}

pub(crate) fn attach(parts: &mut Vec<(String, String)>, slides: &[SlideInput]) -> Result<()> {
    let mut manifest = Manifest { version: 1, slides: Vec::new() };
    let mut occurrences = BTreeMap::<String, usize>::new();
    for (i, input) in slides.iter().enumerate() {
        let path = format!("ppt/slides/slide{}.xml", i + 1);
        let xml = &mut parts.iter_mut().find(|(p, _)| p == &path).context("missing emitted slide")?.1;
        let source = input.details.source_id.clone().unwrap_or_else(|| format!("generated-slide-{}", i + 1));
        let occurrence = occurrences.entry(format!("{source}:{}", input.details.step)).or_default();
        let id = hash(format!("sldr-v1:{source}:{}:{occurrence}", input.details.step).as_bytes());
        *occurrence += 1;
        let mut record = SlideRecord { id: id.clone(), source_id: source, step: input.details.step,
            layout: input.layout.name.clone(), language: input.details.language.clone(), elements: BTreeMap::new() };
        let doc = Document::parse(xml)?;
        let mut replacements = Vec::new();
        for shape in doc.descendants().filter(|n| n.has_tag_name((P, "sp")) || n.has_tag_name((P, "pic"))) {
            let props = shape.descendants().find(|n| n.has_tag_name((P, "cNvPr"))).context("shape without cNvPr")?;
            let zone = props.attribute("name").context("shape without zone")?.to_lowercase();
            let element_id = hash(format!("{id}:{zone}").as_bytes());
            let owner = if input.details.flavor_owned.iter().any(|z| z.eq_ignore_ascii_case(&zone)) { "flavor" } else { "slide" };
            record.elements.insert(element_id.clone(), ElementRecord {
                zone, owner: owner.into(), structure_hash: structure_hash(shape),
                content_hash: text_hash(shape),
            });
            let range = props.range();
            let original = &xml[range.clone()];
            let replacement = format!("{}>{}</p:cNvPr>", original.trim_end_matches("/>"), extension(&element_id, "a"));
            replacements.push((range, replacement));
        }
        for (range, replacement) in replacements.into_iter().rev() { xml.replace_range(range, &replacement); }
        *xml = xml.replace("</p:sld>", &format!("{}</p:sld>", extension(&id, "p")));
        manifest.slides.push(record);
    }
    let json = serde_json::to_string(&manifest)?;
    let item = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><sldr:manifest xmlns:sldr=\"{NS}\" version=\"1\">{}</sldr:manifest>", crate::xml_escape(&json));
    // Deterministic data-store GUID from the manifest content.
    let h = hash(item.as_bytes());
    let guid = format!("{{{}-{}-{}-{}-{}}}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32]).to_uppercase();
    parts.push((ITEM.into(), item));
    parts.push((ITEM_PROPS.into(), format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?><ds:datastoreItem ds:itemID=\"{guid}\" xmlns:ds=\"http://schemas.openxmlformats.org/officeDocument/2006/customXml\"><ds:schemaRefs><ds:schemaRef ds:uri=\"{NS}\"/></ds:schemaRefs></ds:datastoreItem>"
    )));
    parts.push(("customXml/_rels/item1.xml.rels".into(), format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"{}/customXmlProps\" Target=\"itemProps1.xml\"/></Relationships>",
        crate::package::R
    )));
    let rels = &mut parts.iter_mut().find(|(p, _)| p == "ppt/_rels/presentation.xml.rels").context("missing presentation relationships")?.1;
    *rels = rels.replace("</Relationships>", &format!("<Relationship Id=\"rIdSldrManifest\" Type=\"{}/customXml\" Target=\"../{ITEM}\"/></Relationships>", crate::package::R));
    let ct = &mut parts.iter_mut().find(|(p, _)| p == "[Content_Types].xml").context("missing content types")?.1;
    *ct = ct.replace("</Types>", &format!("<Override PartName=\"/{ITEM_PROPS}\" ContentType=\"application/vnd.openxmlformats-officedocument.customXmlProperties+xml\"/></Types>"));
    Ok(())
}
fn extension(id: &str, prefix: &str) -> String {
    format!("<{prefix}:extLst><{prefix}:ext uri=\"{EXT}\"><sldr:identity xmlns:sldr=\"{NS}\" id=\"{id}\"/></{prefix}:ext></{prefix}:extLst>")
}
pub(crate) fn node_id<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.descendants().find(|n| n.has_tag_name((NS, "identity"))).and_then(|n| n.attribute("id"))
}
pub(crate) fn is_identity_node(node: Node<'_, '_>) -> bool {
    node.has_tag_name((NS, "identity")) ||
        (matches!(node.tag_name().name(), "ext" | "extLst") && node.descendants().any(|n| n.has_tag_name((NS, "identity"))))
}

/// Baseline excludes mutable text and display labels but includes the shape's
/// structural/geometry context. Attributes sorted so editor serialization order
/// is irrelevant. Source hashes are change detectors, NOT authentication.
pub(crate) fn structure_hash(shape: Node<'_, '_>) -> String {
    fn visit(node: Node<'_, '_>, out: &mut String) {
        if !node.is_element() || node.has_tag_name((P, "txBody")) || is_identity_node(node) { return; }
        out.push_str(node.tag_name().namespace().unwrap_or("")); out.push(':'); out.push_str(node.tag_name().name());
        let mut attrs: Vec<_> = node.attributes().filter(|a| !(node.has_tag_name((P, "cNvPr")) && matches!(a.name(), "name" | "id"))).collect();
        attrs.sort_by_key(|a| (a.namespace(), a.name()));
        for attr in attrs { out.push('|'); out.push_str(attr.name()); out.push('='); out.push_str(attr.value()); }
        out.push('{'); for child in node.children() { visit(child, out); } out.push('}');
    }
    let mut value = String::new(); visit(shape, &mut value); hash(value.as_bytes())
}

/// The part holding the manifest: whichever custom-XML item has a
/// `sldr:manifest` root (editors may renumber items), else the legacy part.
fn manifest_part(package: &Package) -> Option<String> {
    let mut candidates: Vec<&String> = package.parts.keys()
        .filter(|k| k.starts_with("customXml/") && k.ends_with(".xml") && !k.contains("/_rels/") && !k.contains("itemProps"))
        .collect();
    candidates.sort();
    candidates.into_iter()
        .find(|k| package.xml(k).is_ok_and(|d| d.root_element().has_tag_name((NS, "manifest"))))
        .cloned()
        .or_else(|| package.parts.contains_key(LEGACY_PART).then(|| LEGACY_PART.to_string()))
}

pub(crate) fn load(package: &Package) -> Result<Option<Manifest>> {
    let Some(part) = manifest_part(package) else { return Ok(None) };
    let doc = package.xml(&part)?;
    if !doc.root_element().has_tag_name((NS, "manifest")) { bail!("forged identity manifest root"); }
    let manifest: Manifest = serde_json::from_str(doc.root_element().text().context("empty identity manifest")?)?;
    if manifest.version != 1 { bail!("unsupported sldr identity version {}", manifest.version); }
    let mut ids = BTreeSet::new();
    for slide in &manifest.slides {
        if !ids.insert(&slide.id) { bail!("duplicate provenance slide identity"); }
        for (id, element) in &slide.elements {
            if !ids.insert(id) || !matches!(element.owner.as_str(), "slide" | "flavor") { bail!("invalid element identity/owner"); }
        }
    }
    Ok(Some(manifest))
}

pub(crate) fn record_for<'a>(manifest: &'a Manifest, slide: Node<'_, '_>, path: &str,
    seen: &mut BTreeSet<String>, report: &mut Report) -> Option<&'a SlideRecord> {
    let ext = slide.children().find(|n| n.has_tag_name((P, "extLst")));
    let id = ext.and_then(node_id);
    let record = id.and_then(|id| manifest.slides.iter().find(|s| s.id == id));
    if let Some(record) = record {
        if !seen.insert(record.id.clone()) {
            report.record(Some(&record.id), path, "identity", "duplicated_slide_identity", D::Conflicting,
                "Duplicated slides require an explicit new source identity; no file is overwritten");
        }
    } else {
        report.record(None, path, "identity", "missing_or_forged_identity", D::Conflicting,
            "Restore valid package metadata or explicitly resolve source ownership");
    }
    record
}
