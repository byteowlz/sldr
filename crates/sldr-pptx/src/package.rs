//! Bounded, inert OPC reader. Validation never fetches relationship targets.
use std::{collections::{BTreeMap, BTreeSet}, io::Read};
use anyhow::{bail, Context, Result};
use roxmltree::{Document, Node};
use crate::{Disposition, Report};

pub(crate) const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
pub(crate) const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
pub(crate) const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const MAX_PART: u64 = 32 * 1024 * 1024;
const MAX_TOTAL: u64 = 128 * 1024 * 1024;
const MAX_PARTS: usize = 4096;

pub(crate) struct Package { pub parts: BTreeMap<String, Vec<u8>> }
#[derive(Debug)]
pub(crate) struct Relationship {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub external: bool,
}

impl Package {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        if bytes.len() as u64 > MAX_TOTAL { bail!("package exceeds 128 MiB limit"); }
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
        if zip.len() > MAX_PARTS { bail!("package exceeds 4096 part limit"); }
        let mut parts = BTreeMap::new();
        let mut total = 0;
        for i in 0..zip.len() {
            let file = zip.by_index(i)?;
            let name = file.name().to_owned();
            if file.is_dir() { continue; }
            if !safe_part(&name) { bail!("unsafe package path: {name}"); }
            if parts.contains_key(&name) { bail!("duplicate package part: {name}"); }
            if file.size() > MAX_PART { bail!("part exceeds 32 MiB limit: {name}"); }
            let mut data = Vec::new();
            file.take(MAX_PART + 1).read_to_end(&mut data)?;
            total += data.len() as u64;
            if data.len() as u64 > MAX_PART || total > MAX_TOTAL { bail!("package decompression limit exceeded"); }
            parts.insert(name, data);
        }
        Ok(Self { parts })
    }

    pub fn text(&self, part: &str) -> Result<&str> {
        std::str::from_utf8(self.parts.get(part).with_context(|| format!("missing part {part}"))?)
            .with_context(|| format!("non-UTF8 XML part {part}"))
    }

    pub fn xml(&self, part: &str) -> Result<Document<'_>> {
        // roxmltree's default rejects DTDs. Cap parsed nodes in addition to bytes.
        Ok(Document::parse_with_options(self.text(part)?, roxmltree::ParsingOptions {
            allow_dtd: false, nodes_limit: 200_000,
        }).with_context(|| format!("malformed XML in {part}"))?)
    }

    pub fn rels(&self, owner: &str) -> Result<Vec<Relationship>> {
        let part = rels_path(owner);
        if !self.parts.contains_key(&part) { return Ok(Vec::new()); }
        let doc = self.xml(&part)?;
        let mut ids = BTreeSet::new();
        let mut rels = Vec::new();
        for node in doc.root_element().children().filter(Node::is_element) {
            if !node.has_tag_name((REL, "Relationship")) { bail!("unknown relationship node in {part}"); }
            let id = required(node, "Id")?;
            if !ids.insert(id.to_owned()) { bail!("duplicate relationship ID {id} in {part}"); }
            let target = required(node, "Target")?;
            let kind = required(node, "Type")?;
            let external = match node.attribute("TargetMode") {
                None | Some("Internal") => false,
                Some("External") => true,
                _ => bail!("invalid TargetMode in {part}"),
            };
            rels.push(Relationship {
                id: id.into(), kind: kind.into(), external,
                target: if external { target.into() } else { resolve(owner, target)? },
            });
        }
        Ok(rels)
    }

    pub fn slides(&self) -> Result<Vec<String>> {
        let doc = self.xml("ppt/presentation.xml")?;
        let rels = self.rels("ppt/presentation.xml")?;
        let mut seen = BTreeSet::new();
        let mut paths = Vec::new();
        for node in doc.descendants().filter(|n| n.has_tag_name((P, "sldId"))) {
            let rid = node.attribute((R, "id")).context("slide has no relationship")?;
            let rel = rels.iter().find(|r| r.id == rid && r.kind == format!("{R}/slide") && !r.external)
                .context("missing/invalid slide relationship")?;
            if !seen.insert(rel.target.clone()) { bail!("duplicate slide target {}", rel.target); }
            paths.push(rel.target.clone());
        }
        Ok(paths)
    }

    pub fn validate(&self) -> Result<Report> {
        let mut report = Report::default();
        let ct = self.xml("[Content_Types].xml")?;
        let defaults: BTreeSet<_> = ct.descendants().filter(|n| n.has_tag_name("Default"))
            .filter_map(|n| n.attribute("Extension")).collect();
        let overrides: BTreeSet<_> = ct.descendants().filter(|n| n.has_tag_name("Override"))
            .filter_map(|n| n.attribute("PartName")).collect();
        for part in self.parts.keys() {
            if part != "[Content_Types].xml" && !overrides.contains(format!("/{part}").as_str()) &&
                !defaults.contains(part.rsplit('.').next().unwrap_or("")) {
                bail!("missing content type for {part}");
            }
            if part.ends_with(".xml") || part.ends_with(".rels") {
                let doc = self.xml(part)?;
                let mut ids = BTreeSet::new();
                for node in doc.descendants().filter(|n| n.has_tag_name((P, "cNvPr"))) {
                    if !ids.insert(required(node, "id")?) { bail!("duplicate shape ID in {part}"); }
                }
                if part.ends_with(".rels") {
                    let owner = owner_path(part)?;
                    if !owner.is_empty() && !self.parts.contains_key(&owner) { bail!("orphan relationships: {part}"); }
                    for rel in self.rels(&owner)? {
                        if !rel.external && !self.parts.contains_key(&rel.target) { bail!("missing relationship target {} in {part}", rel.target); }
                        let disposition = if rel.external { Disposition::Unsupported } else { Disposition::Converted };
                        report.record(None, part, &rel.id, &format!("relationship:{}", rel.kind), disposition,
                            if rel.external { "External target not fetched; import does not preserve this relationship" } else { "Internal target validated" });
                    }
                } else {
                    let rels = self.rels(part)?;
                    for attr in doc.descendants().flat_map(|n| n.attributes()) {
                        if attr.namespace() == Some(R) && !rels.iter().any(|r| r.id == attr.value()) {
                            bail!("unresolved {}={} in {part}", attr.name(), attr.value());
                        }
                    }
                }
            }
            let lower = part.to_ascii_lowercase();
            if lower.contains("vbaproject") || lower.contains("/embeddings/") || lower.contains("activex") {
                report.record(None, part, "package", "active_or_embedded_content", Disposition::Conflicting,
                    "Quarantine this package; macros/OLE/ActiveX are never executed or imported");
            }
        }
        Ok(report)
    }
}

