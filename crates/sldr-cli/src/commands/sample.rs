//! `sldr sample` — render the bundled sample deck against a flavor.
//!
//! The sample deck exercises every major layout and ships compiled into
//! the binary, so this command works offline with no slide files. Useful
//! for evaluating a flavor visually before authoring real content, and
//! for the agent slide catalog (each agent can `GET /api/sample` from
//! `sldr serve` to learn what layouts are available).

use std::path::PathBuf;
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use sldr_core::config::Config;
use sldr_core::flavor::{Flavor, FlavorCollection};
use sldr_renderer::sample::render_sample;

pub fn run(flavor_name: &str, output: Option<PathBuf>, no_open: bool) -> Result<()> {
    let config = Config::load()?;
    let flavor = resolve_flavor(&config, flavor_name)?;

    println!(
        "{} Rendering sample deck with flavor {}",
        "·".dimmed(),
        flavor.name.cyan()
    );

    let html = render_sample(flavor, &[]).context("Failed to render sample deck")?;

    let output_path = match output {
        Some(p) => p,
        None => {
            let dir = std::env::temp_dir().join("sldr-sample");
            std::fs::create_dir_all(&dir)?;
            dir.join(format!("sample-{flavor_name}.html"))
        }
    };

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&output_path, &html)
        .with_context(|| format!("Failed to write sample deck to {}", output_path.display()))?;

    println!(
        "{} Wrote sample deck to {}",
        "+".green(),
        output_path.display()
    );

    if !no_open {
        open_in_browser(&output_path.to_string_lossy())?;
    }

    Ok(())
}

fn resolve_flavor(config: &Config, name: &str) -> Result<Flavor> {
    let collection = FlavorCollection::load_from_dirs(&config.flavor_dirs())?;

    if let Some(flavor) = collection.find(name) {
        return Ok(flavor.clone());
    }

    // Fall back to the built-in default if nothing's installed yet — this
    // keeps `sldr sample` working before `sldr init` has been run.
    if name == "default" {
        return Ok(Flavor::default());
    }

    let available: Vec<String> = collection.flavors.iter().map(|f| f.name.clone()).collect();
    Err(anyhow!(
        "Flavor '{}' not found. Available: {}",
        name,
        if available.is_empty() {
            "(none — run `sldr init` to install defaults)".to_string()
        } else {
            available.join(", ")
        }
    ))
}

fn open_in_browser(path: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).spawn()?;
    }

    #[cfg(target_os = "linux")]
    {
        let result = Command::new("xdg-open").arg(path).spawn();
        if result.is_err() {
            for browser in &["firefox", "chromium", "google-chrome", "brave"] {
                if Command::new(browser).arg(path).spawn().is_ok() {
                    return Ok(());
                }
            }
            println!(
                "{} Could not open browser. Open manually: {}",
                "!".yellow(),
                path
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(["/C", "start", path]).spawn()?;
    }

    Ok(())
}
