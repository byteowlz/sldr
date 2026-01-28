//! Open command - start slidev server for a presentation

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use std::process::Command;

pub fn run(presentation: &str, port: Option<String>, rebuild: bool) -> Result<()> {
    let config = Config::load()?;

    // Find the presentation with fuzzy matching
    let output_dir = resolve_presentation(&config, presentation)?;

    let presentation_name = output_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(presentation);

    println!(
        "{} presentation '{}'",
        "Opening".green().bold(),
        presentation_name.cyan()
    );

    let slides_path = output_dir.join("slides.md");
    let package_json = output_dir.join("package.json");

    if !slides_path.exists() {
        anyhow::bail!(
            "slides.md not found in {}. Try rebuilding the presentation.",
            output_dir.display()
        );
    }

    // Check if rebuild is needed or requested
    if rebuild {
        println!("  {} Rebuilding presentation...", "i".blue());
        // Call build command
        super::build::run(
            presentation_name,
            None,
            false,
            false,
            Some(output_dir.to_string_lossy().to_string()),
        )?;
    }

    // Install dependencies if needed
    let node_modules = output_dir.join("node_modules");
    if !node_modules.exists() && package_json.exists() {
        println!("  {} Installing dependencies...", "i".blue());
        let status = Command::new("bun")
            .arg("install")
            .current_dir(&output_dir)
            .status()
            .context("Failed to run bun install. Is bun installed?")?;

        if !status.success() {
            anyhow::bail!("bun install failed");
        }
    }

    let port = port.unwrap_or_else(|| config.config.slidev_port.clone());

    println!(
        "  {} Starting slidev on port {}",
        "i".blue(),
        port.yellow()
    );
    println!(
        "  {} Open {} in your browser",
        ">".cyan(),
        format!("http://localhost:{port}").underline()
    );
    println!("  {} Press Ctrl+C to stop\n", "i".dimmed());

    // Start slidev using bun
    let status = Command::new("bun")
        .args(["run", "dev", "--", "--port", &port])
        .current_dir(&output_dir)
        .status()
        .context("Failed to start slidev")?;

    if !status.success() {
        anyhow::bail!("Slidev exited with error");
    }

    Ok(())
}

fn resolve_presentation(config: &Config, name: &str) -> Result<std::path::PathBuf> {
    let output_dir = config.output_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    // Get list of built presentations
    let mut presentations: Vec<String> = Vec::new();
    if output_dir.exists() {
        for entry in std::fs::read_dir(&output_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                // Check if it has slides.md
                if entry.path().join("slides.md").exists() {
                    if let Some(name) = entry.file_name().to_str() {
                        presentations.push(name.to_string());
                    }
                }
            }
        }
    }

    if presentations.is_empty() {
        anyhow::bail!(
            "No presentations found in {}\nBuild one first with: sldr build <skeleton>",
            output_dir.display()
        );
    }

    match matcher.resolve(name, &presentations) {
        ResolveResult::Found(result) => Ok(output_dir.join(&result.value)),
        ResolveResult::NotFound => {
            println!("{} Presentation '{}' not found.", "!".red(), name);
            println!("Available presentations:");
            for p in &presentations {
                println!("  - {}", p.cyan());
            }
            anyhow::bail!("Presentation not found");
        }
        ResolveResult::Multiple(matches) => {
            let options: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "Multiple presentations match '{name}'. Select one:"
                ))
                .items(&options)
                .default(0)
                .interact()?;
            Ok(output_dir.join(&matches[selection].value))
        }
    }
}
