//! Open command - open a built presentation in the browser

use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use std::process::Command;

pub fn run(presentation: &str, _port: Option<String>, rebuild: bool) -> Result<()> {
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

    // Check for index.html (new renderer) or slides.md (legacy slidev)
    let index_path = output_dir.join("index.html");
    let slides_path = output_dir.join("slides.md");

    if !index_path.exists() && !slides_path.exists() {
        anyhow::bail!(
            "No presentation found in {}. Build one first with: sldr build <skeleton>",
            output_dir.display()
        );
    }

    // Rebuild if requested
    if rebuild {
        println!("  {} Rebuilding presentation...", "i".blue());
        super::build::run(
            presentation_name,
            None,
            false,
            false,
            Some(output_dir.to_string_lossy().to_string()),
        )?;
    }

    if index_path.exists() {
        // New HTML renderer output - open directly in browser
        println!(
            "  {} Opening {} in browser",
            ">".cyan(),
            index_path.display()
        );

        open_in_browser(&index_path.to_string_lossy())?;
    } else {
        // Legacy slidev project - inform user to rebuild
        println!(
            "  {} Found legacy slidev project. Rebuilding as HTML...",
            "i".blue()
        );
        super::build::run(
            presentation_name,
            None,
            false,
            false,
            Some(output_dir.to_string_lossy().to_string()),
        )?;

        if index_path.exists() {
            open_in_browser(&index_path.to_string_lossy())?;
        }
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
                // Check for index.html (new) or slides.md (legacy)
                let has_index = entry.path().join("index.html").exists();
                let has_slides = entry.path().join("slides.md").exists();
                if has_index || has_slides {
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

/// Open a file or URL in the default browser
fn open_in_browser(path: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try xdg-open first, then common browsers
        let result = Command::new("xdg-open").arg(path).spawn();
        if result.is_err() {
            // Fallback: try common browsers
            for browser in &["firefox", "chromium", "google-chrome", "brave"] {
                if Command::new(browser).arg(path).spawn().is_ok() {
                    return Ok(());
                }
            }
            println!(
                "  {} Could not open browser. Open manually: {}",
                "!".yellow(),
                path
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", path]).spawn()?;
    }

    Ok(())
}
