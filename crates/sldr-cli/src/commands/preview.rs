//! Preview command - quickly preview a single slide as self-contained HTML

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Select};
use sldr_core::config::Config;
use sldr_core::flavor::Flavor;
use sldr_core::fuzzy::{ResolveResult, SldrMatcher};
use sldr_core::slide::{Slide, SlideCollection};
use sldr_renderer::{HtmlRenderer, RenderConfig};
use std::process::Command;

pub fn run(slide: &str, _port: Option<String>) -> Result<()> {
    let config = Config::load()?;

    println!("{} slide '{}'", "Previewing".green().bold(), slide.cyan());

    // Find the slide
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());

    let found_slide = match matcher.resolve(slide, &slides.names()) {
        ResolveResult::Found(result) => slides
            .find(&result.value)
            .cloned()
            .context("Slide not found")?,
        ResolveResult::NotFound => {
            // Maybe it's a direct path?
            let direct_path = Config::expand_path(slide);
            if direct_path.exists() {
                Slide::load(&direct_path)?
            } else {
                println!("{} Slide '{}' not found.", "!".red(), slide);
                println!("Available slides:");
                for s in slides.slides.iter().take(10) {
                    println!("  - {}", s.relative_path.cyan());
                }
                if slides.slides.len() > 10 {
                    println!("  ... and {} more", slides.slides.len() - 10);
                }
                anyhow::bail!("Slide not found");
            }
        }
        ResolveResult::Multiple(matches) => {
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

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Multiple slides match '{slide}'. Select one:"))
                .items(&options)
                .default(0)
                .interact()?;

            slides
                .find(&matches[selection].value)
                .cloned()
                .context("Slide not found")?
        }
    };

    let slide_title = found_slide
        .metadata
        .title
        .as_deref()
        .unwrap_or("Preview");

    // Render to a temp HTML file
    let temp_dir = std::env::temp_dir().join(format!("sldr-preview-{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir)?;

    let render_config = RenderConfig {
        title: format!("Preview: {slide_title}"),
        transition: "fade".to_string(),
        ..Default::default()
    };

    let mut renderer = HtmlRenderer::new(render_config).add_flavor(Flavor::default());
    renderer.add_slide(&found_slide);

    let output_path = temp_dir.join("index.html");
    renderer.render_to_file(&output_path)?;

    println!(
        "  {} Opening {} in browser",
        ">".cyan(),
        output_path.display()
    );

    // Open in browser
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("xdg-open")
            .arg(output_path.to_string_lossy().as_ref())
            .spawn();
    }

    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open")
            .arg(output_path.to_string_lossy().as_ref())
            .spawn();
    }

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(["/C", "start", &output_path.to_string_lossy()])
            .spawn();
    }

    println!(
        "\n  {} Preview at: {}",
        "i".blue(),
        output_path.display().to_string().underline()
    );
    println!("  {} Temp files will be cleaned up on next run", "i".dimmed());

    Ok(())
}
