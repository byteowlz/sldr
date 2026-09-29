//! `sldr where <slide>` / `sldr where --layout <name>` — where-used
//! (ADR-0011). Which playlists reference a slide (and when it was last
//! touched in git), or which slides use a layout. Derived from playlists and
//! history; nothing is stored, nothing is written.

use anyhow::{bail, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::fuzzy::SldrMatcher;
use sldr_core::slide::SlideCollection;
use sldr_core::usage::{git_last_touched, slides_using_layout, LayoutUsage, SlideUsage, UsageIndex};

pub fn run(slide: Option<&str>, layout: Option<&str>, json: bool) -> Result<()> {
    let config = Config::load()?;
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;

    match (slide, layout) {
        (Some(s), None) => slide_usage(&config, &slides, s, json),
        (None, Some(l)) => layout_usage(&slides, l, json),
        (None, None) => bail!("Give a slide, or --layout <name>"),
        (Some(_), Some(_)) => bail!("Give either a slide or --layout, not both"),
    }
}

fn slide_usage(config: &Config, slides: &SlideCollection, name: &str, json: bool) -> Result<()> {
    let slide = super::zones::resolve_slide(config, name)?;
    let matcher = SldrMatcher::new(config.matching.clone());
    let index = UsageIndex::build(&config.playlist_dir(), slides, &matcher);
    let usage = SlideUsage {
        slide: slide.relative_path.clone(),
        playlists: index.of(&slide.relative_path).to_vec(),
        last_touched: git_last_touched(&slide.path),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&usage)?);
        return Ok(());
    }
    println!("{} {}", "slide".dimmed(), usage.slide.cyan());
    if let Some(t) = &usage.last_touched {
        println!("{} {} ({})", "last touched".dimmed(), t.date, t.commit);
    }
    if usage.playlists.is_empty() {
        println!("{}", "referenced by no playlist".yellow());
    } else {
        println!(
            "{} {} {}",
            "in".dimmed(),
            usage.playlists.len().to_string().bold(),
            if usage.playlists.len() == 1 { "deck" } else { "decks" }
        );
        for p in &usage.playlists {
            println!(
                "  {:<28} {:>3}/{:<3} {}",
                p.name,
                p.position,
                p.of,
                p.flavor.as_deref().map(|f| format!("flavor {f}")).unwrap_or_default().dimmed()
            );
        }
    }
    for u in index.unresolved.iter().filter(|u| !u.entry.is_empty()) {
        println!(
            "{} playlist '{}' entry '{}' {}",
            "note:".yellow(),
            u.playlist,
            u.entry,
            u.reason
        );
    }
    Ok(())
}

fn layout_usage(slides: &SlideCollection, layout: &str, json: bool) -> Result<()> {
    let usage = LayoutUsage {
        layout: layout.to_string(),
        slides: slides_using_layout(layout, slides).into_iter().map(String::from).collect(),
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&usage)?);
        return Ok(());
    }
    println!(
        "{} {} {} {} {}",
        "layout".dimmed(),
        usage.layout.cyan(),
        "used by".dimmed(),
        usage.slides.len().to_string().bold(),
        if usage.slides.len() == 1 { "slide" } else { "slides" }
    );
    for s in &usage.slides {
        println!("  {s}");
    }
    Ok(())
}
