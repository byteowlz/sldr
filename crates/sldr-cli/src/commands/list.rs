//! List command - show available slides, presentations, playlists, flavors, or scaffolds

use super::json_output::JsonResponse;
use crate::scaffolds::SCAFFOLDS;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use sldr_core::config::Config;
use sldr_core::flavor::{Curation, Density, Flavor, FlavorCollection, Formality, Scheme};
use sldr_core::slide::SlideCollection;
use std::collections::HashSet;

/// JSON output for a slide entry
#[derive(Serialize)]
struct SlideEntry {
    name: String,
    relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout: Option<String>,
}

/// JSON output for a presentation entry
#[derive(Serialize)]
struct PresentationEntry {
    name: String,
    status: String,
}

/// JSON output for a playlist entry
#[derive(Serialize)]
struct PlaylistEntry {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    slides_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flavor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

/// BHT-compatible flavor entry — mirrors the per-scaffold shape in
/// `beautiful-html-templates/index.json` so an agent can read one file
/// and match a brief to a flavor by feeling.
#[derive(Serialize)]
struct FlavorEntry {
    /// Stable identifier (matches BHT's `slug`)
    slug: String,
    /// Human-readable name (BHT's `name`)
    name: String,
    /// One-line summary (BHT's `tagline`; sourced from `description`)
    #[serde(skip_serializing_if = "Option::is_none")]
    tagline: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    mood: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    occasion: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tone: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    formality: Option<Formality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    density: Option<Density>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme: Option<Scheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    best_for: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avoid_for: Option<String>,
}

/// BHT-compatible top-level index. Drop this on disk as `index.json`
/// next to the flavor directory and an agent can match a brief to a
/// flavor without opening every flavor's TOML.
#[derive(Serialize)]
struct FlavorIndex {
    schema_version: u32,
    generated_at: String,
    flavor_count: usize,
    flavors: Vec<FlavorEntry>,
}

impl FlavorEntry {
    fn from_flavor(f: &Flavor) -> Self {
        let Curation {
            mood,
            tone,
            occasion,
            formality,
            density,
            scheme,
            best_for,
            avoid_for,
        } = f.curation.clone();
        Self {
            slug: f.name.clone(),
            name: f.display_name.clone().unwrap_or_else(|| f.name.clone()),
            tagline: f.description.clone(),
            mood,
            occasion,
            tone,
            formality,
            density,
            scheme,
            best_for,
            avoid_for,
        }
    }
}

fn iso8601_utc_now() -> String {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

/// JSON output for a scaffold entry
#[derive(Serialize)]
struct ScaffoldEntry {
    name: String,
    installed: bool,
    bundled: bool,
}

/// Generic list result for JSON output
#[derive(Serialize)]
struct ListResult<T: Serialize> {
    #[serde(rename = "type")]
    list_type: String,
    count: usize,
    items: Vec<T>,
}

pub fn run(what: &str, long: bool, json: bool) -> Result<()> {
    let config = Config::load()?;

    match what.to_lowercase().as_str() {
        "slides" | "slide" | "s" => list_slides(&config, long, json),
        "presentations" | "presentation" | "p" => list_presentations(&config, long, json),
        "playlists" | "playlist" | "pl" => list_playlists(&config, long, json),
        "flavors" | "flavor" | "f" => list_flavors(&config, long, json),
        "scaffolds" | "scaffold" | "sc" => list_scaffolds(&config, long, json),
        "layouts" | "layout" | "la" => list_layouts(&config, json),
        _ => {
            if json {
                let response: JsonResponse<()> = JsonResponse::error(
                    format!("Unknown type '{what}'. Use: slides, presentations, playlists, flavors, or scaffolds"),
                    None,
                );
                response.print();
            } else {
                println!(
                    "{}: Unknown type '{}'. Use: slides, presentations, playlists, flavors, or scaffolds",
                    "Error".red(),
                    what
                );
            }
            Ok(())
        }
    }
}

fn list_slides(config: &Config, long: bool, json: bool) -> Result<()> {
    let slide_dir = config.slide_dir();
    let collection = SlideCollection::load_from_dir(&slide_dir)?;

    if json {
        let items: Vec<SlideEntry> = collection
            .slides
            .iter()
            .map(|s| SlideEntry {
                name: s.name.clone(),
                relative_path: s.relative_path.clone(),
                title: s.metadata.title.clone(),
                tags: s.metadata.tags.clone(),
                topic: s.metadata.topic.clone(),
                layout: s.metadata.layout.clone(),
            })
            .collect();
        let result = ListResult {
            list_type: "slides".to_string(),
            count: items.len(),
            items,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Slides".green().bold(),
        slide_dir.display().to_string().dimmed()
    );

    if collection.slides.is_empty() {
        println!("  {}", "No slides found".dimmed());
        return Ok(());
    }

    for slide in &collection.slides {
        if long {
            let title = slide.metadata.title.as_deref().unwrap_or("(no title)");
            let tags = if slide.metadata.tags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", slide.metadata.tags.join(", "))
            };
            println!(
                "  {} - {}{}",
                slide.relative_path.cyan(),
                title,
                tags.dimmed()
            );
        } else {
            println!("  {}", slide.relative_path);
        }
    }

    println!(
        "\n  {} slide(s)",
        collection.slides.len().to_string().bold()
    );
    Ok(())
}

fn list_presentations(config: &Config, long: bool, json: bool) -> Result<()> {
    let output_dir = config.output_dir();

    let mut entries = Vec::new();

    if output_dir.exists() {
        for entry in std::fs::read_dir(&output_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let slides_path = entry.path().join("slides.md");
                let status = if slides_path.exists() {
                    "ready"
                } else {
                    "incomplete"
                };
                entries.push(PresentationEntry {
                    name,
                    status: status.to_string(),
                });
            }
        }
    }

    if json {
        let result = ListResult {
            list_type: "presentations".to_string(),
            count: entries.len(),
            items: entries,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Presentations".green().bold(),
        output_dir.display().to_string().dimmed()
    );

    if entries.is_empty() {
        println!("  {}", "No presentations found".dimmed());
    } else {
        for entry in &entries {
            if long {
                let status = if entry.status == "ready" {
                    "ready".green()
                } else {
                    "incomplete".yellow()
                };
                println!("  {} [{}]", entry.name.cyan(), status);
            } else {
                println!("  {}", entry.name);
            }
        }
        println!("\n  {} presentation(s)", entries.len().to_string().bold());
    }
    Ok(())
}

fn list_playlists(config: &Config, long: bool, json: bool) -> Result<()> {
    let playlist_dir = config.playlist_dir();

    let mut entries = Vec::new();

    if playlist_dir.exists() {
        for entry in std::fs::read_dir(&playlist_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "toml") {
                let Some(stem) = path.file_stem() else {
                    continue;
                };
                let name = stem.to_string_lossy().to_string();

                match sldr_core::presentation::Playlist::load(&path) {
                    Ok(playlist) => {
                        entries.push(PlaylistEntry {
                            name,
                            slides_count: Some(playlist.slides.len()),
                            flavor: playlist.flavor.clone(),
                            title: playlist.title.clone(),
                        });
                    }
                    Err(_) => {
                        entries.push(PlaylistEntry {
                            name,
                            slides_count: None,
                            flavor: None,
                            title: None,
                        });
                    }
                }
            }
        }
    }

    if json {
        let result = ListResult {
            list_type: "playlists".to_string(),
            count: entries.len(),
            items: entries,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Playlists".green().bold(),
        playlist_dir.display().to_string().dimmed()
    );

    if entries.is_empty() {
        println!("  {}", "No playlists found".dimmed());
    } else {
        for entry in &entries {
            if long {
                if let Some(count) = entry.slides_count {
                    let flavor = entry.flavor.as_deref().unwrap_or("default");
                    println!(
                        "  {} - {} slides, flavor: {}",
                        entry.name.cyan(),
                        count,
                        flavor.yellow()
                    );
                } else {
                    println!("  {} [{}]", entry.name.cyan(), "invalid".red());
                }
            } else {
                println!("  {}", entry.name);
            }
        }
        println!("\n  {} playlist(s)", entries.len().to_string().bold());
    }
    Ok(())
}

fn list_flavors(config: &Config, long: bool, json: bool) -> Result<()> {
    let flavor_dir = config.flavor_dirs()[0].clone();
    let collection = FlavorCollection::load_from_dirs(&config.flavor_dirs())?;

    if json {
        let flavors: Vec<FlavorEntry> = collection
            .flavors
            .iter()
            .map(FlavorEntry::from_flavor)
            .collect();
        let index = FlavorIndex {
            schema_version: 1,
            generated_at: iso8601_utc_now(),
            flavor_count: flavors.len(),
            flavors,
        };
        match serde_json::to_string_pretty(&index) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                let response: JsonResponse<()> =
                    JsonResponse::error(format!("Failed to serialize flavor index: {e}"), None);
                response.print();
            }
        }
        return Ok(());
    }

