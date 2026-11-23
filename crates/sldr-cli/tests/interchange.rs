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

/// A stand-in for installed Chrome: every `--screenshot` yields one fixed
/// transparent 1920x1080 capture with a painted rectangle. No real browser,
/// no network.
#[cfg(unix)]
fn fake_chrome(root: &Path) -> anyhow::Result<std::path::PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let mut capture = image::RgbaImage::new(1920, 1080);
    for y in 300..500 {
        for x in 200..600 {
            capture.put_pixel(x, y, image::Rgba([23, 80, 170, 255]));
        }
    }
    capture.save(root.join("capture.png"))?;
    let script = root.join("chrome");
    std::fs::write(&script, format!(r#"#!/usr/bin/env bash
for arg in "$@"; do
  case "$arg" in --screenshot=*) cp '{dir}/capture.png' "${{arg#--screenshot=}}";; esac
done
"#, dir = root.display()))?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    Ok(script)
}

/// A synthetic library: one deck with an svg fence, an html fence, a fence
/// in a `framed-cols` column, and an ordinary code fence.
#[cfg(unix)]
fn fence_library(root: &Path) -> anyhow::Result<()> {
    let library = root.join("library");
    for dir in ["slides", "playlists", "flavors/probe", "layouts", "scaffolds"] {
        std::fs::create_dir_all(library.join(dir))?;
    }
    let at = |rel: &str| library.join(rel).display().to_string();
    std::fs::create_dir_all(root.join("config/sldr"))?;
    std::fs::write(root.join("config/sldr/config.toml"), format!(
        "[config]\nlibrary=\"{}\"\nflavor_dir=\"{}\"\nlayout_dir=\"{}\"\nscaffold_dir=\"{}\"\ndefault_flavor=\"probe\"\n\
         [presentations]\nslide_dir=\"{}\"\nplaylist_dir=\"{}\"\noutput_dir=\"{}\"\n",
        library.display(), at("flavors"), at("layouts"), at("scaffolds"), at("slides"), at("playlists"),
        root.join("out").display(),
    ))?;
    std::fs::write(library.join("flavors/probe/flavor.toml"),
        "name=\"probe\"\n[colors]\nbackground=\"#ffffff\"\ntext=\"#17232b\"\n[decoration]\nintensity=0.0\n")?;
    let slide = |name: &str, layout: &str, body: &str| std::fs::write(
        library.join(format!("slides/{name}.md")),
        format!("---\nlayout: {layout}\ntitle: Synthetic {name}\n---\n{body}\n"),
    );
    slide("svg", "framed", "```svg\n<svg viewBox=\"0 0 100 50\"><rect width=\"100\" height=\"50\" class=\"sldr-shape\"/></svg>\n```")?;
    slide("html", "framed", "```html\n<div style=\"height:40%;background:#1750aa\"></div>\n```")?;
    slide("cols", "framed-cols", "::left::\nEditable neighbor\n\n::right::\n```svg\n<svg viewBox=\"0 0 10 10\"><circle cx=\"5\" cy=\"5\" r=\"4\"/></svg>\n```")?;
    slide("code", "framed", "```python\nprint(\"still text\")\n```")?;
    std::fs::write(library.join("playlists/fences.toml"),
        "name=\"fences\"\nflavor=\"probe\"\nslides=[\"svg\",\"html\",\"cols\",\"code\"]\n[render]\ntransition=\"none\"\naspect_ratio=\"16/9\"\n")?;
    Ok(())
}

