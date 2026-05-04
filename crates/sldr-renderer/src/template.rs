//! Template engine for wrapping slide HTML into layout structures
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
            html.push_str("  <div class=\"sldr-content\">\n");
            html.push_str("    ");
            html.push_str(content.trim());
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
            // base.css sets grid-template-columns accordingly (45% image / 1fr content).
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
