use std::{collections::BTreeMap, io::{Read, Write}};
use sldr_pptx::{build_deck, build_deck_with_report, import, import_with_report, validate_package, Disposition, SlideDetails, SlideInput, Theme, ZoneContent};
use sldr_renderer::LayoutRegistry;

fn deck() -> Vec<u8> {
    let registry = LayoutRegistry::builtin();
    build_deck(&Theme::from_flavor(&Default::default()), "Synthetic", &[SlideInput { details: Default::default(),
        layout: registry.get("framed").unwrap(),
        fields: vec![("content".into(), ZoneContent::Markdown("- Original".into()))],
    }]).unwrap()
}
fn mutate(bytes: &[u8], change: impl FnOnce(&mut BTreeMap<String, Vec<u8>>)) -> Vec<u8> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut parts = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).unwrap(); let mut data = Vec::new();
        f.read_to_end(&mut data).unwrap(); parts.insert(f.name().to_owned(), data);
    }
    change(&mut parts);
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, bytes) in parts { w.start_file(name, zip::write::SimpleFileOptions::default()).unwrap(); w.write_all(&bytes).unwrap(); }
    w.finish().unwrap().into_inner()
}
fn replace(parts: &mut BTreeMap<String, Vec<u8>>, part: &str, from: &str, to: &str) {
    let text = String::from_utf8(parts[part].clone()).unwrap();
    assert!(text.contains(from)); parts.insert(part.into(), text.replace(from, to).into_bytes());
}

#[test]
fn manual_textbox_and_freeform_are_not_silent_success() {
    let edited = mutate(&deck(), |parts| replace(parts, "ppt/slides/slide1.xml", "</p:spTree>",
        "<p:sp><p:nvSpPr><p:cNvPr id=\"99\" name=\"Manual textbox\"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Manual addition</a:t></a:r></a:p></p:txBody></p:sp><p:sp><p:nvSpPr><p:cNvPr id=\"100\" name=\"Freeform\"/></p:nvSpPr><p:spPr><a:custGeom/></p:spPr></p:sp></p:spTree>"));
    assert!(import(&edited).is_err());
    let converted = import_with_report(&edited).unwrap();
    let omitted: Vec<_> = converted.report.findings.iter().filter(|f| f.disposition == Disposition::Unsupported).collect();
    assert_eq!(omitted.len(), 2);
    assert_eq!(omitted.iter().map(|f| f.element.as_str()).collect::<Vec<_>>(), ["99", "100"]);
    converted.report.enforce(true).unwrap();
    assert!(converted.value[0].body.contains("Original"));
    let json = serde_json::to_value(&converted.report).unwrap();
    assert_eq!(json["schema_version"], 1);
    assert_eq!(serde_json::from_value::<sldr_pptx::Report>(json).unwrap(), converted.report);
}

#[test]
fn malformed_relationships_and_duplicate_shape_ids_fail() {
    for (from, to) in [("../slideLayouts/slideLayout1.xml", "../../../../escape.xml"),
        ("../slideLayouts/slideLayout1.xml", "../missing.xml"), ("Id=\"rId1\"", "Broken=\"rId1\"")] {
        let bytes = mutate(&deck(), |p| replace(p, "ppt/slides/_rels/slide1.xml.rels", from, to));
        assert!(validate_package(&bytes).is_err()); assert!(import_with_report(&bytes).is_err());
    }
    let bytes = mutate(&deck(), |p| replace(p, "ppt/slides/slide1.xml", "id=\"2\"", "id=\"1\""));
    assert!(validate_package(&bytes).is_err());
}

#[test]
fn unknown_attributes_on_known_text_report_exact_locator() {
    let bytes = mutate(&deck(), |p| replace(p, "ppt/slides/slide1.xml", "<a:rPr lang=\"en-US\"", "<a:rPr sz=\"9900\" lang=\"en-US\""));
    let report = import_with_report(&bytes).unwrap().report;
    assert!(report.findings.iter().any(|f| f.element.ends_with("/rPr@sz") && f.disposition == Disposition::Unsupported));
    assert!(report.enforce(false).is_err());
}

