//! `sldr import <file.pptx>` — round-trip a sldr-generated PowerPoint back
//! into slide markdown (trx-4s9s.5). The inverse of `sldr export --format pptx`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;

use sldr_core::config::Config;
use sldr_pptx::ImportedSlide;

pub fn run(file: &str, out: Option<String>) -> Result<()> {
    let config = Config::load()?;
    let bytes = std::fs::read(file).with_context(|| format!("Failed to read {file}"))?;

    println!(
        "{} slides from {}",
        "Importing".green().bold(),
        file.cyan()
    );

    let slides = sldr_pptx::import(&bytes)?;
    if slides.is_empty() {
        anyhow::bail!("No slides found in {file}");
    }

    let out_dir = match out {
        Some(o) => PathBuf::from(o),
        None => config.slide_dir().join("imported"),
    };
    let media_dir = out_dir.join("media");
    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("Failed to create {}", out_dir.display()))?;

    let mut written = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        // Write any embedded images first, then point the body at them.
        let mut body = slide.body.clone();
        for img in &slide.images {
            std::fs::create_dir_all(&media_dir).ok();
            let img_path = media_dir.join(&img.file_name);
            std::fs::write(&img_path, &img.bytes)
                .with_context(|| format!("Failed to write {}", img_path.display()))?;
            body = body.replacen("](IMAGE)", &format!("](media/{})", img.file_name), 1);
        }

        let stem = slide_stem(slide, i);
        let md_path = out_dir.join(format!("{stem}.md"));
        std::fs::write(&md_path, render_markdown(slide, &body))
            .with_context(|| format!("Failed to write {}", md_path.display()))?;
        written.push(md_path);
    }

    println!(
        "\n{} {} slide(s) → {}",
        "Success!".green().bold(),
        written.len(),
        out_dir.display().to_string().cyan()
    );
    for p in &written {
        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
            println!("  {} {name}", "·".dimmed());
        }
    }
    Ok(())
}

/// `001-my-title` from the slide's title, else `001-slide`.
fn slide_stem(slide: &ImportedSlide, idx: usize) -> String {
    let slug = slide
        .title
        .as_deref()
        .map(|t| {
            t.to_lowercase()
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '-' })
                .collect::<String>()
                .split('-')
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join("-")
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "slide".to_string());
    format!("{:03}-{slug}", idx + 1)
}

/// Frontmatter + body for one imported slide.
fn render_markdown(slide: &ImportedSlide, body: &str) -> String {
    let mut fm = String::from("---\n");
    if let Some(t) = &slide.title {
        fm.push_str(&format!("title: {}\n", yaml_value(t)));
    }
    if let Some(s) = &slide.subtitle {
        fm.push_str(&format!("subtitle: {}\n", yaml_value(s)));
    }
    fm.push_str(&format!("layout: {}\n", slide.layout));
    if let Some(f) = &slide.footer {
        fm.push_str(&format!("footer: {}\n", yaml_value(f)));
    }
    if let Some(s) = &slide.source {
        fm.push_str(&format!("source: {}\n", yaml_value(s)));
    }
    if let Some(u) = &slide.source_url {
        fm.push_str(&format!("source_url: {}\n", yaml_value(u)));
    }
    fm.push_str("---\n\n");
    fm.push_str(body);
    fm.push('\n');
    fm
}

/// Quote a YAML scalar when it could be misread (colons, leading specials);
/// otherwise emit it bare.
fn yaml_value(s: &str) -> String {
    let needs_quote = s.contains(':')
        || s.contains('#')
        || s.starts_with(['-', '[', '{', '*', '&', '!', '|', '>', '\'', '"', '@', '`'])
        || s.trim() != s;
    if needs_quote {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
