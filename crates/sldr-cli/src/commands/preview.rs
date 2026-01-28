//! Preview command - quickly preview a single slide or slide file

use anyhow::{Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::slide::SlideCollection;
use std::process::Command;
use dialoguer::{theme::ColorfulTheme, Select};

pub fn run(slide: &str, port: Option<String>) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} slide '{}'",
        "Previewing".green().bold(),
        slide.cyan()
    );

    // Find the slide
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());

    let slide_path = match matcher.resolve(slide, &slides.names()) {
        ResolveResult::Found(result) => {
            slides.find(&result.value)
                .map(|s| s.path.clone())
                .context("Slide not found")?
        }
        ResolveResult::NotFound => {
            // Maybe it's a direct path?
            let direct_path = Config::expand_path(slide);
            if direct_path.exists() {
                direct_path
            } else {
                println!("{} Slide '{}' not found.", "!".red(), slide);
                println!("Available slides:");
                for s in slides.slides.iter().take(10) {
                    println!("  - {}", s.relative_path.cyan());
                }
                if slides.slides.len() > 10 {
                    println!("  ... and {} more", slides.slides.len() - 10);
                }
                anyhow::bail!("Slide not found");
            }
        }
        ResolveResult::Multiple(matches) => {
            let options: Vec<String> = matches
                .iter()
                .map(|m| {
                    let slide = slides.find(&m.value);
                    let title = slide
                        .and_then(|s| s.metadata.title.as_deref())
                        .unwrap_or("");
                    if title.is_empty() {
                        m.value.clone()
                    } else {
                        format!("{} - {}", m.value, title)
                    }
                })
                .collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple slides match '{slide}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;

            slides.find(&matches[selection].value)
                .map(|s| s.path.clone())
                .context("Slide not found")?
        }
    };

    // Create a temporary slidev project for preview
    let temp_dir = std::env::temp_dir().join(format!("sldr-preview-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    // Read the slide content
    let slide_content = std::fs::read_to_string(&slide_path)?;

    // Create slides.md with minimal frontmatter
    let preview_content = format!(
        r"---
theme: default
title: Preview
---

{}
",
        slide_content.trim()
    );

    std::fs::write(temp_dir.join("slides.md"), preview_content)?;

    // Create minimal package.json
    let package_json = serde_json::json!({
        "name": "sldr-preview",
        "type": "module",
        "private": true,
        "scripts": {
            "dev": "slidev --open"
        },
        "dependencies": {
            "@slidev/cli": "^52.0.0",
            "@slidev/theme-default": "latest"
        }
    });
    std::fs::write(
        temp_dir.join("package.json"),
        serde_json::to_string_pretty(&package_json)?,
    )?;

    let port = port.unwrap_or_else(|| config.config.slidev_port.clone());

    println!("  {} Installing dependencies...", "i".blue());

    let install_status = Command::new("bun")
        .arg("install")
        .current_dir(&temp_dir)
        .stdout(std::process::Stdio::null())
        .status()
        .context("Failed to run bun install")?;

    if !install_status.success() {
        anyhow::bail!("bun install failed");
    }

    println!(
        "  {} Starting preview on port {}",
        "i".blue(),
        port.yellow()
    );
    println!(
        "  {} Open {} in your browser",
        ">".cyan(),
        format!("http://localhost:{port}").underline()
    );
    println!("  {} Press Ctrl+C to stop\n", "i".dimmed());

    // Start slidev
    let status = Command::new("bun")
        .args(["run", "dev", "--", "--port", &port])
        .current_dir(&temp_dir)
        .status()
        .context("Failed to start slidev")?;

    // Cleanup temp directory
    let _ = std::fs::remove_dir_all(&temp_dir);

    if !status.success() {
        anyhow::bail!("Slidev exited with error");
    }

    Ok(())
}
