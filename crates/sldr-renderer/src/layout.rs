//! Layout engine for wrapping slide HTML into layout structures
//!
//! Each layout (cover, two-cols, image-left, etc.) wraps the rendered
//! markdown content in the appropriate HTML structure with CSS hooks.

use crate::markdown::MarkdownOutput;

/// Options for `wrap_slide`. Bundled into a struct so we can grow the
/// per-slide knob set (alignment, transitions, click-step density, etc.)
/// without churning the call sites.
pub struct SlideOpts<'a> {
    pub index: usize,
    pub layout: &'a str,
    /// Horizontal alignment override: `Some("left" | "center" | "right")`.
    /// Emitted as `data-align` on the section so CSS overrides the
    /// layout's default without changing markup.
    pub align: Option<&'static str>,
    /// Vertical alignment override: `Some("top" | "center" | "bottom")`.
    pub valign: Option<&'static str>,
    pub rendered: MarkdownOutput,
    pub speaker_notes: Option<&'a str>,
}

/// Layouts whose content is a heading + a sequence of `<img>` paragraphs.
/// For these we promote each `<p><img alt="..."/></p>` to a `<figure>` so
/// alt text can render as a visible caption via `<figcaption>`. The
/// transform is layout-scoped — non-collage layouts keep bare `<p><img/>`.
fn is_collage_layout(layout: &str) -> bool {
    matches!(
        layout,
        "image-grid" | "image-row" | "image-portraits" | "image-stack"
    )
}

/// Promote image-only paragraphs to `<figure>` elements with optional
/// `<figcaption>` from the alt attribute, and gather all resulting figures
/// into a single `<div class="sldr-collage">` wrapper. The wrapper is
/// inserted at the position of the first image-paragraph so it lands
/// after any leading heading + subheadline.
///
/// pulldown-cmark groups consecutive image lines into one `<p>`, so each
/// tag inside an image-only paragraph is split into its own figure
/// (otherwise the grid would treat N images as 1 cell).
fn promote_images_to_figures(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut figures_buf = String::new();
    let mut wrapper_pos: Option<usize> = None;
    let mut rest = html;
    while let Some(p_start) = rest.find("<p>") {
        out.push_str(&rest[..p_start]);
        let after_open = &rest[p_start + 3..];
        let p_end_rel = match after_open.find("</p>") {
            Some(i) => i,
            None => {
                out.push_str(&rest[p_start..]);
                rest = "";
                break;
            }
        };
        let inner = &after_open[..p_end_rel];
        if let Some(figures) = try_render_image_paragraph(inner) {
            if wrapper_pos.is_none() {
                wrapper_pos = Some(out.len());
            }
            figures_buf.push_str(&figures);
        } else {
            out.push_str("<p>");
            out.push_str(inner);
            out.push_str("</p>");
        }
        rest = &after_open[p_end_rel + 4..];
    }
    out.push_str(rest);
    if !figures_buf.is_empty() {
        let pos = wrapper_pos.unwrap_or(out.len());
        let wrapper = format!("<div class=\"sldr-collage\">{figures_buf}</div>");
        out.insert_str(pos, &wrapper);
    }
    out
}

/// If `inner` is composed only of `<img/>` tags (any whitespace between),
/// return the rendered `<figure>` sequence. Otherwise return None — caller
/// keeps the paragraph as-is.
fn try_render_image_paragraph(inner: &str) -> Option<String> {
    let mut figures = String::new();
    let mut cursor = inner;
    let mut found_any = false;
    loop {
        let trimmed = cursor.trim_start();
        if trimmed.is_empty() {
            break;
        }
        if !trimmed.starts_with("<img ") {
            return None;
        }
        let close = trimmed.find("/>")?;
        let img_tag = &trimmed[..close + 2];
        let alt = extract_alt(img_tag).unwrap_or_default();
        figures.push_str("<figure class=\"sldr-collage-item\">");
        figures.push_str(img_tag);
        if !alt.is_empty() {
            figures.push_str("<figcaption>");
            figures.push_str(&alt);
            figures.push_str("</figcaption>");
        }
        figures.push_str("</figure>");
        cursor = &trimmed[close + 2..];
        found_any = true;
    }
    if found_any {
        Some(figures)
    } else {
        None
    }
}

fn extract_alt(img_tag: &str) -> Option<String> {
    let needle = " alt=\"";
    let start = img_tag.find(needle)? + needle.len();
    let end_rel = img_tag[start..].find('"')?;
    Some(img_tag[start..start + end_rel].to_string())
}

