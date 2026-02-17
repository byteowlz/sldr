//! Search command - find slides by content, tags, or metadata

use super::json_output::JsonResponse;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use sldr_core::config::Config;
use sldr_core::slide::SlideCollection;

/// JSON output for a search result entry
#[derive(Serialize)]
struct SearchResultEntry {
    name: String,
    relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    matched_in: Vec<String>,
}

/// JSON output for search results
#[derive(Serialize)]
struct SearchResults {
    query: String,
    count: usize,
    results: Vec<SearchResultEntry>,
}

pub fn run(
    query: &str,
    tags: Option<String>,
    topic: Option<String>,
    long: bool,
    json: bool,
) -> Result<()> {
    let config = Config::load()?;
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;

    if !json {
        println!(
            "{} slides matching '{}'",
            "Searching".green().bold(),
            query.cyan()
        );
    }

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
            match_reasons.push("name".to_string());
        }

        // Search in title
        if let Some(ref title) = slide.metadata.title {
            if title.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("title".to_string());
            }
        }

        // Search in description
        if let Some(ref desc) = slide.metadata.description {
            if desc.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("description".to_string());
            }
        }

        // Search in content
        if slide.content.to_lowercase().contains(&query_lower) {
            matched = true;
            match_reasons.push("content".to_string());
        }

        // Search in tags
        for tag in &slide.metadata.tags {
            if tag.to_lowercase().contains(&query_lower) {
                matched = true;
                match_reasons.push("tags".to_string());
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

    if json {
        let results: Vec<SearchResultEntry> = matches
            .iter()
            .map(|(slide, reasons)| SearchResultEntry {
                name: slide.name.clone(),
                relative_path: slide.relative_path.clone(),
                title: slide.metadata.title.clone(),
                tags: slide.metadata.tags.clone(),
                topic: slide.metadata.topic.clone(),
                matched_in: reasons.clone(),
            })
            .collect();
        let output = SearchResults {
            query: query.to_string(),
            count: results.len(),
            results,
        };
        JsonResponse::success(output).print();
        return Ok(());
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
