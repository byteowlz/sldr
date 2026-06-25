//! New command - create a new slide

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use std::io::Write;

pub fn run(name: &str, scaffold: Option<String>, dir: Option<&String>) -> Result<()> {
    let config = Config::load()?;

    let slide_dir = config.slide_dir();

    // Build the path
    let mut path = slide_dir.clone();
    if let Some(ref subdir) = dir {
        path = path.join(subdir);
    }

    // Ensure .md extension
    let filename = if std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        name.to_string()
    } else {
        format!("{name}.md")
    };

    path = path.join(&filename);

    // Check if file exists
    if path.exists() {
        anyhow::bail!("Slide already exists: {}", path.display());
    }

    println!("{} slide '{}'", "Creating".green().bold(), name.cyan());

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Get scaffold content. A named scaffold's `{{title}}` / `{{name}}`
    // tokens are substituted here so the created slide is ready to use (the
    // default no-scaffold path already fills them in) — agents shouldn't have
    // to hand-edit placeholders left in image paths and frontmatter.
    let content = if let Some(scaffold_name) = scaffold {
        let raw = load_scaffold(&config, &scaffold_name)?;
        apply_tokens(&raw, name)
    } else {
        default_slide_scaffold(name)
    };

    // Write the file
    let mut file = std::fs::File::create(&path)?;
    file.write_all(content.as_bytes())?;

    println!(
        "{} Created {}",
        "Done!".green().bold(),
        path.display().to_string().cyan()
    );

    // Offer to open in editor
    println!(
        "  {} Edit with: {} {}",
        "i".blue(),
        "$EDITOR".dimmed(),
        path.display()
    );
    // Point agents at the structured paths so they don't hand-write markers.
    println!(
        "  {} Batch/multilingual? {}. Diagrams: a ```mermaid fence renders; vector art via {}.",
        "i".blue(),
        "sldr slides create (JSON)".dimmed(),
        "![](x.svg)".dimmed()
    );

    Ok(())
}

/// Substitute scaffold placeholder tokens. `{{name}}` → the slide's file stem
/// (handy for `media/{{name}}.png`), `{{title}}` → a humanized version of it.
fn apply_tokens(content: &str, name: &str) -> String {
    let stem = name.trim_end_matches(".md");
    let title = stem.replace(['_', '-'], " ");
    content.replace("{{title}}", &title).replace("{{name}}", stem)
}

fn default_slide_scaffold(name: &str) -> String {
    let title = name.trim_end_matches(".md").replace(['_', '-'], " ");

    format!(
        r#"---
title: {title}
description: ""
tags: []
layout: default
---

# {title}

<!-- Your slide content here -->
"#
    )
}

fn load_scaffold(config: &Config, scaffold_name: &str) -> Result<String> {
    // Resolution order: library/scaffolds -> configured extra dir ->
    // built-ins bundled in the binary. Unresolved fails loudly (ADR-0007).
    let dirs = config.scaffold_dirs();
    for dir in &dirs {
        for path in [
            dir.join(format!("{scaffold_name}.md")),
            dir.join(scaffold_name),
        ] {
            if path.exists() {
                return Ok(std::fs::read_to_string(path)?);
            }
        }
    }

    let bundled_name = format!("{}.md", scaffold_name.trim_end_matches(".md"));
    if let Some(s) = crate::scaffolds::SCAFFOLDS
        .iter()
        .find(|s| s.name == bundled_name)
    {
        return Ok(s.content.to_string());
    }

    anyhow::bail!(
        "Scaffold '{scaffold_name}' not found (searched: {}, built-ins). Available built-ins: {}",
        dirs.iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", "),
        crate::scaffolds::SCAFFOLDS
            .iter()
            .map(|s| s.name.trim_end_matches(".md"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
