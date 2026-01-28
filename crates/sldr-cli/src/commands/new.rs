//! New command - create a new slide

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use std::io::Write;

pub fn run(name: &str, template: Option<String>, dir: Option<&String>) -> Result<()> {
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

    println!(
        "{} slide '{}'",
        "Creating".green().bold(),
        name.cyan()
    );

    // Create parent directories
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Get template content
    let content = if let Some(template_name) = template {
        load_template(&config, &template_name)?
    } else {
        default_slide_template(name)
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

fn default_slide_template(name: &str) -> String {
    let title = name
        .trim_end_matches(".md")
        .replace(['_', '-'], " ");

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

fn load_template(config: &Config, template_name: &str) -> Result<String> {
    let template_dir = config.template_dir();

    // Try with and without .md extension
    let candidates = [
        template_dir.join(format!("{template_name}.md")),
        template_dir.join(template_name),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(std::fs::read_to_string(path)?);
        }
    }

    // Template not found, use default with a warning
    println!(
        "  {} Template '{}' not found, using default",
        "!".yellow(),
        template_name
    );

    Ok(default_slide_template(template_name))
}
