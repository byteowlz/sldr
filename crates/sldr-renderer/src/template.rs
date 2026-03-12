//! Template engine for wrapping slide HTML into layout structures
//!
//! Each layout (cover, two-cols, image-left, etc.) wraps the rendered
//! markdown content in the appropriate HTML structure with CSS hooks.

use crate::markdown::MarkdownOutput;

/// Wrap rendered markdown in a slide section with the appropriate layout.
///
/// Returns a complete `<section class="sldr-slide" ...>` element.
pub fn wrap_slide(
    index: usize,
    layout: &str,
    rendered: MarkdownOutput,
    speaker_notes: Option<&str>,
) -> String {
    use std::fmt::Write as _;

    let mut html = String::new();

    let _ = writeln!(
        html,
        "<section class=\"sldr-slide\" data-layout=\"{layout}\" data-index=\"{index}\">"
    );

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

    #[test]
    fn test_wrap_default() {
        let html = wrap_slide(
            0,
            "default",
            MarkdownOutput::Single("<h1>Hello</h1>".to_string()),
            None,
        );
        assert!(html.contains("data-layout=\"default\""));
        assert!(html.contains("data-index=\"0\""));
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(!html.contains("sldr-notes"));
    }

    #[test]
    fn test_wrap_two_cols() {
        let html = wrap_slide(
            1,
            "two-cols",
            MarkdownOutput::TwoCols {
                heading: "<h1>Compare</h1>".to_string(),
                left: "<p>Left</p>".to_string(),
                right: "<p>Right</p>".to_string(),
            },
            Some("Speaker note here"),
        );
        assert!(html.contains("data-layout=\"two-cols\""));
        assert!(html.contains("sldr-columns"));
        assert!(html.contains("<p>Left</p>"));
        assert!(html.contains("<p>Right</p>"));
        assert!(html.contains("sldr-notes"));
        assert!(html.contains("Speaker note here"));
    }

    #[test]
    fn test_wrap_with_notes() {
        let html = wrap_slide(
            0,
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("My notes"),
        );
        assert!(html.contains("<aside class=\"sldr-notes\">"));
        assert!(html.contains("My notes"));
    }

    #[test]
    fn test_wrap_empty_notes_omitted() {
        let html = wrap_slide(
            0,
            "cover",
            MarkdownOutput::Single("<h1>Title</h1>".to_string()),
            Some("   "),
        );
        assert!(!html.contains("sldr-notes"));
    }
}
