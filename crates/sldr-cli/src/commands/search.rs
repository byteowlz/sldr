//! Search command — find slides by name, title, tags, topic, description or
//! body text. Runs the core `find` ranking (ADR-0011), the same one the
//! studio finder and the API use, so a human, an agent and the UI see the
//! same order for the same query.

use super::json_output::JsonResponse;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use sldr_core::config::Config;
use sldr_core::find::{find, FindOpts, Hit};
use sldr_core::slide::SlideCollection;

/// JSON output for search results
#[derive(Serialize)]
struct SearchResults {
    query: String,
    count: usize,
    results: Vec<Hit>,
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

    let opts = FindOpts {
        tags: tags
            .map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default(),
        topic,
        limit: None,
    };
    let hits = find(query, &slides, &config.matching, &opts);

    if json {
        let output = SearchResults {
            query: query.to_string(),
            count: hits.len(),
            results: hits,
        };
        JsonResponse::success(output).print();
        return Ok(());
    }

    if hits.is_empty() {
        println!("  {}", "No matches found".dimmed());
        return Ok(());
    }

    println!();
    for hit in &hits {
        if long {
            println!("  {}", hit.relative_path.cyan().bold());
            println!("    Title: {}", hit.title.as_deref().unwrap_or("(no title)"));
            if !hit.tags.is_empty() {
                println!("    Tags:{}", format!(" [{}]", hit.tags.join(", ")).dimmed());
            }
            if let Some(topic) = &hit.topic {
                println!("    Topic: {topic}");
            }
            println!("    Layout: {}", hit.layout);
            for m in &hit.matched {
                println!("    {} {}", format!("{}:", m.field).yellow(), m.snippet.dimmed());
            }
            println!();
        } else {
            let title = hit
                .title
                .as_deref()
                .map(|t| format!(" - {t}"))
                .unwrap_or_default();
            let fields = hit
                .matched
                .iter()
                .map(|m| m.field)
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "  {}{}  {}",
                hit.relative_path.cyan(),
                title.dimmed(),
                fields.dimmed()
            );
        }
    }

    println!("\n  {} result(s)", hits.len().to_string().bold());

    Ok(())
}
