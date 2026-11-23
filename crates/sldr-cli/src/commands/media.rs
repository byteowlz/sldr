//! `sldr media ls` / `sldr media add <slide> <file>` — library media
//! (ADR-0011). Lists every image/video with the slides that use it; stores a
//! file beside the slide that will reference it and prints the reference.

use anyhow::{Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::media::{list_media, store_beside, MediaKind};
use sldr_core::slide::SlideCollection;
use std::path::Path;

pub fn ls(unused_only: bool, json: bool) -> Result<()> {
    let config = Config::load()?;
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let mut index = list_media(&config.slide_dir(), &config.library().join("media"), &slides);
    if unused_only {
        index.files.retain(|f| f.used_by.is_empty());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&index)?);
        return Ok(());
    }
    if index.files.is_empty() {
        println!("{}", "no media files".dimmed());
        return Ok(());
    }
    for f in &index.files {
        let kind = match f.kind {
            MediaKind::Image => "img",
            MediaKind::Video => "vid",
            MediaKind::Other => "   ",
        };
        let uses = match f.used_by.len() {
            0 => "unused".yellow().to_string(),
            1 => format!("in {}", f.used_by[0]).dimmed().to_string(),
            n => format!("in {n} slides").dimmed().to_string(),
        };
        println!("  {kind} {:<48} {:>8}  {}", f.path.cyan(), human(f.bytes), uses);
    }
    println!("\n  {} file(s)", index.files.len().to_string().bold());
    Ok(())
}

pub fn add(slide: &str, file: &Path, name: Option<&str>, overwrite: bool) -> Result<()> {
    let config = Config::load()?;
    let target = super::zones::resolve_slide(&config, slide)?;
    let bytes = std::fs::read(file).with_context(|| format!("Could not read {}", file.display()))?;
    let file_name = name
        .map(str::to_string)
        .or_else(|| file.file_name().map(|n| n.to_string_lossy().to_string()))
        .context("file has no name")?;
    let (path, reference) = store_beside(&target, &file_name, &bytes, overwrite)?;
    println!(
        "{} {} → {}",
        "Stored".green().bold(),
        path.display().to_string().dimmed(),
        reference.cyan()
    );
    println!("  reference it from {} as: ![…]({})", target.relative_path, reference);
    Ok(())
}

fn human(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    //! The usage scan lives in sldr-core and restates what the renderer
    //! loads. These tests render for real and fail when the two disagree,
    //! so the scan cannot drift from the build.

    use anyhow::{bail, ensure, Result};
    use sldr_core::media::{fence_renders_markup, rendered_references};
    use sldr_renderer::markdown::{render_markdown, MarkdownOutput, MediaConfig};
    use sldr_renderer::ImageMode;

    const PNG_1X1: [u8; 68] = [
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
        0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xb5, 0x1c, 0x0c, 0x02, 0x00, 0x00, 0x00, 0x0b, 0x49,
        0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x64, 0x60, 0x00, 0x00, 0x00, 0x06, 0x00, 0x02, 0x30, 0x81, 0xd0, 0x2f, 0x00,
        0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    /// Render each body beside a real `media/p.png` and fail unless the
    /// scan's verdict (`counted`) matches whether the renderer loaded the
    /// file: the path is gone from the output and a data URI stands in.
    fn agrees_with_renderer(tag: &str, cases: &[(String, bool)]) -> Result<()> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos();
        let dir = std::env::temp_dir().join(format!("sldr-media-cli-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(dir.join("media"))?;
        std::fs::write(dir.join("media/p.png"), PNG_1X1)?;
        let config = MediaConfig { image_mode: ImageMode::Embed, slide_dir: Some(dir.clone()), assets_dir: None };
        for (body, counted) in cases {
            let MarkdownOutput::Single(html) = render_markdown(body, &config) else {
                bail!("expected a single-content slide for {body:?}");
            };
            let loaded = !html.contains("media/p.png") && html.contains("data:image/");
            ensure!(
                *counted == loaded,
                "{body:?}: the usage scan says used = {counted}, the renderer produced {html:?}"
            );
        }
        Ok(std::fs::remove_dir_all(&dir)?)
    }

    #[test]
    fn fence_rule_matches_what_the_renderer_passes_through() -> Result<()> {
        let infos = [
            "svg", "html", "svg {1}", "html{all|1-3}", "html title", "", "text", "python", "xml", "markdown", "mermaid",
            "bars", "SVG", "Html", "svgz",
        ];
        let fence = |info: &&str| (format!("```{info}\n<image href=\"media/p.png\"/>\n```\n"), fence_renders_markup(info));
        agrees_with_renderer("fence", &infos.iter().map(fence).collect::<Vec<_>>())
    }

    #[test]
    fn reference_forms_match_what_the_renderer_loads() -> Result<()> {
        let bodies = [
            // Counted and loaded.
            "![x](media/p.png)\n",
            "![x](./media/p.png \"title\")\n",
            "<img src=\"media/p.png\">\n",
            "<img src='media/p.png'>\n",
            "<svg><image href=\"media/p.png\"/></svg>\n",
            "```svg\n<image href=\"media/p.png\"/>\n```\n",
            "```svg\n<image xlink:href='media/p.png'/>\n```\n",
            "```html\n<img src=\"./media/p.png\">\n```\n",
            "~~~html {1}\n<a href=\"media/p.png\">x</a>\n~~~\n",
            // Neither counted nor loaded.
            "```svg\n<image href=media/p.png />\n```\n",
            "```svg\n<image href = \"media/p.png\"/>\n```\n",
            "```text\n<img src=\"media/p.png\">\n![x](media/p.png)\n```\n",
            "```python\nprint('<image href=\"media/p.png\"/>')\n```\n",
            "~~~\n![x](media/p.png)\n~~~\n",
            "````markdown\n```svg\n<image href=\"media/p.png\"/>\n```\n````\n",
            "[link](media/p.png)\n",
        ];
        let form = |body: &&str| ((*body).to_string(), !rendered_references(body).is_empty());
        agrees_with_renderer("forms", &bodies.iter().map(form).collect::<Vec<_>>())
    }
}
