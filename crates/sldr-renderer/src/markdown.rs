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

/// Raw-markdown halves of a slide body, split by `::` markers but **not**
/// converted to HTML — for consumers that map markdown to a non-HTML target
/// (the PPTX exporter turns each into OOXML paragraphs). Keys mirror layout
/// slot names. A plain slide yields only `content`; the others stay `None`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MarkdownSegments {
    pub heading: Option<String>,
    pub left: Option<String>,
    pub right: Option<String>,
    pub content: Option<String>,
    pub image: Option<String>,
}

/// Split a slide body into raw-markdown segments by the same `::left::` /
/// `::right::` / `::content::` / `::image::` markers `render_markdown` uses —
/// but without rendering to HTML. The marker shape is detected identically
/// (fenced code skipped, either order accepted), so a slide splits the same
/// way for HTML and PPTX.
pub fn split_segments(content: &str) -> MarkdownSegments {
    let markers = scan_markers(content);

    if markers.contains_key("left") && markers.contains_key("right") {
        let (l_start, l_end) = markers["left"];
        let (r_start, r_end) = markers["right"];
        if r_start >= l_start {
            let before = content[..l_start].trim();
            return MarkdownSegments {
                heading: (!before.is_empty()).then(|| before.to_string()),
                left: Some(content[l_end..r_start].trim().to_string()),
                right: Some(content[r_end..].trim().to_string()),
                ..Default::default()
            };
        }
    }

    if markers.contains_key("content") && markers.contains_key("image") {
        let (c_start, c_end) = markers["content"];
        let (i_start, i_end) = markers["image"];
        let (content_md, image_md) = if c_start < i_start {
            (&content[c_end..i_start], &content[i_end..])
        } else {
            (&content[c_end..], &content[i_end..c_start])
        };
        return MarkdownSegments {
            content: Some(content_md.trim().to_string()),
            image: Some(image_md.trim().to_string()),
            ..Default::default()
        };
    }

    MarkdownSegments {
        content: Some(content.trim().to_string()),
        ..Default::default()
    }
}

/// Remove split-marker lines that `render_markdown` won't consume — a lone
/// `::content::` with no `::image::`, markers on a single-block slide, a
/// `::right::` before its `::left::`. Such markers would otherwise render as
/// literal `::content::` text on the slide (the #1 authoring footgun). Returns
/// the cleaned content plus the distinct stray marker names found, so the build
/// can warn loudly. Active markers (part of a recognized pair) are kept —
/// `render_markdown` consumes them by position. Fence-aware, like `scan_markers`.
pub fn strip_stray_markers(content: &str) -> (String, Vec<String>) {
    let markers = scan_markers(content);

    // Mirror render_markdown's precedence: left+right (in order) wins, else
    // content+image. Only those markers are consumed; the rest are stray.
    let mut active: std::collections::HashSet<&str> = std::collections::HashSet::new();
    if markers.contains_key("left")
        && markers.contains_key("right")
        && markers["left"].0 <= markers["right"].0
    {
        active.insert("left");
        active.insert("right");
    } else if markers.contains_key("content") && markers.contains_key("image") {
        active.insert("content");
        active.insert("image");
    }

    let mut out = String::with_capacity(content.len());
    let mut stray: Vec<String> = Vec::new();
    let mut in_fence = false;
    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            out.push_str(line);
            continue;
        }
        if !in_fence {
            if let Some(name) = ["left", "right", "content", "image"]
                .into_iter()
                .find(|n| trimmed == format!("::{n}::"))
            {
                if !active.contains(name) {
                    if !stray.iter().any(|s| s == name) {
                        stray.push(name.to_string());
                    }
                    continue; // drop the stray marker line
                }
            }
        }
        out.push_str(line);
    }
    (out, stray)
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

/// Run a media reference through the pipeline and return the src to emit.
fn process_src(src: &str, media_config: &MediaConfig) -> String {
    match media::process_media_src(
        src,
        media_config.slide_dir.as_deref(),
        media_config.image_mode,
        media_config.assets_dir.as_deref(),
    ) {
        MediaEmbed::DataUri(data_uri) => data_uri,
        MediaEmbed::External(url) => url,
        MediaEmbed::AssetFile { html_src, .. } => html_src,
        MediaEmbed::NotFound(original) => original,
    }
}

