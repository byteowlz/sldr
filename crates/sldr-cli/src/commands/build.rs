//! Build command - assemble a presentation from a skeleton

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::flavor::{Flavor, FlavorCollection};
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::{PresentationBuilder, Skeleton};
use sldr_core::slide::SlideCollection;
use std::process::Command;
use tracing::info;

pub fn run(
    skeleton_name: &str,
    flavor: Option<String>,
    pdf: bool,
    pptx: bool,
    output: Option<String>,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation from skeleton '{}'",
        "Building".green().bold(),
        skeleton_name.cyan()
    );

    // Load skeleton
    let skeleton = load_skeleton(&config, skeleton_name)?;
    info!("Loaded skeleton with {} slides", skeleton.slides.len());

    // Determine flavor
    let flavor_name = flavor
        .or(skeleton.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());

    // Load flavor
    let flavor = load_flavor(&config, &flavor_name)?;
    println!("  {} {}", "Flavor:".dimmed(), flavor.name.yellow());

    // Load slides
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());

    // Resolve slide references
    let mut resolved_slides = Vec::new();
    for slide_ref in &skeleton.slides {
        match resolve_with_interactive(&matcher, slide_ref, &slides)? {
            Some(slide) => {
                println!("  {} {}", "+".green(), slide.name);
                resolved_slides.push(slide);
            }
            None => {
                println!("  {} Skipped: {}", "~".yellow(), slide_ref);
            }
        }
    }

    if resolved_slides.is_empty() {
        anyhow::bail!("No slides resolved. Add slides to your skeleton first.");
    }

    // Determine output directory
    let output_dir = output.map_or_else(|| config.output_dir().join(&skeleton.name), |o| Config::expand_path(&o));

    // Build presentation
    let title = skeleton.title.clone().unwrap_or_else(|| skeleton.name.clone());
    let presentation = PresentationBuilder::new(&skeleton.name)
        .title(title)
        .flavor(flavor)
        .slidev_config(skeleton.slidev_config.clone())
        .output_dir(&output_dir)
        .add_slides(resolved_slides)
        .build();

    presentation.write()?;

    println!(
        "\n{} Presentation written to {}",
        "Success!".green().bold(),
        output_dir.display().to_string().cyan()
    );

    // Show next steps
    println!("\n{}", "Next steps:".dimmed());
    println!(
        "  cd {} && bun install && bun dev",
        output_dir.display()
    );

    // Export if requested
    if pdf || pptx {
        export_presentation(&output_dir, pdf, pptx)?;
    }

    Ok(())
}

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
            "No skeletons found in {}\nCreate one with: sldr add <name> <slides>",
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

fn load_flavor(config: &Config, name: &str) -> Result<Flavor> {
    let flavor_dir = config.flavor_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    let collection = FlavorCollection::load_from_dir(&flavor_dir)?;

    if collection.flavors.is_empty() {
        // Return default flavor if none exist
        println!(
            "  {} No flavors found, using built-in default",
            "i".blue()
        );
        return Ok(Flavor::default());
    }

    let flavor_names = collection.names();

    match matcher.resolve(name, &flavor_names) {
        ResolveResult::Found(result) => {
            let flavor_path = flavor_dir.join(&result.value);
            Ok(Flavor::load(&flavor_path)?)
        }
        ResolveResult::NotFound => {
            println!(
                "  {} Flavor '{}' not found, using default",
                "!".yellow(),
                name
            );
            Ok(Flavor::default())
        }
        ResolveResult::Multiple(matches) => {
            let options: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple flavors match '{name}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;
            let flavor_path = flavor_dir.join(&matches[selection].value);
            Ok(Flavor::load(&flavor_path)?)
        }
    }
}

fn resolve_with_interactive(
    matcher: &SldrMatcher,
    slide_ref: &str,
    slides: &SlideCollection,
) -> Result<Option<sldr_core::slide::Slide>> {
    match matcher.resolve(slide_ref, &slides.names()) {
        ResolveResult::Found(result) => {
            Ok(slides.find(&result.value).cloned())
        }
        ResolveResult::NotFound => {
            println!(
                "  {} Slide not found: '{}'",
                "!".red(),
                slide_ref.yellow()
            );
            Ok(None)
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

            let mut items: Vec<&str> = options.iter().map(std::string::String::as_str).collect();
            items.push("(skip)");

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple slides match '{slide_ref}'. Select one:"))
                .items(&items)
                .default(0)
                .interact()?;

            if selection == items.len() - 1 {
                Ok(None) // Skip
            } else {
                Ok(slides.find(&matches[selection].value).cloned())
            }
        }
    }
}

fn export_presentation(output_dir: &std::path::Path, pdf: bool, pptx: bool) -> Result<()> {
    println!("\n{}", "Exporting...".green().bold());

    // Check if node_modules exists
    let node_modules = output_dir.join("node_modules");
    if !node_modules.exists() {
        println!("  {} Installing dependencies...", "i".blue());
        let status = Command::new("bun")
            .arg("install")
            .current_dir(output_dir)
            .status()
            .context("Failed to run bun install. Is bun installed?")?;

        if !status.success() {
            anyhow::bail!("bun install failed");
        }
    }

    if pdf {
        println!("  {} Exporting to PDF...", ">".cyan());
        let status = Command::new("bun")
            .args(["run", "export-pdf"])
            .current_dir(output_dir)
            .status()
            .context("Failed to export PDF")?;

        if status.success() {
            println!(
                "  {} PDF exported to {}/slides-export.pdf",
                "+".green(),
                output_dir.display()
            );
        } else {
            println!("  {} PDF export failed", "!".red());
        }
    }

    if pptx {
        println!("  {} Exporting to PPTX...", ">".cyan());
        let status = Command::new("bun")
            .args(["run", "export-pptx"])
            .current_dir(output_dir)
            .status()
            .context("Failed to export PPTX")?;

        if status.success() {
            println!(
                "  {} PPTX exported to {}/slides-export.pptx",
                "+".green(),
                output_dir.display()
            );
        } else {
            println!("  {} PPTX export failed", "!".red());
        }
    }

    Ok(())
}
