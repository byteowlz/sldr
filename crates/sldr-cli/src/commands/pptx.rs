//! `sldr pptx …` — native OOXML PowerPoint export (trx-4s9s).
//!
//! Distinct from `sldr export --format pptx`, which is the lossy
//! screenshot-per-slide path. These commands generate real, editable OOXML:
//! `template` writes theme + masters + layouts (no slides) for a flavor, so an
//! org user can author branded slides directly in PowerPoint.

use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;

use sldr_core::config::Config;
use sldr_pptx::{build_template, select_layouts, Theme};
use sldr_renderer::{LayoutDef, LayoutRegistry};

/// `sldr pptx template` — write an editable template `.pptx` for a flavor:
/// the flavor's colors/fonts become the theme, and every layout that declares
/// `placeholder-text` zones becomes a slideLayout. No slides — a template.
pub fn template(
    flavor: Option<String>,
    output: Option<String>,
    layouts: Option<String>,
) -> Result<()> {
    let config = Config::load()?;
    let flavor_name = flavor.unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor = super::build::load_flavor(&config, &flavor_name)?;

    println!(
        "{} PowerPoint template for flavor '{}'",
        "Generating".green().bold(),
        flavor.name.cyan()
    );

    // Build the layout registry exactly as a deck would: built-ins, then user
    // dirs override by name — so a template reflects the same layouts as the
    // live deck, custom geometry included.
    let mut registry = LayoutRegistry::builtin();
    for dir in config.layout_dirs() {
        registry.load_dir(&dir)?;
    }

    // Explicit `--layouts a,b` subset, or every layout (select_layouts then
    // keeps only the zone-bearing ones).
    let defs: Vec<&LayoutDef> = match &layouts {
        Some(list) => {
            let mut out = Vec::new();
            for name in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                out.push(registry.resolve(name)?);
            }
            out
        }
        None => registry
            .names()
            .iter()
            .filter_map(|n| registry.get(n))
            .collect(),
    };

    let (eligible, skipped) = select_layouts(defs);

    if eligible.is_empty() {
        anyhow::bail!(
            "No layouts with PPTX placeholder zones found. Annotate a layout with \
             `<!-- sldr:zone … rep=placeholder-text … -->` directives, or pass \
             `--layouts framed,two-cols`."
        );
    }

    // Report (don't silently drop) zone-bearing layouts that aren't template-
    // eligible — their zones are picture/shape/bake, which belong to a filled
    // deck, not a template.
    if !skipped.is_empty() {
        println!(
            "  {} {} (no placeholder-text zones — filled-deck only)",
            "skipped:".yellow(),
            skipped.join(", ")
        );
    }

    let theme = Theme::from_flavor(&flavor);
    let bytes = build_template(&theme, &eligible)
        .context("Failed to generate PPTX template OOXML")?;

    let out_path = match output {
        Some(o) => PathBuf::from(o),
        None => {
            let dir = config.output_dir();
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create output dir {}", dir.display()))?;
            dir.join(format!("{}-template.pptx", flavor.name))
        }
    };
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out_path, &bytes)
        .with_context(|| format!("Failed to write {}", out_path.display()))?;

    let names: Vec<&str> = eligible.iter().map(|d| d.name.as_str()).collect();
    println!(
        "\n{} {} layout(s): {}",
        "Included".green(),
        eligible.len(),
        names.join(", ").cyan()
    );
    println!(
        "{} Wrote {}",
        "Success!".green().bold(),
        out_path.display().to_string().cyan()
    );
    Ok(())
}
