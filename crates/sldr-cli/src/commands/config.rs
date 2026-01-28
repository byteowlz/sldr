//! Config command - view and edit configuration

use anyhow::Result;
use colored::Colorize;
use sldr_core::config::Config;
use std::process::Command;

pub fn run(key: Option<String>, value: Option<String>, edit: bool) -> Result<()> {
    let config_path = Config::config_file_path();

    if edit {
        // Open in editor
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nano".to_string());

        println!(
            "{} config in {}",
            "Opening".green().bold(),
            editor.cyan()
        );

        Command::new(&editor)
            .arg(&config_path)
            .status()?;

        return Ok(());
    }

    let config = Config::load()?;

    match (key, value) {
        (None, None) => {
            // Show all config
            println!("{}", "Configuration".green().bold());
            println!("  {} {}\n", "File:".dimmed(), config_path.display());

            println!("{}", "[config]".cyan());
            println!(
                "  template_dir = {}",
                format!("\"{}\"", config.config.template_dir).yellow()
            );
            println!(
                "  flavor_dir = {}",
                format!("\"{}\"", config.config.flavor_dir).yellow()
            );
            println!(
                "  default_flavor = {}",
                format!("\"{}\"", config.config.default_flavor).yellow()
            );
            println!(
                "  slidev_port = {}",
                format!("\"{}\"", config.config.slidev_port).yellow()
            );
            println!(
                "  agent = {}",
                format!("\"{}\"", config.config.agent).yellow()
            );

            println!("\n{}", "[presentations]".cyan());
            println!(
                "  slide_dir = {}",
                format!("\"{}\"", config.presentations.slide_dir).yellow()
            );
            println!(
                "  output_dir = {}",
                format!("\"{}\"", config.presentations.output_dir).yellow()
            );
            println!(
                "  skeleton_dir = {}",
                format!("\"{}\"", config.presentations.skeleton_dir).yellow()
            );

            println!("\n{}", "[matching]".cyan());
            println!(
                "  threshold = {}",
                config.matching.threshold.to_string().yellow()
            );
            println!(
                "  max_suggestions = {}",
                config.matching.max_suggestions.to_string().yellow()
            );
        }
        (Some(key), None) => {
            // Show specific key
            let value = get_config_value(&config, &key);
            match value {
                Some(v) => println!("{} = {}", key.cyan(), v.yellow()),
                None => println!("{}: Unknown config key '{}'", "Error".red(), key),
            }
        }
        (Some(_key), Some(_value)) => {
            // Set value (TODO: implement actual setting)
            println!(
                "{} Setting config values is not yet implemented",
                "Note:".yellow()
            );
            println!("  Edit the config file directly: {}", config_path.display());
            println!("  Or run: {} config --edit", "sldr".cyan());
        }
        (None, Some(_)) => {
            anyhow::bail!("Cannot set a value without specifying a key");
        }
    }

    Ok(())
}

fn get_config_value(config: &Config, key: &str) -> Option<String> {
    match key {
        "template_dir" | "config.template_dir" => Some(config.config.template_dir.clone()),
        "flavor_dir" | "config.flavor_dir" => Some(config.config.flavor_dir.clone()),
        "default_flavor" | "config.default_flavor" => Some(config.config.default_flavor.clone()),
        "slidev_port" | "config.slidev_port" => Some(config.config.slidev_port.clone()),
        "agent" | "config.agent" => Some(config.config.agent.clone()),
        "slide_dir" | "presentations.slide_dir" => Some(config.presentations.slide_dir.clone()),
        "output_dir" | "presentations.output_dir" => Some(config.presentations.output_dir.clone()),
        "skeleton_dir" | "presentations.skeleton_dir" => {
            Some(config.presentations.skeleton_dir.clone())
        }
        "threshold" | "matching.threshold" => Some(config.matching.threshold.to_string()),
        "max_suggestions" | "matching.max_suggestions" => {
            Some(config.matching.max_suggestions.to_string())
        }
        _ => None,
    }
}
