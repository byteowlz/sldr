//! Markdown to HTML conversion with syntax highlighting
//!
//! Uses pulldown-cmark for markdown parsing and syntect for code highlighting.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// Class style for highlighted code. Prefixed so generated class names
/// (`syn-keyword`, `syn-string`, ...) can't collide with user CSS. The
/// matching color rules are emitted per flavor from `[code] syntax_theme`
/// into the flavor's own <style data-flavor> block — highlighting is part
/// of the style layer and swaps with the flavor at runtime (ADR-0003).
pub const SYN_CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "syn-" };

use crate::media::{self, ImageMode, MediaEmbed};

/// Configuration for media handling during markdown rendering
#[derive(Debug, Clone)]
pub struct MediaConfig {
    /// How to handle local images
    pub image_mode: ImageMode,
    /// Directory containing the slide (for resolving relative image paths)
    pub slide_dir: Option<std::path::PathBuf>,
    /// Directory to copy assets to (for `ImageMode::External`)
    pub assets_dir: Option<std::path::PathBuf>,

}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            image_mode: ImageMode::Embed,
            slide_dir: None,
            assets_dir: None,
        }
    }
}

/// Converts markdown content to HTML with syntax-highlighted code blocks.
///
/// Recognized layout markers (each split is mutually exclusive):
/// - `::left::` + `::right::` — two-column layout (`two-cols`, `two-cols-header`)
/// - `::content::` + `::image::` — content + image column (`image-left`,
///   `image-right`). The layout engine decides DOM order based on layout.
///
/// A marker counts only when it stands alone on a line *outside* fenced
/// code blocks — so slides can document the markers in code samples and
/// inline code without getting split apart. Unrecognized markers pass
/// through as raw text.
pub fn render_markdown(content: &str, media_config: &MediaConfig) -> MarkdownOutput {
    let markers = scan_markers(content);
    if markers.contains_key("left") && markers.contains_key("right") {
        return render_two_cols(content, &markers, media_config);
    }
    if markers.contains_key("content") && markers.contains_key("image") {
        return render_content_image(content, &markers, media_config);
    }

    let html = markdown_to_html(content, media_config);
    MarkdownOutput::Single(html)
}

/// Byte offsets of split markers: lines that are exactly `::name::`
/// (whitespace-tolerant), skipping fenced code blocks (``` or ~~~).
/// Only the first occurrence of each marker is recorded.
fn scan_markers(content: &str) -> std::collections::HashMap<&'static str, (usize, usize)> {
    let mut markers = std::collections::HashMap::new();
    let mut in_fence = false;
    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            for name in ["left", "right", "content", "image"] {
                if trimmed == format!("::{name}::") {
                    markers
                        .entry(name)
                        .or_insert((offset, offset + line.len()));
                }
            }
        }
        offset += line.len();
    }
    markers
}

/// Result of rendering markdown — either a single block or split columns
pub enum MarkdownOutput {
    /// Standard single-content slide
    Single(String),
    /// Two-column slide with optional heading, left column, right column
    TwoCols {
        heading: String,
        left: String,
        right: String,
    },
    /// Content + image split (used by image-left / image-right layouts).
    /// The layout engine picks DOM order from the layout name; the
    /// markdown can declare the two halves in either order.
    ContentImage { content: String, image: String },
}

/// Parse a content+image slide using pre-scanned marker positions.
///
/// The two markers may appear in either order in the markdown — we identify
/// the halves by marker name, not position. The layout engine places them
/// in the correct DOM order based on the layout (`image-left` puts image
/// first, `image-right` puts content first).
fn render_content_image(
    input: &str,
    markers: &std::collections::HashMap<&'static str, (usize, usize)>,
    media_config: &MediaConfig,
) -> MarkdownOutput {
    let (c_start, c_end) = markers["content"];
    let (i_start, i_end) = markers["image"];

    let (content_md, image_md) = if c_start < i_start {
        (&input[c_end..i_start], &input[i_end..])
    } else {
        (&input[c_end..], &input[i_end..c_start])
    };

    MarkdownOutput::ContentImage {
        content: markdown_to_html(content_md.trim(), media_config),
        image: markdown_to_html(image_md.trim(), media_config),
    }
}

