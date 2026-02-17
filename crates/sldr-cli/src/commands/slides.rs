//! Slides command - slide management utilities

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use serde::Serialize;
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Skeleton;
use sldr_core::slide::{SlideCollection, SlideInputBatch};
use std::io::{Read, Write};
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

/// Output structure for created slides (JSON output mode)
#[derive(Serialize)]
struct SlidesCreateResult {
    /// Number of slides successfully created (or would be created in dry-run)
    created_count: usize,
    /// Number of slides that failed
    failed_count: usize,
    /// Total slides in input
    total: usize,
    /// Details of created slides
    created: Vec<CreatedSlide>,
    /// Details of failed slides
    failed: Vec<FailedSlide>,
}

#[derive(Serialize)]
struct CreatedSlide {
    name: String,
    path: String,
}

#[derive(Serialize)]
struct FailedSlide {
    name: String,
    error: String,
}

/// Create slides from JSON input (stdin or file)
///
/// This is the agent-friendly batch slide creation command.
/// Input format: `SlideInputBatch` JSON (see schema)
///
/// Exit codes:
/// - 0: All slides created successfully
/// - 1: Some slides failed (partial success)
/// - 2: Command failed (invalid input, IO error, etc.)
///
/// # Example JSON input:
/// ```json
/// {
///   "directory": "my-topic",
///   "slides": [
///     {
///       "name": "intro",
///       "title": "Introduction",
///       "content": "# Introduction\n\nWelcome to my presentation.",
///       "layout": "cover",
///       "tags": ["intro", "overview"]
///     }
///   ]
/// }
/// ```
pub fn create(file: Option<&str>, dry_run: bool, json_output: bool, force: bool) -> Result<()> {
    use super::json_output::JsonResponse;

    // Wrap the entire operation to handle errors as JSON
    let result = create_inner(file, dry_run, json_output, force);

    match result {
        Ok(()) => Ok(()),
        Err(e) if json_output => {
            // Output error as JSON
            let cause = e.source().map(|s| s.to_string());
            let response: JsonResponse<()> = JsonResponse::error(e.to_string(), cause);
            response.print();
            Ok(()) // Don't propagate error - we've already output it as JSON
        }
        Err(e) => Err(e),
    }
}

fn create_inner(file: Option<&str>, dry_run: bool, json_output: bool, force: bool) -> Result<()> {
    use super::json_output::JsonResponse;

    let config = Config::load()?;

    // Read JSON input
    let json_input = if let Some(path) = file {
        std::fs::read_to_string(path).context(format!("Failed to read file: {path}"))?
    } else {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read from stdin")?;
        buffer
    };

    // Parse the input
    let batch: SlideInputBatch =
        serde_json::from_str(&json_input).context("Failed to parse JSON input")?;

    if !json_output {
        println!(
            "{} Creating {} slide(s){}...",
            "sldr".green().bold(),
            batch.slides.len(),
            if dry_run { " (dry run)" } else { "" }
        );
    }

    let slide_dir = config.slide_dir();
    let mut created: Vec<CreatedSlide> = Vec::new();
    let mut failed: Vec<FailedSlide> = Vec::new();

    for slide_input in &batch.slides {
        // Determine the target directory
        let effective_dir = slide_input.effective_directory(batch.directory.as_deref());

        // Build the full path
        let filename = format!("{}.md", slide_input.name);
        let path = if let Some(ref dir) = effective_dir {
            slide_dir.join(dir).join(&filename)
        } else {
            slide_dir.join(&filename)
        };

        if dry_run {
            if !json_output {
                println!(
                    "  {} Would create: {}",
                    ">".cyan(),
                    path.display().to_string().cyan()
                );
            }
            created.push(CreatedSlide {
                name: slide_input.name.clone(),
                path: path.display().to_string(),
            });
            continue;
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                failed.push(FailedSlide {
                    name: slide_input.name.clone(),
                    error: format!("Failed to create directory: {e}"),
                });
                continue;
            }
        }

        // Check if file already exists (unless --force is set)
        if path.exists() && !force {
            failed.push(FailedSlide {
                name: slide_input.name.clone(),
                error: "File already exists (use --force to overwrite)".to_string(),
            });
            continue;
        }

        // Generate content and write file
        let content = slide_input.to_markdown();
        match std::fs::write(&path, &content) {
            Ok(()) => {
                if !json_output {
                    let action = if path.exists() && force {
                        "Overwrote"
                    } else {
                        "Created"
                    };
                    println!(
                        "  {} {}: {}",
                        "+".green(),
                        action,
                        path.display().to_string().cyan()
                    );
                }
                created.push(CreatedSlide {
                    name: slide_input.name.clone(),
                    path: path.display().to_string(),
                });
            }
            Err(e) => {
                if !json_output {
                    println!("  {} Failed: {} - {}", "!".red(), slide_input.name, e);
                }
                failed.push(FailedSlide {
                    name: slide_input.name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Output results
    if json_output {
        let result = SlidesCreateResult {
            created_count: created.len(),
            failed_count: failed.len(),
            total: batch.slides.len(),
            created,
            failed,
        };
        if dry_run {
            JsonResponse::success_dry_run(result).print();
        } else {
            JsonResponse::success(result).print();
        }
    } else if dry_run {
        println!(
            "\n{} Dry run complete - would create {} slide(s)",
            "i".blue(),
            batch.slides.len()
        );
    } else {
        let success_count = created.len();
        let fail_count = failed.len();
        println!(
            "\n{} Created {} slide(s){}",
            "Done!".green().bold(),
            success_count,
            if fail_count > 0 {
                format!(", {fail_count} failed").red().to_string()
            } else {
                String::new()
            }
        );
    }

    Ok(())
}
