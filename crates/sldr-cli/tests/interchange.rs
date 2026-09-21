//! Public CLI transaction tests; all configuration and assets are synthetic.
use std::{io::{Read, Write}, path::Path, process::Command};

fn command(root: &Path) -> Command {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sldr"));
    c.env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_CACHE_HOME", root.join("cache"));
    c
}
fn input(root: &Path) -> std::path::PathBuf {
    let registry = sldr_renderer::LayoutRegistry::builtin();
    let bytes = sldr_pptx::build_deck(&sldr_pptx::Theme::from_flavor(&Default::default()), "Synthetic", &[
        sldr_pptx::SlideInput { details: Default::default(), layout: registry.get("framed").unwrap(), fields: vec![] }
    ]).unwrap();
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for i in 0..z.len() {
        let mut file = z.by_index(i).unwrap(); let mut bytes = Vec::new(); file.read_to_end(&mut bytes).unwrap();
        if file.name() == "ppt/slides/slide1.xml" {
            bytes = String::from_utf8(bytes).unwrap().replace("</p:spTree>",
                "<p:sp><p:nvSpPr><p:cNvPr id=\"99\" name=\"Manual\"/></p:nvSpPr><p:txBody><a:p><a:r><a:t>Unknown addition</a:t></a:r></a:p></p:txBody></p:sp></p:spTree>").into_bytes();
        }
        w.start_file(file.name(), zip::write::SimpleFileOptions::default()).unwrap(); w.write_all(&bytes).unwrap();
    }
    let path = root.join("input.pptx");
    std::fs::write(&path, w.finish().unwrap().into_inner()).unwrap(); path
}
#[test]
fn strict_import_keeps_existing_destination_and_lossy_reports_omission() {
    let root = tempfile::tempdir().unwrap(); let input = input(root.path());
    let destination = root.path().join("library"); std::fs::create_dir(&destination).unwrap();
    std::fs::write(destination.join("sentinel.md"), "DO NOT MODIFY").unwrap();
    let report = root.path().join("report.json");
    let before: Vec<_> = std::fs::read_dir(root.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    let status = command(root.path()).arg("import").arg(&input).arg("--out").arg(&destination)
        .arg("--strict").arg("--report-json").arg(&report).output().unwrap();
    assert!(!status.status.success());
    assert_eq!(std::fs::read_to_string(destination.join("sentinel.md")).unwrap(), "DO NOT MODIFY");
    assert_eq!(std::fs::read_dir(&destination).unwrap().count(), 1);
    let parsed: sldr_pptx::Report = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert!(parsed.findings.iter().any(|f| f.element == "99"));
    let output = root.path().join("lossy");
    let status = command(root.path()).arg("import").arg(input).arg("--out").arg(&output)
        .arg("--allow-lossy").arg("--report-json").arg(report).output().unwrap();
    assert!(status.status.success(), "{}", String::from_utf8_lossy(&status.stderr));
    assert!(output.join("001-slide.md").exists());
    assert!(std::fs::read_dir(root.path()).unwrap().all(|e| !e.unwrap().file_name().to_string_lossy().starts_with(".tmp")));
    assert!(!before.is_empty());
}
#[test]
fn malformed_input_has_json_report_and_no_partial_destination() {
    let root = tempfile::tempdir().unwrap(); let input = root.path().join("invalid.pptx");
    std::fs::write(&input, b"not a zip").unwrap();
    let dest = root.path().join("imported"); let report = root.path().join("report.json");
    let status = command(root.path()).arg("import").arg(input).arg("--out").arg(&dest)
        .arg("--allow-lossy").arg("--report-json").arg(&report).output().unwrap();
    assert!(!status.status.success()); assert!(!dest.exists());
    let parsed: sldr_pptx::Report = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert!(parsed.findings.iter().any(|f| f.disposition == sldr_pptx::Disposition::Conflicting));
}
