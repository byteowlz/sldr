//! Remove command - remove slides from a presentation skeleton

use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Skeleton;

pub fn run(presentation: &str, slides: Option<&String>, interactive: bool) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} slides from '{}'",
        "Removing".red().bold(),
        presentation.cyan()
    );

    // Find the skeleton
    let skeleton_dir = config.skeleton_dir();
    let skeleton_path = skeleton_dir.join(format!("{presentation}.toml"));

    if !skeleton_path.exists() {
        anyhow::bail!(
            "Skeleton '{}' not found at {}",
            presentation,
            skeleton_path.display()
        );
    }

    let mut skeleton = Skeleton::load(&skeleton_path)?;

    if skeleton.slides.is_empty() {
        println!("  {} Skeleton has no slides", "i".blue());
        return Ok(());
    }

    let slides_to_remove: Vec<usize> = if let Some(slides_arg) = slides {
        if interactive {
            // Interactive mode with slides hint - let user select slides to remove
            select_slides_interactively(&skeleton)?
        } else {
            // Parse slides argument
            let slide_refs: Vec<&str> = slides_arg
                .split([',', ' '])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();

            let matcher = SldrMatcher::new(config.matching.clone());
            let mut indices = Vec::new();

            for slide_ref in slide_refs {
                // Try to match against skeleton slides
                match matcher.resolve(slide_ref, &skeleton.slides) {
                    ResolveResult::Found(result) => {
                        if let Some(idx) = skeleton.slides.iter().position(|s| s == &result.value) {
                            if !indices.contains(&idx) {
                                indices.push(idx);
                            }
                        }
                    }
                    ResolveResult::NotFound => {
                        // Try as numeric index
                        if let Ok(idx) = slide_ref.parse::<usize>() {
                            if idx < skeleton.slides.len() {
                                if !indices.contains(&idx) {
                                    indices.push(idx);
                                }
                            } else {
                                println!("  {} Index {idx} out of range", "!".yellow());
                            }
                        } else {
                            println!(
                                "  {} Slide '{slide_ref}' not found in skeleton",
                                "!".yellow()
                            );
                        }
                    }
                    ResolveResult::Multiple(matches) => {
                        println!(
                            "  {} Multiple matches for '{slide_ref}': {:?}",
                            "!".yellow(),
                            matches.iter().map(|m| &m.value).collect::<Vec<_>>()
                        );
                    }
                }
            }
            indices
        }
    } else {
        // No slides argument - use interactive mode
        select_slides_interactively(&skeleton)?
    };

    if slides_to_remove.is_empty() {
        println!("  {} No slides to remove", "i".blue());
        return Ok(());
    }

    // Show what will be removed
    println!("\nSlides to remove:");
    for &idx in &slides_to_remove {
        println!("  {} {}", "-".red(), skeleton.slides[idx].cyan());
    }

    // Confirm
    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Remove {} slide(s)?", slides_to_remove.len()))
        .default(false)
        .interact()?;

    if !confirm {
        println!("  {} Cancelled", "i".blue());
        return Ok(());
    }

    // Remove slides (in reverse order to maintain indices)
    let mut sorted_indices = slides_to_remove;
    sorted_indices.sort_by(|a, b| b.cmp(a));
    for idx in sorted_indices {
        skeleton.slides.remove(idx);
    }

    // Save
    skeleton.save(&skeleton_path)?;

    println!(
        "\n{} Removed slides from '{}'",
        "Done!".green().bold(),
        presentation.cyan()
    );

    Ok(())
}

/// Helper to interactively select slides for removal
fn select_slides_interactively(skeleton: &Skeleton) -> Result<Vec<usize>> {
    let items: Vec<&str> = skeleton.slides.iter().map(String::as_str).collect();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select slides to remove (space to select, enter to confirm)")
        .items(&items)
        .interact()?;

    Ok(selections)
}