/// Wrap rendered markdown in a slide section with the appropriate layout.
///
/// Returns a complete `<section class="sldr-slide" ...>` element.
pub fn wrap_slide(opts: SlideOpts<'_>) -> String {
    use std::fmt::Write as _;

    let SlideOpts {
        index,
        layout,
        align,
        valign,
        rendered,
        speaker_notes,
    } = opts;

    let mut html = String::new();

    // 1-indexed, zero-padded page label. Emitted as a data attribute so
    // flavors can render page numbers via `content: attr(data-page)` —
    // CSS `counter-increment` would not work because the presenter sets
    // `display: none` on inactive slides and CSS counters only fire for
    // rendered elements (every slide would show '1'). See trx-jbpj.16.
    let page_num = index + 1;
    let _ = write!(
        html,
        "<section class=\"sldr-slide\" data-layout=\"{layout}\" data-index=\"{index}\" data-page=\"{page_num:02}\""
    );
    if let Some(a) = align {
        let _ = write!(html, " data-align=\"{a}\"");
    }
    if let Some(v) = valign {
        let _ = write!(html, " data-valign=\"{v}\"");
    }
    html.push_str(">\n");

    match rendered {
        MarkdownOutput::Single(content) => {
            let content = if is_collage_layout(layout) {
                promote_images_to_figures(content.trim())
            } else {
                content.trim().to_string()
            };
            html.push_str("  <div class=\"sldr-content\">\n");
            html.push_str("    ");
            html.push_str(&content);
            html.push('\n');
            html.push_str("  </div>\n");
        }
        MarkdownOutput::TwoCols {
            heading,
            left,
            right,
        } => {
            html.push_str("  <div class=\"sldr-content\">\n");

            if !heading.is_empty() {
                html.push_str("    ");
                html.push_str(heading.trim());
                html.push('\n');
            }

            html.push_str("    <div class=\"sldr-columns\">\n");
            html.push_str("      <div class=\"sldr-col\">\n");
            html.push_str("        ");
            html.push_str(left.trim());
            html.push('\n');
            html.push_str("      </div>\n");
            html.push_str("      <div class=\"sldr-col\">\n");
            html.push_str("        ");
            html.push_str(right.trim());
            html.push('\n');
            html.push_str("      </div>\n");
            html.push_str("    </div>\n");
            html.push_str("  </div>\n");
        }
        MarkdownOutput::ContentImage { content, image } => {
            // image-left puts the image column first; image-right puts content first.
            // base.css sets grid-layout-columns accordingly (45% image / 1fr content).
            let image_first = layout == "image-left";

            html.push_str("  <div class=\"sldr-content\">\n");
            html.push_str("    <div class=\"sldr-columns\">\n");

            let emit_content = |html: &mut String| {
                html.push_str("      <div class=\"sldr-col-content\">\n");
                html.push_str("        ");
                html.push_str(content.trim());
                html.push('\n');
                html.push_str("      </div>\n");
            };
            let emit_image = |html: &mut String| {
                html.push_str("      <div class=\"sldr-col-image\">\n");
                html.push_str("        ");
                html.push_str(image.trim());
                html.push('\n');
                html.push_str("      </div>\n");
            };

            if image_first {
                emit_image(&mut html);
                emit_content(&mut html);
            } else {
                emit_content(&mut html);
                emit_image(&mut html);
            }

            html.push_str("    </div>\n");
            html.push_str("  </div>\n");
        }
    }

    // Speaker notes (hidden, read by presenter.js)
    if let Some(notes) = speaker_notes {
        if !notes.trim().is_empty() {
            html.push_str("  <aside class=\"sldr-notes\">\n    ");
            html.push_str(notes.trim());
            html.push_str("\n  </aside>\n");
        }
    }

    html.push_str("</section>\n");
    html
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(layout: &'a str, rendered: MarkdownOutput, notes: Option<&'a str>) -> SlideOpts<'a> {
        SlideOpts {
            index: 0,
            layout,
            align: None,
            valign: None,
            rendered,
            speaker_notes: notes,
        }
    }

    #[test]
    fn test_wrap_default() {
        let html = wrap_slide(opts(
            "default",
            MarkdownOutput::Single("<h1>Hello</h1>".to_string()),
            None,
        ));
        assert!(html.contains("data-layout=\"default\""));
        assert!(html.contains("data-index=\"0\""));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(!html.contains("sldr-notes"));
        assert!(!html.contains("data-align"));
        assert!(!html.contains("data-valign"));
    }

    #[test]
    fn test_wrap_two_cols() {
        let mut o = opts(
            "two-cols",
            MarkdownOutput::TwoCols {
                heading: "<h1>Compare</h1>".to_string(),
                left: "<p>Left</p>".to_string(),
                right: "<p>Right</p>".to_string(),
            },
            Some("Speaker note here"),
        );
        o.index = 1;
        let html = wrap_slide(o);
        assert!(html.contains("data-layout=\"two-cols\""));
        assert!(html.contains("sldr-columns"));
        assert!(html.contains("<p>Left</p>"));
        assert!(html.contains("<p>Right</p>"));
        assert!(html.contains("sldr-notes"));
        assert!(html.contains("Speaker note here"));
    }

    #[test]
    fn test_wrap_with_notes() {
        let html = wrap_slide(opts(
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("My notes"),
        ));
        assert!(html.contains("<aside class=\"sldr-notes\">"));
        assert!(html.contains("My notes"));
    }

    #[test]
    fn test_wrap_empty_notes_omitted() {
        let html = wrap_slide(opts(
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("   "),
        ));
        assert!(!html.contains("sldr-notes"));
    }

    #[test]
    fn test_collage_promotes_images_to_figures() {
        let html = wrap_slide(opts(
            "image-grid",
            MarkdownOutput::Single(
                "<h1>Team</h1>\n<p><img src=\"a.jpg\" alt=\"Anna\" />\n</p>\n<p><img src=\"b.jpg\" alt=\"Bilal\" />\n</p>".to_string(),
            ),
            None,
        ));
        assert!(html.contains("<div class=\"sldr-collage\">"));
        assert!(html.contains("<figure class=\"sldr-collage-item\">"));
        assert!(html.contains("<figcaption>Anna</figcaption>"));
        assert!(html.contains("<figcaption>Bilal</figcaption>"));
        // Heading is preserved untouched, before the collage wrapper
        let h1_pos = html.find("<h1>Team</h1>").unwrap();
        let collage_pos = html.find("<div class=\"sldr-collage\">").unwrap();
        assert!(h1_pos < collage_pos);
    }

    #[test]
    fn test_collage_splits_multi_image_paragraph() {
        // pulldown-cmark groups consecutive image lines into one <p>.
        // Each <img> should still get its own <figure>.
        let html = wrap_slide(opts(
            "image-grid",
            MarkdownOutput::Single(
                "<p><img src=\"a.jpg\" alt=\"Anna\" />\n<img src=\"b.jpg\" alt=\"Bilal\" />\n<img src=\"c.jpg\" alt=\"Chen\" /></p>".to_string(),
            ),
            None,
        ));
        assert_eq!(html.matches("<figure class=\"sldr-collage-item\">").count(), 3);
        // All three figures share one wrapper.
        assert_eq!(html.matches("<div class=\"sldr-collage\">").count(), 1);
        assert!(html.contains("<figcaption>Anna</figcaption>"));
        assert!(html.contains("<figcaption>Bilal</figcaption>"));
        assert!(html.contains("<figcaption>Chen</figcaption>"));
    }

    #[test]
    fn test_collage_supports_subheadline() {
        // h1 + p (subhead) + figures: subhead stays out of the collage wrapper.
        let html = wrap_slide(opts(
            "image-grid",
            MarkdownOutput::Single(
                "<h1>Team</h1>\n<p>The folks behind the project.</p>\n<p><img src=\"a.jpg\" alt=\"Anna\" /></p>".to_string(),
            ),
            None,
        ));
        let subhead_pos = html.find("<p>The folks behind the project.</p>").unwrap();
        let collage_pos = html.find("<div class=\"sldr-collage\">").unwrap();
        assert!(subhead_pos < collage_pos);
    }

    #[test]
    fn test_collage_skips_non_image_paragraphs() {
        let html = wrap_slide(opts(
            "image-row",
            MarkdownOutput::Single(
                "<p>Just text</p>\n<p><img src=\"a.jpg\" alt=\"\" />\n</p>".to_string(),
            ),
            None,
        ));
        assert!(html.contains("<p>Just text</p>"));
        assert!(html.contains("<figure class=\"sldr-collage-item\">"));
        // Empty alt → no figcaption
        assert!(!html.contains("<figcaption>"));
    }

    #[test]
    fn test_non_collage_layout_keeps_bare_images() {
        let html = wrap_slide(opts(
            "default",
            MarkdownOutput::Single(
                "<p><img src=\"a.jpg\" alt=\"Anna\" />\n</p>".to_string(),
            ),
            None,
        ));
        assert!(!html.contains("<figure"));
        assert!(html.contains("<img src=\"a.jpg\" alt=\"Anna\""));
    }

    #[test]
    fn test_wrap_emits_alignment_attrs() {
        let mut o = opts(
            "default",
            MarkdownOutput::Single("<h1>Right-aligned</h1>".to_string()),
            None,
        );
        o.align = Some("right");
        o.valign = Some("bottom");
        let html = wrap_slide(o);
        assert!(html.contains("data-align=\"right\""));
        assert!(html.contains("data-valign=\"bottom\""));
    }
}
