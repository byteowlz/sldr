//! Init command - initialize sldr configuration and directories

use crate::templates;
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
        ("Skeletons", config.skeleton_dir()),
        ("Templates", config.template_dir()),
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
            },
            typography: sldr_core::flavor::Typography {
                heading_font: Some("Inter, sans-serif".to_string()),
                body_font: Some("Inter, sans-serif".to_string()),
                code_font: Some("JetBrains Mono, monospace".to_string()),
                base_size: Some("16px".to_string()),
            },
            dark_colors: Some(sldr_core::flavor::ColorScheme {
                primary: Some("#60a5fa".to_string()),
                secondary: Some("#818cf8".to_string()),
                background: Some("#0f172a".to_string()),
                text: Some("#e2e8f0".to_string()),
                accent: Some("#fbbf24".to_string()),
                code_background: Some("#1e293b".to_string()),
                code_text: Some("#e2e8f0".to_string()),
            }),
            background: sldr_core::flavor::BackgroundConfig::default(),
            assets_dir: None,
            source_dir: None,
        };
        default_flavor.save(&default_flavor_dir)?;
        println!("  {} Created default flavor", "+".green());
    }

    // Install bundled templates
    let template_dir = config.template_dir();
    let installed = templates::install_templates(&template_dir, force)?;
    if installed > 0 {
        let verb = if force { "Updated" } else { "Installed" };
        println!(
            "  {} {} {} templates in {}",
            "+".green(),
            verb,
            installed,
            template_dir.display()
        );
    } else {
        println!(
            "  {} Templates already exist in {}",
            "~".yellow(),
            template_dir.display()
        );
    }

    // Create example skeleton
    let example_skeleton = config.skeleton_dir().join("example.toml");
    if !example_skeleton.exists() {
        std::fs::write(
            &example_skeleton,
            r#"# Example presentation skeleton
name = "example"
title = "Example Presentation"
description = "A sample presentation skeleton"

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
        println!("  {} Created example skeleton", "+".green());
    }

    println!("\n{} sldr is ready!", "Done!".green().bold());
    println!("\nNext steps:");
    println!(
        "  1. Create slides in {}",
        config.slide_dir().display().to_string().cyan()
    );
    println!(
        "  2. Create a skeleton in {}",
        config.skeleton_dir().display().to_string().cyan()
    );
    println!(
        "  3. Run {} to build your presentation",
        "sldr build <skeleton>".cyan()
    );

    Ok(())
}
