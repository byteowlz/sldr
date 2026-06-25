//! Markdown body → OOXML paragraphs (`<a:p>`), for filling text placeholders
//! in a deck slide (trx-4s9s.4).
//!
//! This is the deterministic markdown→PPTX mapping, the native-text half of
//! the "honest wall": bullet lists become bulleted paragraphs (the project
//! default **square** bullet, `buChar` U+25AA), plain paragraphs get `buNone`,
//! and inline emphasis maps to run properties (`b`/`i`). Headings render as a
//! bold, bullet-less paragraph. Anything richer than runs of styled text
//! (tables, images, code blocks) is out of scope here — images are a `picture`
//! zone, not body text; a code block degrades to plain monospace paragraphs.
//!
//! Bullets are per-paragraph `<a:pPr>` props — there is no list element in
//! DrawingML; nesting is expressed by `lvl` + the indent of `marL`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Indent step per bullet level, in EMU (matches docs/pptx-spike).
const INDENT_EMU: i64 = 285_750;
/// Square bullet glyph (U+25AA) as an XML numeric entity.
const SQUARE_BULLET: &str = "&#9642;";

/// Convert a markdown fragment into a sequence of `<a:p>…</a:p>` strings,
/// ready to drop inside a `<p:txBody>`. Returns at least one (possibly empty)
/// paragraph so a placeholder is never structurally empty.
pub fn to_paragraphs(markdown: &str) -> Vec<String> {
    let mut w = Walker::default();
    let parser = Parser::new_ext(markdown, Options::empty());
    for ev in parser {
        w.event(ev);
    }
    w.flush_paragraph();
    if w.out.is_empty() {
        w.out.push(empty_paragraph());
    }
    w.out
}

/// One inline run: text plus the emphasis flags active when it was emitted.
struct Run {
    text: String,
    bold: bool,
    italic: bool,
    mono: bool,
}

#[derive(Default)]
struct Walker {
    out: Vec<String>,
    runs: Vec<Run>,
    /// Bullet nesting depth: 0 = not in a list (→ buNone), ≥1 = list level.
    list_level: usize,
    /// Whether the current paragraph is a list item (gets a bullet).
    in_item: bool,
    /// Heading paragraphs render bold, bullet-less.
    heading: bool,
    bold: usize,
    italic: usize,
    mono: usize,
}

