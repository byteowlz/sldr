//! Which media a library reports as used (trx-qsg9). "Used" means the
//! renderer would load the file: a fence that renders as a graphic counts,
//! a fence shown as code does not.

use anyhow::{ensure, Result};
use sldr_core::media::{fence_renders_markup, list_media, rendered_references};
use sldr_core::slide::SlideCollection;

/// Index a throwaway library and report it one line per media file, as
/// `sldr media ls` does: `path: slide, slide` or `path: unused`. `slides`
/// are `(path, body)` and `media` are paths, both relative to the slide
/// directory; `../media/x` is the library-level folder beside it.
fn usage_of(tag: &str, slides: &[(&str, &str)], media: &[&str]) -> Result<Vec<String>> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
    let root = std::env::temp_dir().join(format!("sldr-media-usage-{tag}-{}-{nanos}", std::process::id()));
    let slide_dir = root.join("slides");
    let in_library = |rel: &&str| rel.strip_prefix("../").map_or_else(|| slide_dir.join(rel), |shared| root.join(shared));
    let files = slides.iter().map(|(rel, body)| (slide_dir.join(rel), *body)).chain(media.iter().map(|rel| (in_library(rel), "x")));
    for (path, text) in files {
        std::fs::create_dir_all(path.parent().unwrap_or(&root))?;
        std::fs::write(&path, text)?;
    }
    let collection = SlideCollection::load_from_dir(&slide_dir)?;
    let index = list_media(&slide_dir, &root.join("media"), &collection);
    std::fs::remove_dir_all(&root)?;
    let line = |path: &str, used_by: &[String]| match used_by {
        [] => format!("{path}: unused"),
        slides => format!("{path}: {}", slides.join(", ")),
    };
    Ok(index.files.iter().map(|f| line(&f.path, &f.used_by)).collect())
}

/// An equality check that fails the test by returning an error, with both
/// sides shown.
fn same<G: PartialEq<W> + std::fmt::Debug, W: std::fmt::Debug>(got: &G, want: &W) -> Result<()> {
    ensure!(got == want, "got {got:#?}\nwant {want:#?}");
    Ok(())
}

/// The issue's library: a.png only in an svg fence, b.png only in an html
/// fence, c.png as a markdown image, d.png nowhere. Reporting a.png or
/// b.png unused invites a delete that breaks the slide.
#[test]
fn media_referenced_from_rendered_fences_is_in_use() -> Result<()> {
    let svg = "# A\n\n```svg\n<svg viewBox=\"0 0 10 10\"><image href=\"media/a.png\" width=\"10\" height=\"10\"/></svg>\n```\n";
    let got = usage_of(
        "issue",
        &[("slide-a.md", svg), ("slide-b.md", "# B\n\n```html\n<img src=\"media/b.png\">\n```\n"), ("slide-c.md", "# C\n\n![x](media/c.png)\n")],
        &["media/a.png", "media/b.png", "media/c.png", "media/d.png"],
    )?;
    same(&got, &["media/a.png: slide-a.md", "media/b.png: slide-b.md", "media/c.png: slide-c.md", "media/d.png: unused"])
}

/// A fence shown as code is a code sample, not a use, whatever its text
/// looks like: a named language, no language, or a longer fence quoting an
/// svg fence.
#[test]
fn media_named_only_in_code_fences_is_unused() -> Result<()> {
    let body = concat!(
        "# N\n\n",
        "```text\n<img src=\"media/e.png\">\n![x](media/e.png)\n```\n\n",
        "```python\nprint('<image href=\"media/f.png\"/>')\n```\n\n",
        "~~~\n<img src=\"media/g.png\">\n~~~\n\n",
        "````markdown\n```svg\n<image href=\"media/h.png\"/>\n```\n````\n",
    );
    let got = usage_of("code", &[("slide-n.md", body)], &["media/e.png", "media/f.png", "media/g.png", "media/h.png"])?;
    same(&got, &["media/e.png: unused", "media/f.png: unused", "media/g.png: unused", "media/h.png: unused"])
}