    println!(
        "{} ({})",
        "Flavors".green().bold(),
        flavor_dir.display().to_string().dimmed()
    );

    if collection.flavors.is_empty() {
        println!("  {}", "No flavors found".dimmed());
        println!("  {} Run 'sldr init' to create default flavors", "i".blue());
        return Ok(());
    }

    for flavor in &collection.flavors {
        if long {
            let desc = flavor.description.as_deref().unwrap_or("(no description)");
            println!("  {} - {}", flavor.name.cyan(), desc.dimmed());
        } else {
            println!("  {}", flavor.name);
        }
    }

    println!(
        "\n  {} flavor(s)",
        collection.flavors.len().to_string().bold()
    );
    Ok(())
}

/// List available layouts: built-ins plus user layouts from layout_dir
/// (user files override built-ins by name).
fn list_layouts(config: &Config, json: bool) -> Result<()> {
    let mut renderer = sldr_renderer::HtmlRenderer::new(sldr_renderer::RenderConfig::default());
    for dir in config.layout_dirs() {
        renderer.load_layouts(&dir)?;
    }
    let names = renderer.layout_names();

    if json {
        let payload = serde_json::json!({
            "list_type": "layouts",
            "layout_dir": config.layout_dir().display().to_string(),
            "items": names,
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    println!("{}", "Layouts".bold());
    for name in &names {
        println!("  {name}");
    }
    println!(
        "
  {} user layouts in {} override built-ins by name",
        "i".blue(),
        config.layout_dir().display()
    );
    Ok(())
}

fn list_scaffolds(config: &Config, long: bool, json: bool) -> Result<()> {
    let scaffold_dir = config.scaffold_dir();

    // Collect installed scaffolds from filesystem
    let mut installed_scaffolds: HashSet<String> = HashSet::new();
    if scaffold_dir.exists() {
        for entry in std::fs::read_dir(&scaffold_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    installed_scaffolds.insert(name.to_string());
                }
            }
        }
    }

    let bundled_names: HashSet<_> = SCAFFOLDS.iter().map(|t| t.name.to_string()).collect();

    if json {
        let mut items: Vec<ScaffoldEntry> = Vec::new();

        // Add bundled scaffolds
        for t in SCAFFOLDS {
            items.push(ScaffoldEntry {
                name: t.name.trim_end_matches(".md").to_string(),
                installed: installed_scaffolds.contains(t.name),
                bundled: true,
            });
        }

        // Add custom (non-bundled) installed scaffolds
        for name in &installed_scaffolds {
            if !bundled_names.contains(name) {
                items.push(ScaffoldEntry {
                    name: name.trim_end_matches(".md").to_string(),
                    installed: true,
                    bundled: false,
                });
            }
        }

        let result = ListResult {
            list_type: "scaffolds".to_string(),
            count: items.len(),
            items,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Scaffolds".green().bold(),
        scaffold_dir.display().to_string().dimmed()
    );

    // Categorize bundled scaffolds
    let categories = [
        (
            "Cover/Title",
            vec!["title.md", "cover.md", "intro.md", "research-title.md"],
        ),
        (
            "Section/Structure",
            vec!["section.md", "default.md", "basic.md", "bullets.md"],
        ),
        (
            "Layout",
            vec!["two-cols.md", "two-cols-header.md", "comparison.md"],
        ),
        (
            "Images",
            vec!["image.md", "image-left.md", "image-right.md", "figure.md"],
        ),
        (
            "Code/Technical",
            vec![
                "code.md",
                "code-comparison.md",
                "terminal.md",
                "architecture.md",
            ],
        ),
        ("Data/Charts", vec!["chart.md", "table.md", "results.md"]),
        (
            "Academic",
            vec!["methodology.md", "discussion.md", "references.md", "qna.md"],
        ),
        (
            "Closing",
            vec!["quote.md", "conclusion.md", "thank-you.md", "end.md"],
        ),
    ];

    if long {
        // Show categorized view
        println!("\n  {} Bundled scaffolds:", "Bundled".cyan());
        for (category, scaffolds) in &categories {
            println!("\n    {}:", category.yellow());
            for name in scaffolds {
                let status = if installed_scaffolds.contains(*name) {
                    "installed".green()
                } else {
                    "not installed".dimmed()
                };
                let name_display = name.trim_end_matches(".md");
                println!("      {name_display} [{status}]");
            }
        }

        // Show custom scaffolds (not in bundled list)
        let custom: Vec<_> = installed_scaffolds
            .iter()
            .filter(|n| !bundled_names.contains(*n))
            .collect();

        if !custom.is_empty() {
            println!("\n  {} Custom scaffolds:", "Custom".cyan());
            for name in &custom {
                let name_display = name.trim_end_matches(".md");
                println!("    {name_display}");
            }
        }
    } else {
        // Simple list
        if installed_scaffolds.is_empty() {
            println!("  {}", "No scaffolds installed".dimmed());
            println!(
                "  {} Run 'sldr init' to install bundled scaffolds",
                "Tip:".blue()
            );
        } else {
            for name in &installed_scaffolds {
                let name_display = name.trim_end_matches(".md");
                let is_bundled = bundled_names.contains(name);
                if is_bundled {
                    println!("  {name_display}");
                } else {
                    println!("  {} {}", name_display, "(custom)".dimmed());
                }
            }
        }
    }

    println!(
        "\n  {} installed, {} bundled available",
        installed_scaffolds.len().to_string().bold(),
        SCAFFOLDS.len().to_string().bold()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sldr_core::flavor::{Curation, Flavor, Formality, Scheme};

    #[test]
    fn flavor_entry_maps_curation_to_bht_shape() {
        let flavor = Flavor {
            name: "editorial-serif".to_string(),
            display_name: Some("Editorial Serif".to_string()),
            description: Some("Magazine-grade serif headlines.".to_string()),
            curation: Curation {
                mood: vec!["literary".into(), "warm".into()],
                tone: vec!["editorial".into()],
                occasion: vec!["essay".into()],
                formality: Some(Formality::MediumHigh),
                density: Some(sldr_core::flavor::Density::Low),
                scheme: Some(Scheme::Light),
                best_for: Some("long-form".into()),
                avoid_for: Some("dashboards".into()),
            },
            ..Flavor::default()
        };
        let entry = FlavorEntry::from_flavor(&flavor);
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["slug"], "editorial-serif");
        assert_eq!(json["name"], "Editorial Serif");
        assert_eq!(json["tagline"], "Magazine-grade serif headlines.");
        assert_eq!(json["mood"][0], "literary");
        assert_eq!(json["formality"], "medium-high");
        assert_eq!(json["density"], "low");
        assert_eq!(json["scheme"], "light");
        assert_eq!(json["best_for"], "long-form");
    }

    #[test]
    fn flavor_entry_omits_empty_fields() {
        let flavor = Flavor {
            name: "bare".to_string(),
            display_name: None,
            description: None,
            ..Flavor::default()
        };
        let entry = FlavorEntry::from_flavor(&flavor);
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["slug"], "bare");
        // name falls back to slug when display_name is missing
        assert_eq!(json["name"], "bare");
        assert!(json.get("tagline").is_none(), "empty tagline skipped");
        assert!(json.get("mood").is_none(), "empty mood should be skipped");
        assert!(json.get("formality").is_none());
        assert!(json.get("best_for").is_none());
    }
}