/// trx-xcn1: a ```svg / ```html fence renders as a graphic, so native export
/// places a reported region picture instead of printing the markup as text.
#[test]
#[cfg(unix)]
fn graphic_fences_export_as_baked_pictures_and_code_stays_text() -> anyhow::Result<()> {
    use anyhow::{ensure, Context};
    let root = tempfile::tempdir()?;
    fence_library(root.path())?;
    let chrome = fake_chrome(root.path())?;
    let export = |mode: &str, out: &Path, report: &Path| command(root.path()).env("CHROME_BIN", &chrome)
        .args(["export", "fences", "--format", "pptx", mode]).arg("-o").arg(out)
        .arg("--report-json").arg(report).output();

    // Baking is a reported loss: strict rejects it and publishes nothing.
    let (strict, strict_report) = (root.path().join("strict.pptx"), root.path().join("strict.json"));
    let refused = export("--strict", &strict, &strict_report)?;
    ensure!(!refused.status.success(), "strict export must reject baked regions");
    ensure!(!strict.exists(), "strict rejection must not publish a deck");

    let (deck, report) = (root.path().join("lossy.pptx"), root.path().join("lossy.json"));
    let exported = export("--allow-lossy", &deck, &report)?;
    ensure!(exported.status.success(), "{}", String::from_utf8_lossy(&exported.stderr));

    let mut package = zip::ZipArchive::new(std::fs::File::open(&deck)?)?;
    let mut slide = |n: usize| -> anyhow::Result<String> {
        let mut xml = String::new();
        package.by_name(&format!("ppt/slides/slide{n}.xml"))?.read_to_string(&mut xml)?;
        Ok(xml)
    };
    let picture = |xml: &str, zone: &str| xml.split("<p:pic>").skip(1)
        .any(|pic| pic.contains(&format!("name=\"{zone}\"")) && pic.contains("<a:blip r:embed="));
    for (n, zone) in [(1, "Content"), (2, "Content"), (3, "Right")] {
        let xml = slide(n)?;
        ensure!(picture(&xml, zone), "slide {n}: no picture in the {zone} zone");
        ensure!(!xml.contains("<a:t>&lt;"), "slide {n}: fence markup exported as text");
    }
    ensure!(slide(3)?.contains("<a:t>Editable neighbor</a:t>"), "the neighboring column must stay text");
    let code = slide(4)?;
    ensure!(code.contains("<a:t>print(\"still text\")</a:t>"), "a code fence must stay text");
    ensure!(!code.contains("<p:pic>"), "a code fence must not become a picture");

    let parsed: sldr_pptx::Report = serde_json::from_slice(&std::fs::read(&report)?)?;
    let found = |n: usize, element: &str, feature: &str| parsed.findings.iter()
        .find(|f| f.part == format!("ppt/slides/slide{n}.xml") && f.element.starts_with(element) && f.feature == feature)
        .map(|f| f.disposition);
    for (n, zone) in [(1, "content"), (2, "content"), (3, "right")] {
        ensure!(found(n, zone, "region_raster") == Some(sldr_pptx::Disposition::Baked), "slide {n}: fence not reported as a baked region");
        for text_only in ["code_block_structure", "html"] {
            ensure!(found(n, zone, text_only).is_none(), "slide {n}: fence still accounted as {text_only}");
        }
    }
    ensure!(found(4, "content", "code_block_structure") == Some(sldr_pptx::Disposition::Unsupported), "a code fence must stay reported text");
    ensure!(found(4, "content", "region_raster").is_none(), "a code fence must not be reported as baked");
    let strict_findings: sldr_pptx::Report = serde_json::from_slice(&std::fs::read(&strict_report)?)?;
    ensure!(strict_findings.findings.iter().any(|f| f.feature == "region_raster"), "strict rejection must name the baked fence");

    // The unedited deck maps back without edits; the fences stay untouched.
    let slides = root.path().join("library/slides");
    let sources = |dir: &Path| -> anyhow::Result<Vec<(std::ffi::OsString, String)>> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            files.push((entry.file_name(), std::fs::read_to_string(entry.path())?));
        }
        files.sort();
        Ok(files)
    };
    let before = sources(&slides)?;
    let applied = command(root.path()).arg("import").arg(&deck).args(["--apply", "--dry-run", "--strict"]).output()?;
    ensure!(applied.status.success(), "{}", String::from_utf8_lossy(&applied.stderr));
    let stdout = String::from_utf8(applied.stdout).context("import output is not UTF-8")?;
    ensure!(stdout.contains("no edits to apply"), "{stdout}");
    ensure!(sources(&slides)? == before, "a dry run changed slide sources");
    Ok(())
}

/// A fence whose render paints nothing is not replaced by a blank picture:
/// it stays reported, unsupported source text.
#[test]
#[cfg(unix)]
fn graphic_fence_that_paints_nothing_stays_reported_text() -> anyhow::Result<()> {
    use anyhow::ensure;
    let root = tempfile::tempdir()?;
    fence_library(root.path())?;
    let (deck, report) = (root.path().join("lossy.pptx"), root.path().join("lossy.json"));
    let chrome = fake_chrome(root.path())?;
    // Replace the capture with a fully transparent one.
    image::RgbaImage::new(1920, 1080).save(root.path().join("capture.png"))?;
    let exported = command(root.path()).env("CHROME_BIN", &chrome)
        .args(["export", "fences", "--format", "pptx", "--allow-lossy"]).arg("-o").arg(&deck)
        .arg("--report-json").arg(&report).output()?;
    ensure!(exported.status.success(), "{}", String::from_utf8_lossy(&exported.stderr));
    let stdout = String::from_utf8_lossy(&exported.stdout);
    ensure!(stdout.contains("rendered nothing visible"), "{stdout}");
    let parsed: sldr_pptx::Report = serde_json::from_slice(&std::fs::read(&report)?)?;
    ensure!(!parsed.findings.iter().any(|f| f.feature == "region_raster"), "a blank render was reported as a baked picture");
    ensure!(parsed.findings.iter().any(|f| f.part == "ppt/slides/slide1.xml"
        && f.feature == "code_block_structure" && f.disposition == sldr_pptx::Disposition::Unsupported),
        "the unpictured fence must stay reported source text");
    Ok(())
}