impl Walker {
    fn event(&mut self, ev: Event) {
        match ev {
            Event::Start(Tag::List(_)) => {
                // Flush the parent item's own text (at the current level)
                // before descending — otherwise it inherits the deeper indent.
                self.flush_paragraph();
                self.list_level += 1;
            }
            Event::End(TagEnd::List(_)) => {
                self.list_level = self.list_level.saturating_sub(1);
            }
            Event::Start(Tag::Item) => {
                self.flush_paragraph();
                self.in_item = true;
            }
            Event::End(TagEnd::Item) => self.flush_paragraph(),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => {
                // A loose-list item wraps its text in a paragraph; keep the
                // bullet by only flushing here when not inside an item.
                if !self.in_item {
                    self.flush_paragraph();
                }
            }
            Event::Start(Tag::Heading { .. }) => self.heading = true,
            Event::End(TagEnd::Heading(_)) => {
                self.flush_paragraph();
                self.heading = false;
            }
            Event::Start(Tag::Strong) => self.bold += 1,
            Event::End(TagEnd::Strong) => self.bold = self.bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => self.italic += 1,
            Event::End(TagEnd::Emphasis) => self.italic = self.italic.saturating_sub(1),
            Event::Text(t) => self.push_text(&t),
            Event::Code(t) => {
                self.mono += 1;
                self.push_text(&t);
                self.mono -= 1;
            }
            Event::SoftBreak | Event::HardBreak => self.push_text(" "),
            // Code blocks: emit each line as a plain paragraph.
            Event::Start(Tag::CodeBlock(_)) => self.flush_paragraph(),
            Event::End(TagEnd::CodeBlock) => self.flush_paragraph(),
            _ => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let (bold, italic, mono) = (self.bold > 0 || self.heading, self.italic > 0, self.mono > 0);
        // Merge with the previous run if formatting matches.
        if let Some(last) = self.runs.last_mut() {
            if last.bold == bold && last.italic == italic && last.mono == mono {
                last.text.push_str(text);
                return;
            }
        }
        self.runs.push(Run {
            text: text.to_string(),
            bold,
            italic,
            mono,
        });
    }

    /// Emit the accumulated runs as one `<a:p>` and reset run state.
    fn flush_paragraph(&mut self) {
        if self.runs.is_empty() {
            self.in_item = false;
            return;
        }
        let bullet = self.in_item && self.list_level > 0;
        let ppr = if bullet {
            let level = self.list_level.max(1);
            let mar_l = INDENT_EMU * level as i64;
            let lvl = if level > 1 {
                format!(" lvl=\"{}\"", level - 1)
            } else {
                String::new()
            };
            format!(
                "<a:pPr marL=\"{mar_l}\" indent=\"-{INDENT_EMU}\"{lvl}><a:buFont typeface=\"Arial\"/><a:buChar char=\"{SQUARE_BULLET}\"/></a:pPr>"
            )
        } else {
            "<a:pPr marL=\"0\" indent=\"0\"><a:buNone/></a:pPr>".to_string()
        };

        let mut p = String::from("<a:p>");
        p.push_str(&ppr);
        for run in self.runs.drain(..) {
            p.push_str(&run_xml(&run));
        }
        p.push_str("</a:p>");
        self.out.push(p);
        self.in_item = false;
    }
}

fn run_xml(run: &Run) -> String {
    let mut rpr = String::from("<a:rPr lang=\"en-US\"");
    if run.bold {
        rpr.push_str(" b=\"1\"");
    }
    if run.italic {
        rpr.push_str(" i=\"1\"");
    }
    if run.mono {
        // Close the attributes, add a monospace latin typeface child.
        rpr.push_str("><a:latin typeface=\"Consolas\"/></a:rPr>");
    } else {
        rpr.push_str("/>");
    }
    format!("<a:r>{rpr}<a:t>{}</a:t></a:r>", xml_escape(&run.text))
}

fn empty_paragraph() -> String {
    "<a:p><a:pPr marL=\"0\" indent=\"0\"><a:buNone/></a:pPr></a:p>".to_string()
}

/// A single plain-text paragraph (no bullet), for chrome fields like the
/// headline or footer that are not markdown bodies.
pub fn plain_paragraph(text: &str) -> String {
    format!(
        "<a:p><a:pPr marL=\"0\" indent=\"0\"><a:buNone/></a:pPr><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r></a:p>",
        xml_escape(text)
    )
}

/// Reference a `HeadingLevel` so the import doesn't need its own copy; kept
/// here to document that heading depth is intentionally collapsed (all
/// headings render as one bold paragraph style).
#[allow(dead_code)]
fn heading_depth(level: HeadingLevel) -> u8 {
    level as u8
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bullets_get_square_buchar() {
        let ps = to_paragraphs("- first\n- second");
        assert_eq!(ps.len(), 2);
        assert!(ps[0].contains("buChar char=\"&#9642;\""));
        assert!(ps[0].contains("<a:t>first</a:t>"));
        assert!(ps[1].contains("<a:t>second</a:t>"));
    }

    #[test]
    fn test_plain_paragraph_gets_bunone() {
        let ps = to_paragraphs("Just a sentence.");
        assert_eq!(ps.len(), 1);
        assert!(ps[0].contains("<a:buNone/>"));
        assert!(!ps[0].contains("buChar"));
    }

    #[test]
    fn test_bold_and_italic_runs() {
        let ps = to_paragraphs("normal **bold** and *italic*");
        let p = &ps[0];
        assert!(p.contains("b=\"1\""));
        assert!(p.contains("i=\"1\""));
        assert!(p.contains("<a:t>bold</a:t>"));
        assert!(p.contains("<a:t>italic</a:t>"));
    }

    #[test]
    fn test_nested_list_levels() {
        let ps = to_paragraphs("- top\n    - nested");
        assert_eq!(ps.len(), 2);
        assert!(ps[0].contains(&format!("marL=\"{INDENT_EMU}\"")));
        assert!(ps[1].contains(&format!("marL=\"{}\"", INDENT_EMU * 2)));
        assert!(ps[1].contains("lvl=\"1\""));
    }

    #[test]
    fn test_heading_is_bold_bulletless() {
        let ps = to_paragraphs("# Title\n\nbody");
        assert!(ps[0].contains("b=\"1\""));
        assert!(ps[0].contains("<a:buNone/>"));
        assert!(ps[0].contains("<a:t>Title</a:t>"));
    }

    #[test]
    fn test_xml_escaping() {
        let ps = to_paragraphs("a < b & c");
        assert!(ps[0].contains("a &lt; b &amp; c"));
    }

    #[test]
    fn test_empty_yields_one_empty_paragraph() {
        let ps = to_paragraphs("");
        assert_eq!(ps.len(), 1);
        assert!(ps[0].contains("<a:buNone/>"));
    }

    #[test]
    fn test_inline_code_gets_mono_typeface() {
        let ps = to_paragraphs("use `cargo build` now");
        assert!(ps[0].contains("Consolas"));
        assert!(ps[0].contains("<a:t>cargo build</a:t>"));
    }
}
