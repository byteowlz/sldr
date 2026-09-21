//! Export accounting at the existing zone/content boundary.
use std::collections::BTreeSet;
use pulldown_cmark::{Event, Options, Parser, Tag};
use crate::{Disposition as D, Report, SlideInput, ZoneContent, ZoneRep};

pub(crate) fn deck(slides: &[SlideInput]) -> Report {
    let mut report = Report::default();
    for (i, slide) in slides.iter().enumerate() {
        let part = format!("ppt/slides/slide{}.xml", i + 1);
        if slide.layout.zones.is_empty() {
            report.record(Some(&part), &part, &slide.layout.name, "missing_zones", D::Unsupported,
                "Annotate this layout with zones or use --flatten");
        }
        if slide.layout.css.as_ref().is_some_and(|css| !css.trim().is_empty()) {
            report.record(Some(&part), &part, "layout", "custom_css", D::Unsupported,
                "Native zones do not project custom layout CSS; use explicit lossy/flatten policy");
        }
        let mut names = BTreeSet::new();
        for zone in &slide.layout.zones {
            if !names.insert(&zone.name) {
                report.record(Some(&part), &part, &zone.name, "duplicate_zone", D::Conflicting, "Give each zone a unique name");
            }
            if ![zone.x, zone.y, zone.w, zone.h].iter().all(|n| n.is_finite()) ||
                zone.x < 0.0 || zone.y < 0.0 || zone.w <= 0.0 || zone.h <= 0.0 || zone.x + zone.w > 100.01 || zone.y + zone.h > 100.01 {
                report.record(Some(&part), &part, &zone.name, "invalid_geometry", D::Conflicting, "Use finite positive geometry inside the slide");
            }
            if matches!(zone.rep, ZoneRep::Shape | ZoneRep::Bake) {
                report.record(Some(&part), &part, &zone.name, zone.rep.as_token(), D::Unsupported,
                    "Native shape/region baking is not implemented for this zone");
            }
        }
        let mut fields = BTreeSet::new();
        for (name, content) in &slide.fields {
            if !fields.insert(name) {
                report.record(Some(&part), &part, name, "duplicate_field", D::Conflicting, "Resolve duplicate field ownership");
            }
            let zone = slide.layout.zones.iter().find(|z| z.name == *name);
            let supported = zone.is_some_and(|z| match content {
                ZoneContent::Picture { .. } => true,
                _ => z.rep == ZoneRep::PlaceholderText && z.ph.is_some(),
            });
            if !supported {
                report.record(Some(&part), &part, name, "unmapped_field", D::Unsupported, "Declare a compatible zone; this field would be omitted");
                continue;
            }
            match content {
                ZoneContent::Markdown(md) => markdown(md, &part, name, &mut report),
                ZoneContent::Picture { bytes, ext, .. } if bytes.is_empty() || !matches!(ext.as_str(), "png" | "jpeg" | "jpg" | "gif") => {
                    report.record(Some(&part), &part, name, "invalid_image", D::Conflicting, "Supply nonempty supported image bytes and extension");
                }
                _ => {},
            }
            report.record(Some(&part), &part, name, "zone_content", D::Converted,
                "Editable text or positioned image; font metrics and line wrapping depend on the editor");
        }
    }
    report
}

fn markdown(md: &str, part: &str, zone: &str, report: &mut Report) {
    for (event, range) in Parser::new_ext(md, Options::all()).into_offset_iter() {
        let feature = match event {
            Event::Start(Tag::Image { .. }) => "inline_image",
            Event::Start(Tag::Link { .. }) => "hyperlink",
            Event::Start(Tag::Heading { .. }) => "heading_depth_collapsed",
            Event::Start(Tag::CodeBlock(_)) => "code_block_structure",
            Event::Start(Tag::List(Some(_))) => "ordered_list_numbering",
            Event::Start(Tag::Table(_)) => "table",
            Event::Start(Tag::BlockQuote(_)) => "blockquote",
            Event::Start(Tag::Strikethrough) => "strikethrough",
            Event::Start(Tag::FootnoteDefinition(_)) | Event::FootnoteReference(_) => "footnote",
            Event::Html(_) | Event::InlineHtml(_) => "html",
            Event::InlineMath(_) | Event::DisplayMath(_) => "math",
            Event::TaskListMarker(_) => "task_list",
            Event::Rule => "horizontal_rule",
            Event::HardBreak => "hard_line_break",
            _ => continue,
        };
        report.record(Some(part), part, &format!("{zone}@{}", range.start), feature, D::Unsupported,
            "Only paragraphs, unordered bullets, emphasis and inline code have native text mappings");
    }
}