/// Parse a two-column slide using pre-scanned marker positions.
fn render_two_cols(
    content: &str,
    markers: &std::collections::HashMap<&'static str, (usize, usize)>,
    media_config: &MediaConfig,
) -> MarkdownOutput {
    let (l_start, l_end) = markers["left"];
    let (r_start, r_end) = markers["right"];
    if r_start < l_start {
        // ::right:: before ::left:: is not a recognized shape.
        return MarkdownOutput::Single(markdown_to_html(content, media_config));
    }

    let before_left = content[..l_start].trim();
    let left_md = content[l_end..r_start].trim();
    let right_md = content[r_end..].trim();

    let heading = if before_left.is_empty() {
        String::new()
    } else {
        markdown_to_html(before_left, media_config)
    };

    let left = markdown_to_html(left_md, media_config);
    let right = markdown_to_html(right_md, media_config);

    MarkdownOutput::TwoCols {
        heading,
        left,
        right,
    }
}

/// Core markdown -> HTML conversion with syntax highlighting and media embedding
fn markdown_to_html(input: &str, media_config: &MediaConfig) -> String {
    let ss = SyntaxSet::load_defaults_newlines();

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(input, options);

    let mut output = String::new();
    let mut in_code_block = false;
    let mut code_lang = String::new();
    let mut code_content = String::new();
    let mut in_image = false;
    let mut image_alt = String::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                code_content.clear();
                code_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        // Strip any build annotations like {all|1-3}
                        let lang_str = lang.as_ref();
                        lang_str
                            .split_once(['{', ' '])
                            .map_or(lang_str, |(base, _)| base)
                            .to_string()
                    }
                    CodeBlockKind::Indented => String::new(),
                };
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;

                // Class-based syntax highlighting: spans carry syn-*
                // classes; the colors live in the flavor's style block.
                let highlighted = if code_lang.is_empty() {
                    None
                } else if let Some(syntax) = ss.find_syntax_by_token(&code_lang) {
                    let mut generator =
                        ClassedHTMLGenerator::new_with_class_style(syntax, &ss, SYN_CLASS_STYLE);
                    let mut ok = true;
                    for line in LinesWithEndings::from(&code_content) {
                        if generator
                            .parse_html_for_line_which_includes_newline(line)
                            .is_err()
                        {
                            ok = false;
                            break;
                        }
                    }
                    ok.then(|| generator.finalize())
                } else {
                    None
                };

                if let Some(inner) = highlighted {
                    output.push_str("<pre class=\"sldr-code\"><code class=\"syn-code\">");
                    output.push_str(&inner);
                    output.push_str("</code></pre>\n");
                } else {
                    // Fallback: plain code block
                    output.push_str("<pre class=\"sldr-code\"><code>");
                    output.push_str(&html_escape(&code_content));
                    output.push_str("</code></pre>\n");
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    code_content.push_str(text.as_ref());
                } else if in_image {
                    // Collect alt text for image tag
                    image_alt.push_str(text.as_ref());
                } else {
                    output.push_str(&html_escape(text.as_ref()));
                }
            }
            Event::Code(text) => {
                output.push_str("<code>");
                output.push_str(&html_escape(text.as_ref()));
                output.push_str("</code>");
            }
            Event::SoftBreak => {
                output.push('\n');
            }
            Event::HardBreak => {
                output.push_str("<br />\n");
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                output.push_str(html.as_ref());
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                in_image = true;
                image_alt.clear();

                // Process the image source through the media pipeline
                let src = dest_url.as_ref();
                let processed_src = match media::process_media_src(
                    src,
                    media_config.slide_dir.as_deref(),
                    media_config.image_mode,
                    media_config.assets_dir.as_deref(),
                ) {
                    MediaEmbed::DataUri(data_uri) => data_uri,
                    MediaEmbed::External(url) => url,
                    MediaEmbed::AssetFile { html_src, .. } => html_src,
                    MediaEmbed::NotFound(original) => original,
                };

                output.push_str("<img src=\"");
                output.push_str(&processed_src);
                output.push('"');
                if !title.is_empty() {
                    output.push_str(" title=\"");
                    output.push_str(title.as_ref());
                    output.push('"');
                }
                // alt text will be added when we hit End(Image)
                output.push_str(" alt=\"");
            }
            Event::End(TagEnd::Image) => {
                output.push_str(&html_escape(&image_alt));
                output.push_str("\" />\n");
                in_image = false;
                image_alt.clear();
            }
            Event::Start(tag) => {
                write_open_tag(&mut output, &tag);
            }
            Event::End(tag) => {
                write_close_tag(&mut output, tag);
            }
            Event::Rule => {
                output.push_str("<hr />\n");
            }
            Event::FootnoteReference(name) => {
                output.push_str("<sup class=\"sldr-fn\"><a href=\"#fn-");
                output.push_str(name.as_ref());
                output.push_str("\">");
                output.push_str(name.as_ref());
                output.push_str("</a></sup>");
            }
            Event::TaskListMarker(checked) => {
                if checked {
                    output.push_str("<input type=\"checkbox\" checked disabled /> ");
                } else {
                    output.push_str("<input type=\"checkbox\" disabled /> ");
                }
            }
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                // Math support can be added later
            }
        }
    }

    output
}

