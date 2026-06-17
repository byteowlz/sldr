//! Remove command - remove slides from a presentation playlist

use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Playlist;

pub fn run(presentation: &str, slides: Option<&String>, interactive: bool) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} slides from '{}'",
        "Removing".red().bold(),
        presentation.cyan()
    );

    // Find the playlist
    let playlist_dir = config.playlist_dir();
    let playlist_path = playlist_dir.join(format!("{presentation}.toml"));

    if !playlist_path.exists() {
        anyhow::bail!(
            "Playlist '{}' not found at {}",
            presentation,
            playlist_path.display()
        );
    }

    let mut playlist = Playlist::load(&playlist_path)?;

    if playlist.slides.is_empty() {
        println!("  {} Playlist has no slides", "i".blue());
        return Ok(());
    }

    let slides_to_remove: Vec<usize> = if let Some(slides_arg) = slides {
        if interactive {
            // Interactive mode with slides hint - let user select slides to remove
            select_slides_interactively(&playlist)?
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
                // Try to match against playlist slides
                match matcher.resolve(slide_ref, &playlist.slides) {
                    ResolveResult::Found(result) => {
                        if let Some(idx) = playlist.slides.iter().position(|s| s == &result.value) {
                            if !indices.contains(&idx) {
                                indices.push(idx);
                            }
                        }
                    }
                    ResolveResult::NotFound => {
                        // Try as numeric index
                        if let Ok(idx) = slide_ref.parse::<usize>() {
                            if idx < playlist.slides.len() {
                                if !indices.contains(&idx) {
                                    indices.push(idx);
                                }
                            } else {
                                anyhow::bail!(
                                    "Index {idx} out of range (playlist has {} slides)",
                                    playlist.slides.len()
                                );
                            }
                        } else {
                            anyhow::bail!("Slide '{slide_ref}' not found in playlist");
                        }
                    }
                    ResolveResult::Multiple(matches) => {
                        anyhow::bail!(
                            "Ambiguous slide reference '{slide_ref}'. Candidates: {}",
                            matches
                                .iter()
                                .map(|m| m.value.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                }
            }
            indices
        }
    } else {
        // No slides argument - use interactive mode (requires a terminal)
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            anyhow::bail!(
                "No slides specified and not attached to a terminal. \
                 Pass the slides to remove as an argument."
            );
        }
        select_slides_interactively(&playlist)?
    };

    if slides_to_remove.is_empty() {
        println!("  {} No slides to remove", "i".blue());
        return Ok(());
    }

    // Show what will be removed
    println!("\nSlides to remove:");
    for &idx in &slides_to_remove {
        println!("  {} {}", "-".red(), playlist.slides[idx].cyan());
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
        playlist.slides.remove(idx);
    }

    // Save
    playlist.save(&playlist_path)?;

    println!(
        "\n{} Removed slides from '{}'",
        "Done!".green().bold(),
        presentation.cyan()
    );

    Ok(())
}

/// Helper to interactively select slides for removal
fn select_slides_interactively(playlist: &Playlist) -> Result<Vec<usize>> {
    let items: Vec<&str> = playlist.slides.iter().map(String::as_str).collect();

    let selections = MultiSelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select slides to remove (space to select, enter to confirm)")
        .items(&items)
        .interact()?;

    Ok(selections)
}
