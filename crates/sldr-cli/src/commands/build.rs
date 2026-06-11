//! Build command - assemble a presentation from a playlist into self-contained HTML

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::flavor::{Flavor, FlavorCollection};
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::presentation::Playlist;
use sldr_core::slide::SlideCollection;
use sldr_renderer::{HtmlRenderer, RenderConfig};

pub fn run(
    playlist_name: &str,
    flavor: Option<String>,
    lang: Option<String>,
    pdf: bool,
    _pptx: bool,
    output: Option<String>,
    images: &str,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation from playlist '{}'",
        "Building".green().bold(),
        playlist_name.cyan()
    );

    // Load playlist
    let playlist = load_playlist(&config, playlist_name)?;

    // Determine flavor
    let flavor_name = flavor
        .or(playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());

    // Load flavor
    let flavor = load_flavor(&config, &flavor_name)?;
    println!("  {} {}", "Flavor:".dimmed(), flavor.name.yellow());

    // Load slides
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());

    // Resolve slide references
    let mut resolved_slides = Vec::new();
    for slide_ref in &playlist.slides {
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
        anyhow::bail!("No slides resolved. Add slides to your playlist first.");
    }

    // Determine output directory
    let output_dir = output.map_or_else(
        || config.output_dir().join(&playlist.name),
        |o| Config::expand_path(&o),
    );

    // Build HTML presentation using sldr-renderer
    let title = playlist
        .title
        .clone()
        .unwrap_or_else(|| playlist.name.clone());

    let transition = playlist
        .slidev_config
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = playlist
        .slidev_config
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    let image_mode = match images {
        "external" => sldr_renderer::ImageMode::External,
        _ => sldr_renderer::ImageMode::Embed,
    };

    // Language axis: CLI --lang > playlist default_lang > "en" (ADR-0007).
    let default_language = playlist
        .default_lang
        .clone()
        .unwrap_or_else(|| "en".to_string());

    let render_config = RenderConfig {
        title,
        transition,
        aspect_ratio,
        speaker_notes: true,
        image_mode,
        output_dir: Some(output_dir.clone()),
        language: lang,
        default_language,
    };

    let mut renderer = HtmlRenderer::new(render_config).add_flavor(flavor);
    renderer.load_layouts(&config.layout_dir())?;
    renderer.add_slides(&resolved_slides)?;

    // Write to output_dir/index.html
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("index.html");
    renderer.render_to_file(&output_path)?;

    for warning in renderer.warnings() {
        eprintln!("  {} {}", "!".yellow(), warning.yellow());
    }

    println!(
        "\n{} Presentation written to {}",
        "Success!".green().bold(),
        output_path.display().to_string().cyan()
    );

    // Show next steps
    println!("\n{}", "Next steps:".dimmed());
    println!(
        "  Open {} in your browser",
        output_path.display().to_string().underline()
    );
    println!("  Or run: sldr open {}", playlist_name);

    if pdf {
        println!("\n  {} Exporting to PDF...", ">".cyan());
        super::export::run(playlist_name, None, None, "pdf")?;
    }

    Ok(())
}

pub fn load_playlist(config: &Config, name: &str) -> Result<Playlist> {
    let playlist_dir = config.playlist_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    // Find all playlist files
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

    if playlist_files.is_empty() {
        anyhow::bail!(
            "No playlists found in {}\nCreate one with: sldr add <name> <slides>",
            playlist_dir.display()
        );
    }

    // Resolve the playlist name
    let playlist_name = match matcher.resolve(name, &playlist_files) {
        ResolveResult::Found(result) => result.value,
        ResolveResult::NotFound => {
            println!("{} Playlist '{}' not found.", "!".red(), name);
            println!("Available playlists:");
            for s in &playlist_files {
                println!("  - {}", s.cyan());
            }
            anyhow::bail!("Playlist not found");
        }
        ResolveResult::Multiple(matches) => {
            let options: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple playlists match '{name}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;
            matches[selection].value.clone()
        }
    };

    let playlist_path = playlist_dir.join(format!("{playlist_name}.toml"));
    Playlist::load(&playlist_path).context(format!("Failed to load playlist: {playlist_name}"))
}

pub fn load_flavor(config: &Config, name: &str) -> Result<Flavor> {
    let flavor_dir = config.flavor_dir();
    let matcher = SldrMatcher::new(config.matching.clone());

    let collection = FlavorCollection::load_from_dir(&flavor_dir)?;

    if collection.flavors.is_empty() {
        println!("  {} No flavors found, using built-in default", "i".blue());
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

/// Resolve a slide reference, prompting on ambiguity only when attached to a
/// terminal. Non-interactive callers (agents, scripts, CI) get deterministic
/// behavior instead: ambiguity and missing slides fail loudly with the
/// candidate list / searched name in the error — a build must never silently
/// skip or guess a slide.
pub fn resolve_with_interactive(
    matcher: &SldrMatcher,
    slide_ref: &str,
    slides: &SlideCollection,
) -> Result<Option<sldr_core::slide::Slide>> {
    use std::io::IsTerminal;

    match matcher.resolve(slide_ref, &slides.names()) {
        ResolveResult::Found(result) => Ok(slides.find(&result.value).cloned()),
        ResolveResult::NotFound => {
            anyhow::bail!("Slide not found: '{slide_ref}'");
        }
        ResolveResult::Multiple(matches) => {
            if !std::io::stdin().is_terminal() {
                let candidates: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
                anyhow::bail!(
                    "Ambiguous slide reference '{slide_ref}'. Candidates: {}",
                    candidates.join(", ")
                );
            }
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
                Ok(None)
            } else {
                Ok(slides.find(&matches[selection].value).cloned())
            }
        }
    }
}