/// Write an opening HTML tag for a pulldown-cmark tag
fn write_open_tag(out: &mut String, tag: &Tag<'_>) {
    match tag {
        Tag::Paragraph => out.push_str("<p>"),
        Tag::Heading { level, .. } => {
            out.push_str("<h");
            out.push_str(&(*level as u8).to_string());
            out.push('>');
        }
        Tag::BlockQuote(_) => out.push_str("<blockquote>\n"),
        Tag::List(Some(start)) => {
            if *start == 1 {
                out.push_str("<ol>\n");
            } else {
                out.push_str("<ol start=\"");
                out.push_str(&start.to_string());
                out.push_str("\">\n");
            }
        }
        Tag::List(None) => out.push_str("<ul>\n"),
        Tag::Item => out.push_str("<li>"),
        Tag::Emphasis => out.push_str("<em>"),
        Tag::Strong => out.push_str("<strong>"),
        Tag::Strikethrough => out.push_str("<del>"),
        Tag::Link { dest_url, title, .. } => {
            out.push_str("<a href=\"");
            out.push_str(dest_url.as_ref());
            out.push('"');
            if !title.is_empty() {
                out.push_str(" title=\"");
                out.push_str(title.as_ref());
                out.push('"');
            }
            out.push('>');
        }
        Tag::Image { .. } => {
            // Handled in main loop with media processing
        }
        Tag::Table(alignments) => {
            out.push_str("<table>\n");
            // Store alignments for later use - we handle them in thead/tbody
            let _ = alignments; // Used implicitly via column positions
        }
        Tag::TableHead => out.push_str("<thead>\n<tr>\n"),
        Tag::TableRow => out.push_str("<tr>\n"),
        Tag::TableCell => out.push_str("<td>"),
        Tag::FootnoteDefinition(name) => {
            out.push_str("<div class=\"sldr-footnote\" id=\"fn-");
            out.push_str(name.as_ref());
            out.push_str("\">\n");
        }
        Tag::HtmlBlock | Tag::MetadataBlock(_) | Tag::CodeBlock(_) => {} // handled in main loop
        Tag::DefinitionList => out.push_str("<dl>\n"),
        Tag::DefinitionListTitle => out.push_str("<dt>"),
        Tag::DefinitionListDefinition => out.push_str("<dd>"),
    }
}

/// Write a closing HTML tag
fn write_close_tag(out: &mut String, tag: TagEnd) {
    match tag {
        TagEnd::Paragraph => out.push_str("</p>\n"),
        TagEnd::Heading(level) => {
            out.push_str("</h");
            out.push_str(&(level as u8).to_string());
            out.push_str(">\n");
        }
        TagEnd::BlockQuote(_) => out.push_str("</blockquote>\n"),
        TagEnd::List(ordered) => {
            if ordered {
                out.push_str("</ol>\n");
            } else {
                out.push_str("</ul>\n");
            }
        }
        TagEnd::Item => out.push_str("</li>\n"),
        TagEnd::Emphasis => out.push_str("</em>"),
        TagEnd::Strong => out.push_str("</strong>"),
        TagEnd::Strikethrough => out.push_str("</del>"),
        TagEnd::Link => out.push_str("</a>"),
        TagEnd::Image => {
            // Handled in main loop with media processing
        }
        TagEnd::Table => out.push_str("</tbody>\n</table>\n"),
        TagEnd::TableHead => out.push_str("</tr>\n</thead>\n<tbody>\n"),
        TagEnd::TableRow => out.push_str("</tr>\n"),
        TagEnd::TableCell => out.push_str("</td>\n"),
        TagEnd::FootnoteDefinition => out.push_str("</div>\n"),
        TagEnd::HtmlBlock | TagEnd::MetadataBlock(_) | TagEnd::CodeBlock => {} // handled elsewhere
        TagEnd::DefinitionList => out.push_str("</dl>\n"),
        TagEnd::DefinitionListTitle => out.push_str("</dt>\n"),
        TagEnd::DefinitionListDefinition => out.push_str("</dd>\n"),
    }
}

