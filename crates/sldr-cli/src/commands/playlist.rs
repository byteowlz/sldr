//! Playlist command - playlist management utilities

use super::json_output::JsonResponse;
use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sldr_core::config::Config;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::{Playlist, PlaylistInput};
use sldr_core::slide::{SlideCollection, SlideInput};
use std::io::Read;
use std::path::Path;

/// Output structure for playlist creation (JSON output mode)
#[derive(Serialize)]
struct PlaylistCreateResult {
    name: String,
    path: String,
    slides_count: usize,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    saved_slides: Vec<SavedSlide>,
}

/// Output for a saved slide file
#[derive(Serialize)]
struct SavedSlide {
    name: String,
    path: String,
}

/// Output structure for playlist validation (JSON output mode)
#[derive(Serialize)]
struct ValidationResult {
    name: String,
    valid: bool,
    slides_total: usize,
    slides_found: usize,
    slides_missing: usize,
    found: Vec<ResolvedSlide>,
    missing: Vec<String>,
}

#[derive(Serialize)]
struct ResolvedSlide {
    reference: String,
    resolved: String,
}

/// Output structure for create-from-dir (JSON output mode)
#[derive(Serialize)]
struct FromDirResult {
    name: String,
    path: String,
    slides_count: usize,
    slides: Vec<String>,
}

/// Extended playlist input that optionally includes slide content.
///
/// When `--save-slides` is used, this format allows the caller to provide
/// both playlist metadata and full slide content in a single JSON payload.
/// The slides are saved as individual markdown files in the slides directory,
/// and the playlist TOML references them.
///
/// # Example JSON input with slide_definitions:
/// ```json
/// {
///   "name": "my-presentation",
///   "title": "My Presentation",
///   "slides": ["intro", "main-topic", "conclusion"],
///   "flavor": "default",
///   "slide_definitions": [
///     {
///       "name": "intro",
///       "title": "Introduction",
///       "content": "# Welcome\n\nThis is the intro.",
///       "layout": "cover",
///       "tags": ["intro"]
///     },
///     {
///       "name": "main-topic",
///       "title": "Main Topic",
///       "content": "# Main Topic\n\nKey points here.",
///       "tags": ["content"]
///     }
///   ]
/// }
/// ```
#[derive(Deserialize)]
struct ExtendedPlaylistInput {
    /// Standard playlist fields
    #[serde(flatten)]
    playlist: PlaylistInput,

    /// Optional slide definitions to save as markdown files
    #[serde(default)]
    slide_definitions: Vec<SlideInput>,
}