/// Fence references resolve like markdown ones: relative to the slide's own
/// folder, through `./` and `..`, into a sibling folder or the library-level
/// `media/`. A URL that merely ends in a local file's name is not a use.
#[test]
fn fence_references_resolve_relative_to_the_slide() -> Result<()> {
    let body = concat!(
        "```svg\n",
        "<image xlink:href='./media/own.png'/>\n",
        "<image href=\"../other/media/sibling.png\"/>\n",
        "<image href=\"../../media/library.png\"/>\n",
        "<image href=\"https://example.invalid/media/remote.png\"/>\n",
        "```\n",
    );
    let media = ["deck/media/own.png", "other/media/sibling.png", "../media/library.png", "media/remote.png"];
    same(
        &usage_of("relative", &[("deck/slide-x.md", body)], &media)?,
        &[
            "../media/library.png: deck/slide-x.md",
            "deck/media/own.png: deck/slide-x.md",
            "media/remote.png: unused",
            "other/media/sibling.png: deck/slide-x.md",
        ],
    )
}

#[test]
fn only_svg_and_html_fences_render_as_markup() -> Result<()> {
    for info in ["svg", "html", "svg {1}", "html{all|1-3}", " svg ", "html title"] {
        ensure!(fence_renders_markup(info), "{info:?} should render as markup");
    }
    for info in ["", "text", "python", "xml", "mermaid", "bars", "SVG", "Html", "svgz", "htmlbars", "svg\t{1}"] {
        ensure!(!fence_renders_markup(info), "{info:?} should be shown as code");
    }
    Ok(())
}

/// Quoting, attribute and path forms: single and double quotes, `href`,
/// `xlink:href` and `src` count; remote, data, absolute, unquoted and
/// `href = "…"` do not.
#[test]
fn rendered_references_reads_the_forms_the_renderer_resolves() -> Result<()> {
    let body = concat!(
        "![a](media/md.png \"title\")\n",
        "<img src='media/single.png'> <a href=\"media/linked.png\">x</a>\n",
        "<svg><image xlink:href=\"media/inline.png\"/></svg>\n",
        "```svg\n",
        "<image href=\"media/href.png\"/>\n",
        "<image xlink:href='./media/xlink.png'/>\n",
        "<image href=\"../shared/media/up.png\"/>\n",
        "<image href=\"https://example.invalid/media/remote.png\"/>\n",
        "<image href=\"data:image/png;base64,AAAA\"/>\n",
        "<image href=\"/abs/media/root.png\"/>\n",
        "<image href=media/unquoted.png />\n",
        "<image href = \"media/spaced.png\"/>\n",
        "```\n",
        "~~~html {1}\n",
        "<video src=\"media/clip.mp4\"></video>\n",
        "~~~\n",
    );
    same(
        &rendered_references(body),
        &[
            "media/md.png",
            "media/single.png",
            "media/linked.png",
            "media/inline.png",
            "media/href.png",
            "./media/xlink.png",
            "../shared/media/up.png",
            "media/clip.mp4",
        ],
    )
}

#[test]
fn rendered_references_follows_fence_boundaries() -> Result<()> {
    // A closing fence needs the opener's character.
    let mixed = "```svg\n<image href=\"media/one.png\"/>\n~~~\n<image href=\"media/two.png\"/>\n```\n";
    same(&rendered_references(mixed), &["media/one.png", "media/two.png"])?;
    // An unclosed fence runs to the end of the body.
    let unclosed = "```python\nopen(\"x\") # <img src=\"media/code.png\">\n![x](media/code.png)\n";
    let nothing: [&str; 0] = [];
    same(&rendered_references(unclosed), &nothing)?;
    // Not a fence: four columns in, or backticks in the info string.
    let prose = "    ```text\n![x](media/indented.png)\n``` `svg` ```\n![y](media/inline.png)\n";
    same(&rendered_references(prose), &["media/indented.png", "media/inline.png"])
}
