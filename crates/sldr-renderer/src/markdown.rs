//! Markdown to HTML conversion with syntax highlighting
//!
//! Uses pulldown-cmark for markdown parsing and syntect for code highlighting.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

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
/// The `::left::` and `::right::` column markers are detected and returned
/// separately so the template engine can place them in the correct slots.
pub fn render_markdown(content: &str, media_config: &MediaConfig) -> MarkdownOutput {
    // Check for column markers
    if content.contains("::left::") && content.contains("::right::") {
        return render_two_cols(content, media_config);
    }

    let html = markdown_to_html(content, media_config);
    MarkdownOutput::Single(html)
}

/// Result of rendering markdown - either a single block or split columns
pub enum MarkdownOutput {
    /// Standard single-content slide
    Single(String),
    /// Two-column slide with optional heading, left column, right column
    TwoCols {
        heading: String,
        left: String,
        right: String,
    },
}

/// Parse a two-column slide by splitting on ::left:: and ::right:: markers
fn render_two_cols(content: &str, media_config: &MediaConfig) -> MarkdownOutput {
    // Split on ::left:: first
    let (before_left, after_left) = match content.split_once("::left::") {
        Some((before, after)) => (before.trim(), after),
        None => return MarkdownOutput::Single(markdown_to_html(content, media_config)),
    };

    // Split the remainder on ::right::
    let (left_md, right_md) = match after_left.split_once("::right::") {
        Some((left, right)) => (left.trim(), right.trim()),
        None => return MarkdownOutput::Single(markdown_to_html(content, media_config)),
    };

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
    let ts = ThemeSet::load_defaults();
    let theme = &ts.themes["base16-ocean.dark"];

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

                // Try syntax highlighting
                let highlighted = if code_lang.is_empty() {
                    None
                } else if let Some(syntax) = ss.find_syntax_by_token(&code_lang) {
                    highlighted_html_for_string(&code_content, &ss, syntax, theme).ok()
                } else {
                    None
                };

                if let Some(html) = highlighted {
                    // syntect wraps in <pre style="..."><code>...</code></pre>
                    // Replace with our class-based approach
                    let html = inject_code_class(&html);
                    output.push_str(&html);
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

/// Replace syntect's inline-style <pre> with our class-based version
fn inject_code_class(syntect_html: &str) -> String {
    // syntect output: <pre style="background-color:#2b303b;">...
    // We want: <pre class="sldr-code" style="...">...
    syntect_html.replacen("<pre style=\"", "<pre class=\"sldr-code\" style=\"", 1)
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
            MarkdownOutput::Single(_) => panic!("Expected TwoCols"),
        }
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
    }
}