#[test]
fn unsupported_zones_fields_and_markdown_are_accounted_for() {
    let registry = LayoutRegistry::builtin();
    let mut layout = registry.get("framed").unwrap().clone();
    layout.zones[0].rep = sldr_renderer::ZoneRep::Shape;
    layout.zones[1].rep = sldr_renderer::ZoneRep::Bake;
    let slides = [SlideInput { details: Default::default(), layout: &layout, fields: vec![
        ("content".into(), ZoneContent::Markdown("![missing](absent.png)\n\n<div>Effect</div>".into())),
        ("unknown".into(), ZoneContent::Text("Unmapped".into())),
    ] }];
    let theme = Theme::from_flavor(&Default::default());
    assert!(build_deck(&theme, "Strict", &slides).is_err());
    let result = build_deck_with_report(&theme, "Lossy", &slides).unwrap();
    for feature in ["shape", "bake", "inline_image", "html", "unmapped_field"] {
        assert!(result.report.findings.iter().any(|f| f.feature == feature), "{feature}");
    }
}

#[test]
#[test]
fn notes_round_trip_preserves_speaker_notes_and_source() {
    let registry = LayoutRegistry::builtin();
    let theme = Theme::from_flavor(&Default::default());
    let bytes = build_deck(&theme, "Notes", &[SlideInput {
        layout: registry.get("framed").unwrap(),
        fields: vec![("content".into(), ZoneContent::Markdown("- Body".into()))],
        details: SlideDetails { source_id: Some("notes-slide".into()), notes: Some("Remember the punchline.\nSecond line.".into()), ..Default::default() },
    }]).unwrap();
    // A notesSlide part must exist, be content-typed, and carry a rel back.
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes.as_slice())).unwrap();
    let mut names = Vec::new();
    for i in 0..z.len() { names.push(z.by_index(i).unwrap().name().to_owned()); }
    assert!(names.contains(&"ppt/notesSlides/notesSlide1.xml".into()));
    assert!(names.contains(&"ppt/notesSlides/_rels/notesSlide1.xml.rels".into()));
    let ct = { let mut f = z.by_name("[Content_Types].xml").unwrap(); let mut s=String::new(); std::io::Read::read_to_string(&mut f,&mut s).unwrap(); s };
    assert!(ct.contains("notesSlide+xml"));
    let slide_rels = { let mut f = z.by_name("ppt/slides/_rels/slide1.xml.rels").unwrap(); let mut s=String::new(); std::io::Read::read_to_string(&mut f,&mut s).unwrap(); s };
    assert!(slide_rels.contains("/notesSlide"));
    drop(z);
    // Import preserves both notes and the stable source identity.
    let imported = import(&bytes).unwrap();
    assert_eq!(imported[0].notes.as_deref(), Some("Remember the punchline.\nSecond line."));
    assert_eq!(imported[0].source_id.as_deref(), Some("notes-slide"));
    assert!(imported[0].identity.is_some());
}

#[test]
fn plain_deck_is_deterministic_and_valid() {
    assert_eq!(deck(), deck());
    validate_package(&deck()).unwrap().enforce(false).unwrap();
    import(&deck()).unwrap();
}

#[test]
fn unsafe_archive_paths_and_bounded_bomb_are_rejected() {
    for name in ["../escape.xml", "/absolute.xml", "ppt/../escape.xml", "ppt\\escape.xml"] {
        let bytes = mutate(&deck(), |p| { p.insert(name.into(), b"<x/>".to_vec()); });
        assert!(validate_package(&bytes).is_err());
    }
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    zip.start_file("bomb.xml", zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).unwrap();
    zip.write_all(&vec![b' '; 32 * 1024 * 1024 + 1]).unwrap();
    let bytes = zip.finish().unwrap().into_inner();
    assert!(bytes.len() < 100_000);
    assert!(validate_package(&bytes).unwrap_err().to_string().contains("limit"));
}
