//! Slides command - slide management utilities

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Skeleton;
use sldr_core::slide::SlideCollection;
use std::io::Write;
use std::path::PathBuf;

/// Derive empty slides from a skeleton - creates stub files for any missing slides
pub fn derive(skeleton_name: &str, template: Option<&str>, dry_run: bool) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} slides from skeleton '{}'",
        "Deriving".green().bold(),
        skeleton_name.cyan()
    );

    // Load the skeleton
    let skeleton = load_skeleton(&config, skeleton_name)?;

    // Load existing slides
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let existing_names = slides.names();
    let matcher = SldrMatcher::new(config.matching.clone());

    // Find missing slides
    let mut missing_slides: Vec<&str> = Vec::new();
    let mut found_slides: Vec<(&str, String)> = Vec::new();

    for slide_ref in &skeleton.slides {
        match matcher.resolve(slide_ref, &existing_names) {
            ResolveResult::Found(result) => {
                found_slides.push((slide_ref.as_str(), result.value));
            }
            ResolveResult::NotFound => {
                missing_slides.push(slide_ref.as_str());
            }
            ResolveResult::Multiple(matches) => {
                // Consider it found if there are matches
                found_slides.push((slide_ref.as_str(), matches[0].value.clone()));
            }
        }
    }

    // Report status
    println!(
        "\n{} Skeleton '{}' references {} slides:",
        "i".blue(),
        skeleton.name,
        skeleton.slides.len()
    );

    if !found_slides.is_empty() {
        println!("\n  {} Found ({}):", "Existing".green(), found_slides.len());
        for (ref_name, resolved) in &found_slides {
            if *ref_name == resolved {
                println!("    {} {}", "+".green(), ref_name);
            } else {
                println!("    {} {} -> {}", "+".green(), ref_name, resolved.dimmed());
            }
        }
    }

    if missing_slides.is_empty() {
        println!("\n{} All slides already exist!", "Success!".green().bold());
        return Ok(());
    }

    println!(
        "\n  {} Missing ({}):",
        "Missing".yellow(),
        missing_slides.len()
    );
    for name in &missing_slides {
        println!("    {} {}", "-".red(), name);
    }

    if dry_run {
        println!("\n{} Dry run - no files created", "i".blue());
        println!("  Would create {} slide(s):", missing_slides.len());
        for name in &missing_slides {
            let path = compute_slide_path(&config, name);
            println!("    {} {}", ">".cyan(), path.display());
        }
        return Ok(());
    }

    // Create missing slides
    println!(
        "\n{} Creating {} missing slide(s)...",
        "Creating".green().bold(),
        missing_slides.len()
    );

    let mut created = 0;
    let mut failed = 0;

    for name in &missing_slides {
        let path = compute_slide_path(&config, name);

        match create_slide(&config, name, &path, template) {
            Ok(()) => {
                println!(
                    "  {} Created {}",
                    "+".green(),
                    path.display().to_string().cyan()
                );
                created += 1;
            }
            Err(e) => {
                println!("  {} Failed to create {}: {}", "!".red(), name, e);
                failed += 1;
            }
        }
    }

    println!(
        "\n{} Created {} slide(s){}",
        "Done!".green().bold(),
        created,
        if failed > 0 {
            format!(", {failed} failed").red().to_string()
        } else {
            String::new()
        }
    );

    Ok(())
}

/// Load a skeleton by name with fuzzy matching
fn load_skeleton(config: &Config, name: &str) -> Result<Skeleton> {
    let skeleton_dir = config.skeleton_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    // Find all skeleton files
    let mut skeleton_files: Vec<String> = Vec::new();
    if skeleton_dir.exists() {
        for entry in std::fs::read_dir(&skeleton_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    skeleton_files.push(name.to_string());
                }
            }
        }
    }

    if skeleton_files.is_empty() {
        anyhow::bail!(
            "No skeletons found in {}\nCreate one first.",
            skeleton_dir.display()
        );
    }

    // Resolve the skeleton name
    let skeleton_name = match matcher.resolve(name, &skeleton_files) {
        ResolveResult::Found(result) => result.value,
        ResolveResult::NotFound => {
            println!("{} Skeleton '{}' not found.", "!".red(), name);
            println!("Available skeletons:");
            for s in &skeleton_files {
                println!("  - {}", s.cyan());
            }
            anyhow::bail!("Skeleton not found");
        }
        ResolveResult::Multiple(matches) => {
            // Interactive selection
            let options: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple skeletons match '{name}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;
            matches[selection].value.clone()
        }
    };

    let skeleton_path = skeleton_dir.join(format!("{skeleton_name}.toml"));
    Skeleton::load(&skeleton_path).context(format!("Failed to load skeleton: {skeleton_name}"))
}

/// Compute the path for a new slide based on the reference name
fn compute_slide_path(config: &Config, name: &str) -> PathBuf {
    let slide_dir = config.slide_dir();

    // Handle subdirectory references (e.g., "ai/transformers")
    let name_path = std::path::Path::new(name);

    // Ensure .md extension
    let filename = if name_path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
    {
        name.to_string()
    } else {
        format!("{name}.md")
    };

    slide_dir.join(filename)
}

/// Create a new slide file
fn create_slide(config: &Config, name: &str, path: &PathBuf, template: Option<&str>) -> Result<()> {
    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Get content from template or use default
    let content = if let Some(template_name) = template {
        load_template(config, template_name, name)?
    } else {
        default_slide_template(name)
    };

    // Write the file
    let mut file = std::fs::File::create(path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

/// Generate default slide template content
fn default_slide_template(name: &str) -> String {
    // Extract just the filename without path and extension
    let base_name = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);

    let title = base_name.replace(['_', '-'], " ");

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

/// Load a template file and apply the slide name
fn load_template(config: &Config, template_name: &str, slide_name: &str) -> Result<String> {
    let template_dir = config.template_dir();

    // Try with and without .md extension
    let candidates = [
        template_dir.join(format!("{template_name}.md")),
        template_dir.join(template_name),
    ];

    for path in &candidates {
        if path.exists() {
            let content = std::fs::read_to_string(path)?;
            // Replace placeholder with slide name
            let base_name = std::path::Path::new(slide_name)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(slide_name);
            let title = base_name.replace(['_', '-'], " ");
            return Ok(content
                .replace("{{title}}", &title)
                .replace("{{name}}", slide_name));
        }
    }

    // Template not found, use default with a warning
    println!(
        "  {} Template '{}' not found, using default",
        "!".yellow(),
        template_name
    );

    Ok(default_slide_template(slide_name))
}
