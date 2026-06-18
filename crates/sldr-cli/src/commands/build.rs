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
    single_file: bool,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} presentation from playlist '{}'",
        "Building".green().bold(),
        playlist_name.cyan()
    );

    // Load playlist
    let playlist = load_playlist(&config, playlist_name)?;

    // Flavor axis (ADR-0007): CLI --flavor (comma list = embed set for
    // the runtime switcher) > playlist default > config default. The
    // first flavor is active; the rest embed as toggleable style blocks.
    let flavor_arg = flavor
        .or(playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor_names: Vec<&str> = flavor_arg
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut flavors = Vec::new();
    for name in &flavor_names {
        flavors.push(load_flavor(&config, name)?);
    }
    println!(
        "  {} {}",
        "Flavor:".dimmed(),
        flavors
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
            .yellow()
    );

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
        .render
        .transition
        .clone()
        .unwrap_or_else(|| "fade".to_string());

    let aspect_ratio = playlist
        .render
        .aspect_ratio
        .clone()
        .unwrap_or_else(|| "16/9".to_string());

    // Artifact tier (ADR-0006): directory with media siblings by default
    // (browser-native — streams and seeks over file://); --single-file
    // inlines everything for the universal one-file handoff.
    let image_mode = if single_file {
        sldr_renderer::ImageMode::Embed
    } else {
        sldr_renderer::ImageMode::External
    };

    // Language axis: CLI --lang (comma list = embed set, first active) >
    // playlist default_lang > "en" (ADR-0007).
    let default_language = playlist
        .default_lang
        .clone()
        .unwrap_or_else(|| "en".to_string());
    let languages: Vec<String> = lang
        .as_deref()
        .map(|l| {
            l.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let render_config = RenderConfig {
        title,
        transition,
        aspect_ratio,
        speaker_notes: true,
        image_mode,
        output_dir: Some(output_dir.clone()),
        languages,
        default_language,
    };

    let mut renderer = HtmlRenderer::new(render_config).add_flavors(flavors);
    for dir in config.layout_dirs() {
        renderer.load_layouts(&dir)?;
    }
    renderer.add_slides(&resolved_slides)?;

    // Write to output_dir/index.html
    std::fs::create_dir_all(&output_dir)?;
    let output_path = output_dir.join("index.html");
    renderer.render_to_file(&output_path)?;

    for warning in renderer.warnings() {
        eprintln!("  {} {}", "!".yellow(), warning.yellow());
    }

    // Single-file ceiling warning: data-URI media must materialize fully
    // in memory; past tens of MB, video playback degrades or fails.
    if single_file {
        let size = std::fs::metadata(&output_path)?.len();
        if size > 25 * 1024 * 1024 {
            eprintln!(
                "  {} single-file output is {} MB — large inlined media \
                 degrades playback; consider the default directory output",
                "!".yellow(),
                size / (1024 * 1024)
            );
        }
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

/// Resolve a flavor by name across the library and configured extra dirs
/// (library wins, ADR-0007), falling through to the built-in default only
/// for the name "default". Any other unresolved name fails loudly with the
/// searched paths and available names — never a silent substitute.
pub fn load_flavor(config: &Config, name: &str) -> Result<Flavor> {
    let flavor_dirs = config.flavor_dirs();
    let matcher = SldrMatcher::new(config.matching.clone());

    let collection = FlavorCollection::load_from_dirs(&flavor_dirs)?;

    let searched = || {
        flavor_dirs
            .iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };

    if collection.flavors.is_empty() {
        if name == "default" {
            return Ok(Flavor::default());
        }
        anyhow::bail!(
            "Flavor '{name}' not found (searched: {}, built-ins).              Only the built-in 'default' flavor is available.",
            searched()
        );
    }

    let flavor_names = collection.names();

    match matcher.resolve(name, &flavor_names) {
        ResolveResult::Found(result) => collection
            .find(&result.value)
            .cloned()
            .with_context(|| format!("Failed to load flavor '{}'", result.value)),
        ResolveResult::NotFound => {
            if name == "default" {
                // The built-in default is the last stop of the resolution
                // order (library -> extras -> built-ins).
                return Ok(Flavor::default());
            }
            anyhow::bail!(
                "Flavor '{name}' not found (searched: {}, built-ins). Available: {}",
                searched(),
                flavor_names.join(", ")
            );
        }
        ResolveResult::Multiple(matches) => {
            let options: Vec<&str> = matches.iter().map(|m| m.value.as_str()).collect();
            if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                anyhow::bail!(
                    "Ambiguous flavor reference '{name}'. Candidates: {}",
                    options.join(", ")
                );
            }
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple flavors match '{name}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;
            collection
                .find(matches[selection].value.as_str())
                .cloned()
                .with_context(|| {
                    format!("Failed to load flavor '{}'", matches[selection].value)
                })
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
