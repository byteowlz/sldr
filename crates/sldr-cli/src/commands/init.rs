//! Init command - initialize sldr configuration and directories

use crate::flavors;
use crate::scaffolds;
use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::flavor::Flavor;

pub fn run(_global: bool, force: bool) -> Result<()> {
    println!("{} sldr", "Initializing".green().bold());

    let config = Config::default();

    // Create config file
    let config_path = Config::config_file_path();
    if config_path.exists() {
        println!(
            "  {} Config already exists: {}",
            "~".yellow(),
            config_path.display()
        );
    } else {
        config.save()?;
        println!(
            "  {} Created config: {}",
            "+".green(),
            config_path.display()
        );
    }

    // Create directories
    let dirs_to_create = [
        ("Slides", config.slide_dir()),
        ("Output", config.output_dir()),
        ("Playlists", config.playlist_dir()),
        ("Scaffolds", config.scaffold_dir()),
        ("Flavors", config.flavor_dir()),
    ];

    for (name, path) in dirs_to_create {
        if path.exists() {
            println!("  {} {} dir exists: {}", "~".yellow(), name, path.display());
        } else {
            std::fs::create_dir_all(&path)?;
            println!("  {} Created {} dir: {}", "+".green(), name, path.display());
        }
    }

    // Create default flavor
    let default_flavor_dir = config.flavor_dir().join("default");
    if !default_flavor_dir.exists() {
        let default_flavor = Flavor {
            name: "default".to_string(),
            display_name: Some("Default".to_string()),
            description: Some("Clean, minimal default flavor".to_string()),
            colors: sldr_core::flavor::ColorScheme {
                primary: Some("#3b82f6".to_string()),         // Blue
                secondary: Some("#6366f1".to_string()),       // Indigo
                background: Some("#ffffff".to_string()),      // White
                text: Some("#1f2937".to_string()),            // Gray-800
                accent: Some("#f59e0b".to_string()),          // Amber
                code_background: Some("#f3f4f6".to_string()), // Gray-100
                code_text: Some("#1f2937".to_string()),       // Gray-800
                ..Default::default()
            },
            typography: sldr_core::flavor::Typography {
                heading_font: Some("Inter, sans-serif".to_string()),
                body_font: Some("Inter, sans-serif".to_string()),
                code_font: Some("JetBrains Mono, monospace".to_string()),
                base_size: Some("16px".to_string()),
                ..Default::default()
            },
            dark_colors: Some(sldr_core::flavor::ColorScheme {
                primary: Some("#60a5fa".to_string()),
                secondary: Some("#818cf8".to_string()),
                background: Some("#0f172a".to_string()),
                text: Some("#e2e8f0".to_string()),
                accent: Some("#fbbf24".to_string()),
                code_background: Some("#1e293b".to_string()),
                code_text: Some("#e2e8f0".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        default_flavor.save(&default_flavor_dir)?;
        println!("  {} Created default flavor", "+".green());
    }

    // Install bundled example flavors (seed flavors + BHT-derived ports).
    let flavor_dir = config.flavor_dir();
    let installed_flavors = flavors::install_flavors(&flavor_dir, force)?;
    if installed_flavors > 0 {
        let verb = if force { "Updated" } else { "Installed" };
        println!(
            "  {} {} {} example flavors in {}",
            "+".green(),
            verb,
            installed_flavors,
            flavor_dir.display()
        );
    } else {
        println!(
            "  {} Example flavors already exist in {}",
            "~".yellow(),
            flavor_dir.display()
        );
    }

    // Install bundled scaffolds
    let scaffold_dir = config.scaffold_dir();
    let installed = scaffolds::install_scaffolds(&scaffold_dir, force)?;
    if installed > 0 {
        let verb = if force { "Updated" } else { "Installed" };
        println!(
            "  {} {} {} scaffolds in {}",
            "+".green(),
            verb,
            installed,
            scaffold_dir.display()
        );
    } else {
        println!(
            "  {} Scaffolds already exist in {}",
            "~".yellow(),
            scaffold_dir.display()
        );
    }

    // Create example playlist
    let example_playlist = config.playlist_dir().join("example.toml");
    if !example_playlist.exists() {
        std::fs::write(
            &example_playlist,
            r#"# Example presentation playlist
name = "example"
title = "Example Presentation"
description = "A sample presentation playlist"

# List slides by name (fuzzy matched)
slides = [
    # "intro",
    # "topic-1",
    # "conclusion",
]

# Optional: specify a flavor
# flavor = "default"

# Rendering options
[slidev_config]
transition = "fade"
aspect_ratio = "16/9"
"#,
        )?;
        println!("  {} Created example playlist", "+".green());
    }

    println!("\n{} sldr is ready!", "Done!".green().bold());
    println!("\nNext steps:");
    println!(
        "  1. Create slides in {}",
        config.slide_dir().display().to_string().cyan()
    );
    println!(
        "  2. Create a playlist in {}",
        config.playlist_dir().display().to_string().cyan()
    );
    println!(
        "  3. Run {} to build your presentation",
        "sldr build <playlist>".cyan()
    );

    Ok(())
}
