//! List command - show available slides, presentations, skeletons, or flavors

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::flavor::FlavorCollection;
use sldr_core::slide::SlideCollection;

pub fn run(what: &str, long: bool) -> Result<()> {
    let config = Config::load()?;

    match what.to_lowercase().as_str() {
        "slides" | "slide" | "s" => list_slides(&config, long),
        "presentations" | "presentation" | "p" => list_presentations(&config, long),
        "skeletons" | "skeleton" | "sk" => list_skeletons(&config, long),
        "flavors" | "flavor" | "f" => list_flavors(&config, long),
        _ => {
            println!(
                "{}: Unknown type '{}'. Use: slides, presentations, skeletons, or flavors",
                "Error".red(),
                what
            );
            Ok(())
        }
    }
}

fn list_slides(config: &Config, long: bool) -> Result<()> {
    let slide_dir = config.slide_dir();
    println!(
        "{} ({})",
        "Slides".green().bold(),
        slide_dir.display().to_string().dimmed()
    );

    let collection = SlideCollection::load_from_dir(&slide_dir)?;

    if collection.slides.is_empty() {
        println!("  {}", "No slides found".dimmed());
        return Ok(());
    }

    for slide in &collection.slides {
        if long {
            let title = slide
                .metadata
                .title
                .as_deref()
                .unwrap_or("(no title)");
            let tags = if slide.metadata.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", slide.metadata.tags.join(", "))
            };
            println!(
                "  {} - {}{}",
                slide.relative_path.cyan(),
                title,
                tags.dimmed()
            );
        } else {
            println!("  {}", slide.relative_path);
        }
    }

    println!("\n  {} slide(s)", collection.slides.len().to_string().bold());
    Ok(())
}

fn list_presentations(config: &Config, long: bool) -> Result<()> {
    let output_dir = config.output_dir();
    println!(
        "{} ({})",
        "Presentations".green().bold(),
        output_dir.display().to_string().dimmed()
    );

    if !output_dir.exists() {
        println!("  {}", "No presentations found".dimmed());
        return Ok(());
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&output_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            if long {
                let slides_path = entry.path().join("slides.md");
                let status = if slides_path.exists() {
                    "ready".green()
                } else {
                    "incomplete".yellow()
                };
                println!("  {} [{}]", name_str.cyan(), status);
            } else {
                println!("  {name_str}");
            }
            count += 1;
        }
    }

    if count == 0 {
        println!("  {}", "No presentations found".dimmed());
    } else {
        println!("\n  {} presentation(s)", count.to_string().bold());
    }
    Ok(())
}

fn list_skeletons(config: &Config, long: bool) -> Result<()> {
    let skeleton_dir = config.skeleton_dir();
    println!(
        "{} ({})",
        "Skeletons".green().bold(),
        skeleton_dir.display().to_string().dimmed()
    );

    if !skeleton_dir.exists() {
        println!("  {}", "No skeletons found".dimmed());
        return Ok(());
    }

    let mut count = 0;
    for entry in std::fs::read_dir(&skeleton_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().is_some_and(|ext| ext == "toml") {
            let Some(stem) = path.file_stem() else { continue };
            let name = stem.to_string_lossy();

            if long {
                // Load and show skeleton details
                match sldr_core::presentation::Skeleton::load(&path) {
                    Ok(skeleton) => {
                        let slides_count = skeleton.slides.len();
                        let flavor = skeleton.flavor.as_deref().unwrap_or("default");
                        println!(
                            "  {} - {} slides, flavor: {}",
                            name.cyan(),
                            slides_count,
                            flavor.yellow()
                        );
                    }
                    Err(_) => {
                        println!("  {} [{}]", name.cyan(), "invalid".red());
                    }
                }
            } else {
                println!("  {name}");
            }
            count += 1;
        }
    }

    if count == 0 {
        println!("  {}", "No skeletons found".dimmed());
    } else {
        println!("\n  {} skeleton(s)", count.to_string().bold());
    }
    Ok(())
}

fn list_flavors(config: &Config, long: bool) -> Result<()> {
    let flavor_dir = config.flavor_dir();
    println!(
        "{} ({})",
        "Flavors".green().bold(),
        flavor_dir.display().to_string().dimmed()
    );

    let collection = FlavorCollection::load_from_dir(&flavor_dir)?;

    if collection.flavors.is_empty() {
        println!("  {}", "No flavors found".dimmed());
        println!(
            "  {} Run 'sldr init' to create default flavors",
            "i".blue()
        );
        return Ok(());
    }

    for flavor in &collection.flavors {
        if long {
            let desc = flavor
                .description
                .as_deref()
                .unwrap_or("(no description)");
            println!("  {} - {}", flavor.name.cyan(), desc.dimmed());
        } else {
            println!("  {}", flavor.name);
        }
    }

    println!("\n  {} flavor(s)", collection.flavors.len().to_string().bold());
    Ok(())
}