/// Run the media pipeline over local image/video references inside raw
/// markup (```html / ```svg fences and inline HTML): `src="…"`, `href="…"`
/// and `xlink:href="…"` values that name a local media file are embedded or
/// copied exactly like a markdown image, so a hand-built diagram can use
/// icons and pictures from the slide's `media/` folder. URLs, anchors and
/// non-media paths pass through untouched.
fn rewrite_media_attrs(html: &str, media_config: &MediaConfig) -> String {
    const ATTRS: [&str; 3] = ["xlink:href=", "href=", "src="];
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    loop {
        // Next attribute occurrence (earliest of the three).
        let hit = ATTRS
            .iter()
            .filter_map(|a| rest.find(a).map(|i| (i, *a)))
            .min_by_key(|(i, _)| *i);
        let Some((idx, attr)) = hit else {
            out.push_str(rest);
            return out;
        };
        let value_start = idx + attr.len();
        out.push_str(&rest[..value_start]);
        rest = &rest[value_start..];
        let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            continue;
        };
        let Some(end) = rest[1..].find(quote) else {
            continue;
        };
        let value = &rest[1..1 + end];
        out.push(quote);
        if media::is_video(value) || media::is_local_image(value) {
            out.push_str(&process_src(value, media_config));
        } else {
            out.push_str(value);
        }
        out.push(quote);
        rest = &rest[1 + end + 1..];
    }
}

