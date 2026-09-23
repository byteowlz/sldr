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