/// Create a playlist from JSON input (stdin or file)
///
/// When `save_slides` is true, the JSON input can include a `slide_definitions`
/// array with full slide content. Each definition is saved as an individual
/// markdown file in the slides directory.
pub fn create(
    file: Option<&str>,
    dry_run: bool,
    json_output: bool,
    force: bool,
    save_slides: bool,
) -> Result<()> {
    let result = create_inner(file, dry_run, json_output, force, save_slides);

    match result {
        Ok(()) => Ok(()),
        Err(e) if json_output => {
            let cause = e.source().map(|s| s.to_string());
            let response: JsonResponse<()> = JsonResponse::error(e.to_string(), cause);
            response.print();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn create_inner(
    file: Option<&str>,
    dry_run: bool,
    json_output: bool,
    force: bool,
    save_slides: bool,
) -> Result<()> {
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

    // Parse the input - use extended format if save_slides is requested
    let (input, slide_definitions): (PlaylistInput, Vec<SlideInput>) = if save_slides {
        let extended: ExtendedPlaylistInput =
            serde_json::from_str(&json_input).context("Failed to parse JSON input")?;
        (extended.playlist, extended.slide_definitions)
    } else {
        let input: PlaylistInput =
            serde_json::from_str(&json_input).context("Failed to parse JSON input")?;
        (input, Vec::new())
    };

    let playlist_dir = config.playlist_dir();
    let playlist_path = playlist_dir.join(format!("{}.toml", input.name));

    if !json_output {
        println!(
            "{} Creating playlist '{}'{}...",
            "sldr".green().bold(),
            input.name.cyan(),
            if dry_run { " (dry run)" } else { "" }
        );
    }

    if dry_run {
        let mut saved_slides = Vec::new();
        if save_slides && !slide_definitions.is_empty() {
            let slide_dir = config.slide_dir();
            for def in &slide_definitions {
                let path = slide_dir.join(format!("{}.md", def.name));
                if !json_output {
                    println!(
                        "  {} Would save slide: {}",
                        ">".cyan(),
                        path.display().to_string().cyan()
                    );
                }
                saved_slides.push(SavedSlide {
                    name: def.name.clone(),
                    path: path.display().to_string(),
                });
            }
        }
        let result = PlaylistCreateResult {
            name: input.name.clone(),
            path: playlist_path.display().to_string(),
            slides_count: input.slides.len(),
            saved_slides,
        };
        if json_output {
            JsonResponse::success_dry_run(result).print();
        } else {
            println!(
                "  {} Would create: {}",
                ">".cyan(),
                playlist_path.display().to_string().cyan()
            );
            println!("  {} slides: {}", "i".blue(), input.slides.len());
        }
        return Ok(());
    }

    // Check if file already exists (unless --force is set)
    if playlist_path.exists() && !force {
        anyhow::bail!(
            "Playlist '{}' already exists at {} (use --force to overwrite)",
            input.name,
            playlist_path.display()
        );
    }

    // Save slide definitions as individual markdown files if requested
    let mut saved_slides = Vec::new();
    if save_slides && !slide_definitions.is_empty() {
        let slide_dir = config.slide_dir();

        if !json_output {
            println!(
                "\n{} Saving {} slide(s) as markdown...",
                "sldr".green().bold(),
                slide_definitions.len()
            );
        }

        for def in &slide_definitions {
            let effective_dir = def.effective_directory(None);
            let filename = format!("{}.md", def.name);
            let path = if let Some(ref dir) = effective_dir {
                slide_dir.join(dir).join(&filename)
            } else {
                slide_dir.join(&filename)
            };

            // Create parent directories
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Check if file exists (unless --force)
            if path.exists() && !force {
                if !json_output {
                    println!(
                        "  {} Skipped: {} (already exists, use --force to overwrite)",
                        "!".yellow(),
                        path.display()
                    );
                }
                continue;
            }

            let content = def.to_markdown();
            std::fs::write(&path, &content)?;

            if !json_output {
                println!(
                    "  {} Saved: {}",
                    "+".green(),
                    path.display().to_string().cyan()
                );
            }

            saved_slides.push(SavedSlide {
                name: def.name.clone(),
                path: path.display().to_string(),
            });
        }
    }

    // Convert to Playlist and save
    let playlist: Playlist = input.into();

    // Create parent directory if needed
    if let Some(parent) = playlist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    playlist.save(&playlist_path)?;

    let result = PlaylistCreateResult {
        name: playlist.name.clone(),
        path: playlist_path.display().to_string(),
        slides_count: playlist.slides.len(),
        saved_slides,
    };

    if json_output {
        JsonResponse::success(result).print();
    } else {
        println!(
            "\n  {} Created: {}",
            "+".green(),
            playlist_path.display().to_string().cyan()
        );
        println!(
            "\n{} Playlist '{}' created with {} slides",
            "Done!".green().bold(),
            playlist.name,
            playlist.slides.len()
        );
    }

    Ok(())
}

/// Auto-generate a playlist from all slides in a directory
///
/// Scans the given directory (relative to slide_dir or absolute) for .md files
/// and creates a playlist TOML referencing all found slides.
pub fn create_from_dir(
    dir: &str,
    name: Option<&str>,
    dry_run: bool,
    json_output: bool,
    force: bool,
) -> Result<()> {
    let result = create_from_dir_inner(dir, name, dry_run, json_output, force);

    match result {
        Ok(()) => Ok(()),
        Err(e) if json_output => {
            let cause = e.source().map(|s| s.to_string());
            let response: JsonResponse<()> = JsonResponse::error(e.to_string(), cause);
            response.print();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn create_from_dir_inner(
    dir: &str,
    name: Option<&str>,
    dry_run: bool,
    json_output: bool,
    force: bool,
) -> Result<()> {
    let config = Config::load()?;
    let slide_dir = config.slide_dir();

    // Resolve the directory: absolute or relative to slide_dir
    let scan_dir = if Path::new(dir).is_absolute() {
        std::path::PathBuf::from(dir)
    } else {
        slide_dir.join(dir)
    };

    if !scan_dir.exists() {
        anyhow::bail!("Directory '{}' does not exist", scan_dir.display());
    }

    if !scan_dir.is_dir() {
        anyhow::bail!("'{}' is not a directory", scan_dir.display());
    }

    // Derive playlist name from directory name if not provided
    let playlist_name = name.map(String::from).unwrap_or_else(|| {
        scan_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string()
    });

    // Collect slide paths relative to slide_dir
    let collection = SlideCollection::load_from_dir(&scan_dir)?;

    if collection.slides.is_empty() {
        anyhow::bail!("No slides found in '{}'", scan_dir.display());
    }

    // Build slide references relative to the main slide_dir
    let slide_refs: Vec<String> = collection
        .slides
        .iter()
        .map(|s| {
            // Try to make paths relative to slide_dir for portability
            if let Ok(rel) = scan_dir.join(&s.relative_path).strip_prefix(&slide_dir) {
                rel.to_string_lossy().to_string()
            } else {
                // If scan_dir is a subdir of slide_dir, use relative from slide_dir
                let full_path = scan_dir.join(&s.relative_path);
                if let Ok(rel) = full_path.strip_prefix(&slide_dir) {
                    rel.to_string_lossy().to_string()
                } else {
                    s.relative_path.clone()
                }
            }
        })
        .collect();

    let playlist_dir = config.playlist_dir();
    let playlist_path = playlist_dir.join(format!("{playlist_name}.toml"));

    if !json_output {
        println!(
            "{} Generating playlist '{}' from {}{}...",
            "sldr".green().bold(),
            playlist_name.cyan(),
            scan_dir.display().to_string().dimmed(),
            if dry_run { " (dry run)" } else { "" }
        );
        println!("  {} Found {} slide(s)", "i".blue(), slide_refs.len());
        for slide in &slide_refs {
            println!("    {} {}", "+".green(), slide);
        }
    }

    if dry_run {
        let result = FromDirResult {
            name: playlist_name.clone(),
            path: playlist_path.display().to_string(),
            slides_count: slide_refs.len(),
            slides: slide_refs,
        };
        if json_output {
            JsonResponse::success_dry_run(result).print();
        } else {
            println!(
                "\n  {} Would create: {}",
                ">".cyan(),
                playlist_path.display().to_string().cyan()
            );
        }
        return Ok(());
    }

    // Check if file already exists
    if playlist_path.exists() && !force {
        anyhow::bail!(
            "Playlist '{}' already exists at {} (use --force to overwrite)",
            playlist_name,
            playlist_path.display()
        );
    }

    // Build the title from the directory name
    let title = playlist_name.replace(['_', '-'], " ");

    let playlist = Playlist {
        name: playlist_name.clone(),
        title: Some(title),
        description: Some(format!("Auto-generated from {}", scan_dir.display())),
        slides: slide_refs.clone(),
        flavor: None,
        default_lang: None,
        render: Default::default(),
    };

    // Create parent directory if needed
    if let Some(parent) = playlist_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    playlist.save(&playlist_path)?;

    let result = FromDirResult {
        name: playlist_name.clone(),
        path: playlist_path.display().to_string(),
        slides_count: slide_refs.len(),
        slides: slide_refs,
    };

    if json_output {
        JsonResponse::success(result).print();
    } else {
        println!(
            "\n  {} Created: {}",
            "+".green(),
            playlist_path.display().to_string().cyan()
        );
        println!(
            "\n{} Playlist '{}' created with {} slides",
            "Done!".green().bold(),
            playlist_name,
            result.slides_count
        );
    }

    Ok(())
}

/// Validate a playlist - check if all referenced slides exist
///
/// Exit codes:
/// - 0: All slides exist (valid)
/// - 1: Some slides missing (invalid)
/// - 2: Command failed (playlist not found, etc.)
pub fn validate(playlist_name: &str, json_output: bool) -> Result<()> {
    // Wrap the entire operation to handle errors as JSON
    let result = validate_inner(playlist_name, json_output);

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

fn validate_inner(playlist_name: &str, json_output: bool) -> Result<()> {
    let config = Config::load()?;

    // Load the playlist
    let playlist_dir = config.playlist_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    // Find playlist file with fuzzy matching
    let mut playlist_files: Vec<String> = Vec::new();
    if playlist_dir.exists() {
        for entry in std::fs::read_dir(&playlist_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                    playlist_files.push(name.to_string());
                }
            }
        }
    }

    let resolved_name = match matcher.resolve(playlist_name, &playlist_files) {
        ResolveResult::Found(result) => result.value,
        ResolveResult::NotFound => {
            anyhow::bail!("Playlist '{playlist_name}' not found");
        }
        ResolveResult::Multiple(matches) => {
            let names: Vec<_> = matches.iter().map(|m| m.value.as_str()).collect();
            anyhow::bail!("Multiple playlists match '{playlist_name}': {names:?}");
        }
    };

    let playlist_path = playlist_dir.join(format!("{resolved_name}.toml"));
    let playlist = Playlist::load(&playlist_path)?;

    // Load existing slides
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let existing_names = slides.names();

    // Check each slide reference
    let mut found: Vec<ResolvedSlide> = Vec::new();
    let mut missing: Vec<String> = Vec::new();

    for slide_ref in &playlist.slides {
        match matcher.resolve(slide_ref, &existing_names) {
            ResolveResult::Found(result) => {
                found.push(ResolvedSlide {
                    reference: slide_ref.clone(),
                    resolved: result.value,
                });
            }
            ResolveResult::Multiple(matches) => {
                // Consider first match as found
                found.push(ResolvedSlide {
                    reference: slide_ref.clone(),
                    resolved: matches[0].value.clone(),
                });
            }
            ResolveResult::NotFound => {
                missing.push(slide_ref.clone());
            }
        }
    }

    let is_valid = missing.is_empty();

    if json_output {
        let result = ValidationResult {
            name: playlist.name.clone(),
            valid: is_valid,
            slides_total: playlist.slides.len(),
            slides_found: found.len(),
            slides_missing: missing.len(),
            found,
            missing,
        };
        JsonResponse::success(result).print();
    } else {
        println!(
            "{} Validating playlist '{}'...",
            "sldr".green().bold(),
            playlist.name.cyan()
        );

        println!("\n  {} Total slides: {}", "i".blue(), playlist.slides.len());
        println!("  {} Found: {}", "+".green(), found.len());

        if !missing.is_empty() {
            println!("  {} Missing: {}", "-".red(), missing.len());
            println!("\n  Missing slides:");
            for name in &missing {
                println!("    {} {}", "-".red(), name);
            }
        }

        if is_valid {
            println!(
                "\n{} Playlist '{}' is valid - all slides exist",
                "Valid!".green().bold(),
                playlist.name
            );
        } else {
            println!(
                "\n{} Playlist '{}' has {} missing slide(s)",
                "Invalid!".red().bold(),
                playlist.name,
                missing.len()
            );
            println!(
                "\n  {} Run 'sldr slides derive {}' to create missing slides",
                "Tip:".cyan(),
                playlist.name
            );
        }
    }

    Ok(())
}
