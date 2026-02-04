//! Search command - find slides by content, tags, or metadata

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::slide::SlideCollection;

pub fn run(query: &str, tags: Option<String>, topic: Option<String>, long: bool) -> Result<()> {
    let config = Config::load()?;
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;

    println!(
        "{} slides matching '{}'",
        "Searching".green().bold(),
        query.cyan()
    );

    let query_lower = query.to_lowercase();
    let tag_filter: Vec<String> = tags
        .map(|t| t.split(',').map(|s| s.trim().to_lowercase()).collect())
        .unwrap_or_default();
    let topic_filter = topic.map(|t| t.to_lowercase());

    let mut matches = Vec::new();

    for slide in &slides.slides {
        let mut matched = false;
        let mut match_reasons = Vec::new();

        // Search in name
        if slide.name.to_lowercase().contains(&query_lower) {
            matched = true;
            match_reasons.push("name");
        }

        // Search in title
        if let Some(ref title) = slide.metadata.title {
            if title.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("title");
            }
        }

        // Search in description
        if let Some(ref desc) = slide.metadata.description {
            if desc.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("description");
            }
        }

        // Search in content
        if slide.content.to_lowercase().contains(&query_lower) {
            matched = true;
            match_reasons.push("content");
        }

        // Search in tags
        for tag in &slide.metadata.tags {
            if tag.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("tags");
                break;
            }
        }

        // Apply tag filter
        if !tag_filter.is_empty() {
            let slide_tags: Vec<String> = slide
                .metadata
                .tags
                .iter()
                .map(|t| t.to_lowercase())
                .collect();
            if !tag_filter.iter().any(|t| slide_tags.contains(t)) {
                matched = false;
            }
        }

        // Apply topic filter
        if let Some(ref topic_f) = topic_filter {
            if let Some(ref slide_topic) = slide.metadata.topic {
                if !slide_topic.to_lowercase().contains(topic_f) {
                    matched = false;
                }
            } else {
                matched = false;
            }
        }

        if matched {
            matches.push((slide, match_reasons));
        }
    }

    if matches.is_empty() {
        println!("  {}", "No matches found".dimmed());
        return Ok(());
    }

    println!();
    for (slide, reasons) in &matches {
        if long {
            let title = slide.metadata.title.as_deref().unwrap_or("(no title)");
            let tags = if slide.metadata.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", slide.metadata.tags.join(", "))
            };
            let reason_str = reasons.join(", ");

            println!("  {}", slide.relative_path.cyan().bold());
            println!("    Title: {title}");
            if !tags.is_empty() {
                println!("    Tags:{}", tags.dimmed());
            }
            if let Some(ref topic) = slide.metadata.topic {
                println!("    Topic: {topic}");
            }
            println!("    Matched in: {}", reason_str.yellow());
            println!();
        } else {
            let title = slide
                .metadata
                .title
                .as_deref()
                .map(|t| format!(" - {t}"))
                .unwrap_or_default();
            println!("  {}{}", slide.relative_path.cyan(), title.dimmed());
        }
    }

    println!("\n  {} result(s)", matches.len().to_string().bold());

    Ok(())
}