fn required<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str> {
    node.attribute(name).with_context(|| format!("missing {name} attribute"))
}
fn safe_part(path: &str) -> bool {
    !path.is_empty() && !path.contains(['\\', ':', '\0', '%', '?', '#']) &&
        path.split('/').all(|p| !p.is_empty() && p != "." && p != "..")
}
pub(crate) fn rels_path(owner: &str) -> String {
    match owner.rsplit_once('/') {
        Some((dir, name)) => format!("{dir}/_rels/{name}.rels"),
        None => format!("_rels/{owner}.rels"),
    }
}
fn owner_path(path: &str) -> Result<String> {
    if path == "_rels/.rels" { return Ok(String::new()); }
    let (dir, name) = path.rsplit_once("/_rels/").context("invalid relationship part path")?;
    Ok(format!("{dir}/{}", name.strip_suffix(".rels").context("invalid rel suffix")?))
}
pub(crate) fn resolve(owner: &str, target: &str) -> Result<String> {
    if target.contains(['\\', ':', '\0', '%', '?', '#']) { bail!("unsafe relationship target {target}"); }
    let mut stack: Vec<&str> = if target.starts_with('/') { Vec::new() } else {
        owner.rsplit_once('/').map(|(d, _)| d.split('/').collect()).unwrap_or_default()
    };
    for piece in target.trim_start_matches('/').split('/') {
        match piece {
            ".." => { if stack.pop().is_none() { bail!("relationship traversal beyond package root"); } }
            "." => {},
            "" => bail!("empty relationship path component"),
            _ => stack.push(piece),
        }
    }
    let result = stack.join("/");
    if !safe_part(&result) { bail!("unsafe relationship target {target}"); }
    Ok(result)
}

/// Validate the ZIP/XML/content-type/relationship closure, without authorizing
/// external-deck import. This is structural evidence, not Office conformance.
pub fn validate_package(bytes: &[u8]) -> Result<Report> {
    Package::read(bytes)?.validate()
}
