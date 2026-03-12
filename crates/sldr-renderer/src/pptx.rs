//! Minimal PPTX writer - creates a PowerPoint file from PNG slide screenshots
//!
//! A PPTX file is a ZIP archive containing:
//! - `[Content_Types].xml` - MIME type mappings
//! - `_rels/.rels` - root relationships
//! - `ppt/presentation.xml` - presentation definition
//! - `ppt/_rels/presentation.xml.rels` - slide relationships
//! - `ppt/slides/slideN.xml` - one per slide
//! - `ppt/slides/_rels/slideN.xml.rels` - image relationship per slide
//! - `ppt/slides/layout1.xml` - blank slide layout
//! - `ppt/slides/_rels/layout1.xml.rels`
//! - `ppt/slideMasters/slideMaster1.xml`
//! - `ppt/slideMasters/_rels/slideMaster1.xml.rels`
//! - `ppt/media/imageN.png` - slide screenshot images
//!
//! This is intentionally minimal - one image per slide, 16:9 aspect ratio.

use std::fmt::Write as _;
use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};

/// Width and height in EMUs (English Metric Units)
/// 1 inch = 914400 EMUs
/// 16:9 at 10" wide = 10" x 5.625"
const SLIDE_WIDTH_EMU: u64 = 9_144_000; // 10 inches
const SLIDE_HEIGHT_EMU: u64 = 5_143_500; // 5.625 inches

/// Create a PPTX file from a list of PNG image paths
pub fn create_pptx(images: &[impl AsRef<Path>], output: &Path) -> Result<()> {
    let file = std::fs::File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);

    let options = zip::write::FileOptions::<()>::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let slide_count = images.len();

    // [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types_xml(slide_count).as_bytes())?;

    // _rels/.rels
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(root_rels_xml().as_bytes())?;

    // ppt/presentation.xml
    zip.start_file("ppt/presentation.xml", options)?;
    zip.write_all(presentation_xml(slide_count).as_bytes())?;

    // ppt/_rels/presentation.xml.rels
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    zip.write_all(presentation_rels_xml(slide_count).as_bytes())?;

    // Slide master
    zip.start_file("ppt/slideMasters/slideMaster1.xml", options)?;
    zip.write_all(slide_master_xml().as_bytes())?;

    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)?;
    zip.write_all(slide_master_rels_xml().as_bytes())?;

    // Slide layout
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)?;
    zip.write_all(slide_layout_xml().as_bytes())?;

    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)?;
    zip.write_all(slide_layout_rels_xml().as_bytes())?;

    // Slides and their images
    for (i, img_path) in images.iter().enumerate() {
        let slide_num = i + 1;

        // Slide XML
        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), options)?;
        zip.write_all(slide_xml(slide_num).as_bytes())?;

        // Slide relationships
        zip.start_file(
            format!("ppt/slides/_rels/slide{slide_num}.xml.rels"),
            options,
        )?;
        zip.write_all(slide_rels_xml(slide_num).as_bytes())?;

        // Image file
        let img_data = std::fs::read(img_path.as_ref())
            .with_context(|| format!("Failed to read image: {}", img_path.as_ref().display()))?;
        zip.start_file(format!("ppt/media/image{slide_num}.png"), options)?;
        zip.write_all(&img_data)?;
    }

    zip.finish()?;
    Ok(())
}

fn content_types_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
"#,
    );

    for i in 1..=slide_count {
        let _ = writeln!(
            xml,
            "  <Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        );
    }

    xml.push_str("</Types>\n");
    xml
}

fn root_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
</Relationships>
"#
}

fn presentation_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId r:id="rId100"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
"#,
    );

    for i in 1..=slide_count {
        let id = 255 + i;
        let _ = writeln!(xml, "    <p:sldId id=\"{id}\" r:id=\"rId{i}\"/>");
    }

    xml.push_str(
        r#"  </p:sldIdLst>
  <p:sldSz cx="9144000" cy="5143500"/>
  <p:notesSz cx="6858000" cy="9144000"/>
</p:presentation>
"#,
    );
    xml
}

fn presentation_rels_xml(slide_count: usize) -> String {
    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId100" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
"#,
    );

    for i in 1..=slide_count {
        let _ = writeln!(
            xml,
            "  <Relationship Id=\"rId{i}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{i}.xml\"/>"
        );
    }

    xml.push_str("</Relationships>\n");
    xml
}

fn slide_xml(slide_num: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr/>
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="2" name="Slide {slide_num}"/>
          <p:cNvPicPr/>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rImg{slide_num}"/>
          <a:stretch><a:fillRect/></a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm>
            <a:off x="0" y="0"/>
            <a:ext cx="{SLIDE_WIDTH_EMU}" cy="{SLIDE_HEIGHT_EMU}"/>
          </a:xfrm>
          <a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
        </p:spPr>
      </p:pic>
    </p:spTree>
  </p:cSld>
</p:sld>
"#
    )
}

fn slide_rels_xml(slide_num: usize) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rImg{slide_num}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{slide_num}.png"/>
  <Relationship Id="rLayout1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>
"#
    )
}

fn slide_master_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:bg><p:bgPr><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
  <p:sldLayoutIdLst>
    <p:sldLayoutId r:id="rLayout1"/>
  </p:sldLayoutIdLst>
</p:sldMaster>
"#
}

fn slide_master_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rLayout1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
</Relationships>
"#
}

fn slide_layout_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             type="blank">
  <p:cSld>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr/>
    </p:spTree>
  </p:cSld>
</p:sldLayout>
"#
}

fn slide_layout_rels_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rMaster1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>
"#
}
