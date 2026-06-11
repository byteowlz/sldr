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

    // Get scaffold content
    let content = if let Some(scaffold_name) = scaffold {
        load_scaffold(&config, &scaffold_name)?
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

    Ok(())
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
    let scaffold_dir = config.scaffold_dir();

    // Try with and without .md extension
    let candidates = [
        scaffold_dir.join(format!("{scaffold_name}.md")),
        scaffold_dir.join(scaffold_name),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(std::fs::read_to_string(path)?);
        }
    }

    // Scaffold not found, use default with a warning
    println!(
        "  {} Scaffold '{}' not found, using default",
        "!".yellow(),
        scaffold_name
    );

    Ok(default_slide_scaffold(scaffold_name))
}