/// Basic HTML escaping for text content
fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> MediaConfig {
        MediaConfig::default()
    }

    #[test]
    fn test_simple_markdown() {
        let html = markdown_to_html("# Hello\n\nWorld", &default_config());
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<p>World</p>"));
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let html = markdown_to_html(md, &default_config());
        assert!(html.contains("sldr-code"));
        assert!(html.contains("main"));
    }

    #[test]
    fn test_two_cols() {
        let md = "# Title\n\n::left::\n\nLeft stuff\n\n::right::\n\nRight stuff";
        let result = render_markdown(md, &default_config());
        match result {
            MarkdownOutput::TwoCols {
                heading,
                left,
                right,
            } => {
                assert!(heading.contains("Title"));
                assert!(left.contains("Left stuff"));
                assert!(right.contains("Right stuff"));
            }
            _ => panic!("Expected TwoCols"),
        }
    }

    #[test]
    fn test_content_image_split() {
        let md = "::content::\n\n# Side by side\n\nBody copy.\n\n::image::\n\n![](pic.png)";
        let result = render_markdown(md, &default_config());
        match result {
            MarkdownOutput::ContentImage { content, image } => {
                assert!(content.contains("Side by side"));
                assert!(content.contains("Body copy"));
                assert!(image.contains("pic.png"));
            }
            _ => panic!("Expected ContentImage"),
        }
    }

    #[test]
    fn test_content_image_split_reversed_order() {
        // Markers in opposite order: ::image:: before ::content::.
        let md = "::image::\n\n![](pic.png)\n\n::content::\n\n# Title\n\nBody.";
        let result = render_markdown(md, &default_config());
        match result {
            MarkdownOutput::ContentImage { content, image } => {
                assert!(content.contains("Title"));
                assert!(content.contains("Body"));
                assert!(image.contains("pic.png"));
            }
            _ => panic!("Expected ContentImage"),
        }
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    }

    #[test]
    fn markers_inside_code_fences_do_not_split() {
        // A slide documenting the markers must not get cut apart by its
        // own code samples (fence-aware, line-anchored scanning).
        let md = "# Real two-col\n\n::left::\nBefore code.\n```markdown\n::left::\nfirst\n::right::\nsecond\n```\n::right::\nRight column.\n";
        match render_markdown(md, &MediaConfig::default()) {
            MarkdownOutput::TwoCols { left, right, .. } => {
                assert!(left.contains("Before code."), "left: {left}");
                assert!(left.contains("first"), "code stays in left: {left}");
                assert!(right.contains("Right column."), "right: {right}");
            }
            _ => panic!("expected TwoCols"),
        }
    }

    #[test]
    fn inline_code_markers_do_not_split() {
        let md = "::left::\nUses `::left::` and `::right::` markers.\n::right::\nRight.\n";
        match render_markdown(md, &MediaConfig::default()) {
            MarkdownOutput::TwoCols { left, right, .. } => {
                assert!(left.contains("markers"), "left: {left}");
                assert!(right.trim_end().ends_with("Right.</p>"), "right: {right}");
            }
            _ => panic!("expected TwoCols"),
        }
    }

    #[test]
    fn markers_must_stand_alone_on_a_line() {
        let md = "Some prose mentioning ::left:: and ::right:: inline.\n";
        assert!(matches!(
            render_markdown(md, &MediaConfig::default()),
            MarkdownOutput::Single(_)
        ));
    }
}
