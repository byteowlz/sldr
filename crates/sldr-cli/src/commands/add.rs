//! Add command - add slides to an existing presentation

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Skeleton;
use sldr_core::slide::SlideCollection;


pub fn run(presentation: &str, slides: &str, position: Option<usize>) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} slides to '{}'",
        "Adding".green().bold(),
        presentation.cyan()
    );

    // Parse slide list (comma or space separated)
    let slide_names: Vec<&str> = slides
        .split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if slide_names.is_empty() {
        anyhow::bail!("No slides specified");
    }

    // Load available slides
    let available_slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());

    // Find the skeleton file
    let skeleton_dir = config.skeleton_dir();
    let skeleton_path = skeleton_dir.join(format!("{presentation}.toml"));

    let mut skeleton = if skeleton_path.exists() {
        Skeleton::load(&skeleton_path)?
    } else {
        // Create a new skeleton
        println!(
            "  {} Creating new skeleton '{}'",
            "i".blue(),
            presentation.cyan()
        );
        Skeleton {
            name: presentation.to_string(),
            title: None,
            description: None,
            slides: Vec::new(),
            flavor: None,
            slidev_config: sldr_core::presentation::SlidevConfig::default(),
        }
    };

    // Resolve and add slides
    let mut added = 0;
    for slide_name in slide_names {
        match matcher.resolve(slide_name, &available_slides.names()) {
            ResolveResult::Found(result) => {
                let slide_ref = result.value.clone();

                // Check if already in skeleton
                if skeleton.slides.contains(&slide_ref) {
                    println!(
                        "  {} '{}' already in presentation",
                        "~".yellow(),
                        slide_ref
                    );
                    continue;
                }

                // Add at position or append
                if let Some(pos) = position {
                    let insert_at = pos.min(skeleton.slides.len());
                    skeleton.slides.insert(insert_at + added, slide_ref.clone());
                } else {
                    skeleton.slides.push(slide_ref.clone());
                }

                println!("  {} {}", "+".green(), slide_ref);
                added += 1;
            }
            ResolveResult::NotFound => {
                println!(
                    "  {} Slide not found: '{}'",
                    "!".red(),
                    slide_name.yellow()
                );
            }
            ResolveResult::Multiple(matches) => {
                println!(
                    "  {} Multiple matches for '{}': {}",
                    "?".yellow(),
                    slide_name,
                    matches
                        .iter()
                        .map(|m| m.value.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
    }

    if added > 0 {
        // Ensure skeleton directory exists
        std::fs::create_dir_all(&skeleton_dir)?;
        skeleton.save(&skeleton_path)?;

        println!(
            "\n{} Added {} slide(s) to '{}'",
            "Done!".green().bold(),
            added,
            presentation.cyan()
        );
    } else {
        println!("\n{}", "No slides were added".yellow());
    }

    Ok(())
}