/// Render a ```bars fence — one `label | value` line per bar — into a
/// `.sldr-bars` list (label, track + fill, value), the survey / benchmark /
/// probability visual every deck needs and nobody should hand-build.
///
/// `value` is a number optionally followed by a unit that is shown as-is
/// (`93 %`, `4.2 s`, `12k`). Bars scale to the largest value, or to 100 when
/// any value carries a `%`. A `**bold**` label marks the highlighted bar.
/// Blank lines and `#` comments are skipped; a line without `|` is ignored.
fn render_bars(src: &str) -> String {
    struct Bar {
        label: String,
        value: f64,
        display: String,
        hi: bool,
    }
    let mut bars: Vec<Bar> = Vec::new();
    let mut percent = false;
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((label, rest)) = line.split_once('|') else {
            continue;
        };
        let (label, hi) = {
            let l = label.trim();
            match l.strip_prefix("**").and_then(|s| s.strip_suffix("**")) {
                Some(inner) => (inner.trim().to_string(), true),
                None => (l.to_string(), false),
            }
        };
        let rest = rest.trim();
        // Leading number (digits, one decimal point, optional sign / comma
        // thousands separator); the remainder is the displayed unit.
        let num_end = rest
            .char_indices()
            .take_while(|(i, c)| c.is_ascii_digit() || *c == '.' || *c == ',' || (*i == 0 && *c == '-'))
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
        let value = rest[..num_end].replace(',', "").parse::<f64>().unwrap_or(0.0);
        if rest.contains('%') {
            percent = true;
        }
        bars.push(Bar {
            label,
            value,
            display: rest.to_string(),
            hi,
        });
    }
    if bars.is_empty() {
        return String::new();
    }
    let max = bars.iter().map(|b| b.value).fold(0.0_f64, f64::max);
    let scale = if percent { max.max(100.0) } else { max };
    let mut out = String::from("<div class=\"sldr-bars\">\n");
    for b in bars {
        let pct = if scale > 0.0 {
            (b.value.max(0.0) / scale * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let cls = if b.hi { "sldr-bar is-hi" } else { "sldr-bar" };
        out.push_str(&format!(
            "<div class=\"{cls}\" style=\"--sldr-bar:{pct:.1}%\"><span class=\"sldr-bar-label\">{}</span><span class=\"sldr-bar-track\"><span class=\"sldr-bar-fill\"></span></span><span class=\"sldr-bar-value\">{}</span></div>\n",
            html_escape(&b.label),
            html_escape(&b.display)
        ));
    }
    out.push_str("</div>\n");
    out
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
    // (processed src, markdown title, Some(mime) when the src is a video,
    // pixel dimensions when known)
    let mut pending_media: Option<(String, String, Option<&'static str>, Option<(u32, u32)>)> =
        None;

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

                // ```mermaid → a diagram div the bundled mermaid.js renders
                // client-side when the slide is shown. The source is the div's
                // text content (escaped; the browser decodes it for mermaid).
                if code_lang == "mermaid" {
                    output.push_str("<div class=\"sldr-mermaid mermaid\">");
                    output.push_str(&html_escape(code_content.trim()));
                    output.push_str("</div>\n");
                    continue;
                }
                // ```svg / ```html → raw passthrough: render the markup instead
                // of highlighting it, so a hand-written SVG/HTML figure in a
                // fence works (the natural agent instinct).
                if code_lang == "svg" || code_lang == "html" {
                    output.push_str(&rewrite_media_attrs(code_content.trim(), media_config));
                    output.push('\n');
                    continue;
                }
                // ```bars → a horizontal bar list (label, bar, value) styled
                // by the diagram tokens. See `render_bars`.
                if code_lang == "bars" {
                    output.push_str(&render_bars(&code_content));
                    continue;
                }

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
                output.push_str(&rewrite_media_attrs(html.as_ref(), media_config));
            }
            Event::Start(Tag::Image { dest_url, title, .. }) => {
                in_image = true;
                image_alt.clear();
                // The tag is emitted at End(Image), once the alt text is
                // collected — a video needs it as an attribute up front.
                pending_media = Some((
                    process_src(dest_url.as_ref(), media_config),
                    title.to_string(),
                    media::is_video(dest_url.as_ref()).then(|| media::video_mime(dest_url.as_ref())),
                    media::image_dimensions(dest_url.as_ref(), media_config.slide_dir.as_deref()),
                ));
            }
            Event::End(TagEnd::Image) => {
                let (src, title, video_mime, dims) = pending_media.take().unwrap_or_default();
                let alt = html_escape(&image_alt);
                if let Some(mime) = video_mime {
                    // `![alt](clip.mp4 "poster.png")` — the markdown title
                    // doubles as the poster frame, run through the same
                    // media pipeline as any image.
                    let mut attrs = String::from("controls playsinline preload=\"metadata\"");
                    if !title.is_empty() {
                        attrs.push_str(" poster=\"");
                        attrs.push_str(&process_src(&title, media_config));
                        attrs.push('"');
                    }
                    if !alt.is_empty() {
                        attrs.push_str(" aria-label=\"");
                        attrs.push_str(&alt);
                        attrs.push('"');
                    }
                    output.push_str(&media::video_tag(&src, mime, &attrs));
                    output.push('\n');
                } else {
                    output.push_str("<img src=\"");
                    output.push_str(&src);
                    output.push('"');
                    if let Some((w, h)) = dims {
                        // Intrinsic size: gives the browser the aspect ratio
                        // before load, and lets collage layouts size cells by it.
                        output.push_str(&format!(" width=\"{w}\" height=\"{h}\""));
                    }
                    if !title.is_empty() {
                        output.push_str(" title=\"");
                        output.push_str(&html_escape(&title));
                        output.push('"');
                    }
                    output.push_str(" alt=\"");
                    output.push_str(&alt);
                    output.push_str("\" />\n");
                }
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
    fn test_raw_markup_media_attrs_go_through_pipeline() {
        // Local media inside ```html / ```svg fences and inline HTML is
        // resolved like a markdown image: a missing file stays as written
        // (NotFound passthrough), URLs and non-media hrefs are untouched.
        let cfg = MediaConfig {
            image_mode: ImageMode::Embed,
            slide_dir: Some(std::path::PathBuf::from("/nonexistent")),
            assets_dir: None,
        };
        let out = rewrite_media_attrs(
            "<a href=\"https://x.org/a.png\"><img src='media/i.svg'></a><use xlink:href=\"#id\"/>",
            &cfg,
        );
        assert_eq!(
            out,
            "<a href=\"https://x.org/a.png\"><img src='media/i.svg'></a><use xlink:href=\"#id\"/>"
        );
        // A real file gets embedded.
        let dir = std::env::temp_dir().join("sldr-raw-media-test");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("dot.svg"), "<svg xmlns='http://www.w3.org/2000/svg'/>").unwrap();
        let cfg = MediaConfig {
            image_mode: ImageMode::Embed,
            slide_dir: Some(dir.clone()),
            assets_dir: None,
        };
        let out = rewrite_media_attrs("<image href=\"dot.svg\"/>", &cfg);
        assert!(out.starts_with("<image href=\"data:image/svg+xml"), "{out}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_mermaid_block_becomes_diagram_div() {
        let md = "```mermaid\nflowchart LR\n  A --> B\n```";
        let html = markdown_to_html(md, &default_config());
        assert!(html.contains("<div class=\"sldr-mermaid mermaid\">"));
        assert!(html.contains("flowchart LR"));
        // Not a syntax-highlighted code block.
        assert!(!html.contains("sldr-code"));
        // Arrow source preserved (escaped) for mermaid to read.
        assert!(html.contains("A --&gt; B"));
    }

    #[test]
    fn test_svg_and_html_fences_pass_through_raw() {
        let svg = "```svg\n<svg><circle r=\"5\"/></svg>\n```";
        let html = markdown_to_html(svg, &default_config());
        assert!(html.contains("<svg><circle r=\"5\"/></svg>"));
        assert!(!html.contains("sldr-code"));

        let raw = "```html\n<b>bold</b>\n```";
        let html = markdown_to_html(raw, &default_config());
        assert!(html.contains("<b>bold</b>"));
        assert!(!html.contains("&lt;b&gt;"));
    }

    #[test]
    fn test_bars_fence_renders_bar_list() {
        let md = "```bars\n# next token\n**blue** | 93 %\nclear | 68 %\nnot | 26 %\n```";
        let html = markdown_to_html(md, &default_config());
        assert!(html.contains("<div class=\"sldr-bars\">"));
        assert_eq!(html.matches("class=\"sldr-bar-track\"").count(), 3);
        assert_eq!(html.matches("class=\"sldr-bar-fill\"").count(), 3);
        // Percent values scale against 100, the bold label is the highlight.
        assert!(html.contains("<div class=\"sldr-bar is-hi\" style=\"--sldr-bar:93.0%\">"));
        assert!(html.contains("<span class=\"sldr-bar-label\">blue</span>"));
        assert!(html.contains("<span class=\"sldr-bar-value\">93 %</span>"));
        assert!(!html.contains("next token"));
        assert!(!html.contains("sldr-code"));

        // Plain numbers scale against the largest value; units pass through.
        let md = "```bars\nA | 4,000 ms\nB | 2000 ms\n```";
        let html = markdown_to_html(md, &default_config());
        assert!(html.contains("style=\"--sldr-bar:100.0%\""));
        assert!(html.contains("style=\"--sldr-bar:50.0%\""));
        assert!(html.contains("<span class=\"sldr-bar-value\">4,000 ms</span>"));
    }

    #[test]
    fn test_split_segments_two_cols_raw() {
        let seg = split_segments("# Heading\n\n::left::\nLeft md\n::right::\nRight md");
        assert_eq!(seg.heading.as_deref(), Some("# Heading"));
        assert_eq!(seg.left.as_deref(), Some("Left md"));
        assert_eq!(seg.right.as_deref(), Some("Right md"));
        // Raw markdown, not HTML.
        assert!(seg.left.unwrap().starts_with("Left"));
    }

    #[test]
    fn test_split_segments_content_image() {
        let seg = split_segments("::content::\n# Hi\n::image::\n![](p.png)");
        assert_eq!(seg.content.as_deref(), Some("# Hi"));
        assert_eq!(seg.image.as_deref(), Some("![](p.png)"));
    }

    #[test]
    fn test_strip_stray_lone_content_marker() {
        // A lone ::content:: (no ::image::) is stray → removed + reported.
        let (out, stray) = strip_stray_markers("::content::\n\nSome body text.");
        assert!(!out.contains("::content::"));
        assert!(out.contains("Some body text."));
        assert_eq!(stray, vec!["content".to_string()]);
    }

    #[test]
    fn test_strip_keeps_active_pair() {
        // A real content+image pair is kept (render_markdown consumes it).
        let (out, stray) = strip_stray_markers("::content::\nText\n::image::\n![](x.png)");
        assert!(out.contains("::content::"));
        assert!(out.contains("::image::"));
        assert!(stray.is_empty());
    }

    #[test]
    fn test_strip_ignores_markers_in_code_fences() {
        let md = "::content::\n```\n::content::\n```";
        let (_out, stray) = strip_stray_markers(md);
        // Only the real lone marker (outside the fence) is stray, counted once.
        assert_eq!(stray, vec!["content".to_string()]);
    }

    #[test]
    fn test_split_segments_plain() {
        let seg = split_segments("Just body text.");
        assert_eq!(seg.content.as_deref(), Some("Just body text."));
        assert!(seg.left.is_none() && seg.right.is_none());
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
