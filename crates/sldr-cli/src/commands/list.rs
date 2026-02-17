//! List command - show available slides, presentations, skeletons, flavors, or templates

use super::json_output::JsonResponse;
use crate::templates::TEMPLATES;
use anyhow::Result;
use colored::Colorize;
use serde::Serialize;
use sldr_core::config::Config;
use sldr_core::flavor::FlavorCollection;
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

/// JSON output for a skeleton entry
#[derive(Serialize)]
struct SkeletonEntry {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    slides_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flavor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
}

/// JSON output for a flavor entry
#[derive(Serialize)]
struct FlavorEntry {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

/// JSON output for a template entry
#[derive(Serialize)]
struct TemplateEntry {
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
        "skeletons" | "skeleton" | "sk" => list_skeletons(&config, long, json),
        "flavors" | "flavor" | "f" => list_flavors(&config, long, json),
        "templates" | "template" | "t" => list_templates(&config, long, json),
        _ => {
            if json {
                let response: JsonResponse<()> = JsonResponse::error(
                    format!("Unknown type '{what}'. Use: slides, presentations, skeletons, flavors, or templates"),
                    None,
                );
                response.print();
            } else {
                println!(
                    "{}: Unknown type '{}'. Use: slides, presentations, skeletons, flavors, or templates",
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

fn list_skeletons(config: &Config, long: bool, json: bool) -> Result<()> {
    let skeleton_dir = config.skeleton_dir();

    let mut entries = Vec::new();

    if skeleton_dir.exists() {
        for entry in std::fs::read_dir(&skeleton_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().is_some_and(|ext| ext == "toml") {
                let Some(stem) = path.file_stem() else {
                    continue;
                };
                let name = stem.to_string_lossy().to_string();

                match sldr_core::presentation::Skeleton::load(&path) {
                    Ok(skeleton) => {
                        entries.push(SkeletonEntry {
                            name,
                            slides_count: Some(skeleton.slides.len()),
                            flavor: skeleton.flavor.clone(),
                            title: skeleton.title.clone(),
                        });
                    }
                    Err(_) => {
                        entries.push(SkeletonEntry {
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
            list_type: "skeletons".to_string(),
            count: entries.len(),
            items: entries,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Skeletons".green().bold(),
        skeleton_dir.display().to_string().dimmed()
    );

    if entries.is_empty() {
        println!("  {}", "No skeletons found".dimmed());
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
        println!("\n  {} skeleton(s)", entries.len().to_string().bold());
    }
    Ok(())
}

fn list_flavors(config: &Config, long: bool, json: bool) -> Result<()> {
    let flavor_dir = config.flavor_dir();
    let collection = FlavorCollection::load_from_dir(&flavor_dir)?;

    if json {
        let items: Vec<FlavorEntry> = collection
            .flavors
            .iter()
            .map(|f| FlavorEntry {
                name: f.name.clone(),
                display_name: f.display_name.clone(),
                description: f.description.clone(),
            })
            .collect();
        let result = ListResult {
            list_type: "flavors".to_string(),
            count: items.len(),
            items,
        };
        JsonResponse::success(result).print();
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

fn list_templates(config: &Config, long: bool, json: bool) -> Result<()> {
    let template_dir = config.template_dir();

    // Collect installed templates from filesystem
    let mut installed_templates: HashSet<String> = HashSet::new();
    if template_dir.exists() {
        for entry in std::fs::read_dir(&template_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md") {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    installed_templates.insert(name.to_string());
                }
            }
        }
    }

    let bundled_names: HashSet<_> = TEMPLATES.iter().map(|t| t.name.to_string()).collect();

    if json {
        let mut items: Vec<TemplateEntry> = Vec::new();

        // Add bundled templates
        for t in TEMPLATES {
            items.push(TemplateEntry {
                name: t.name.trim_end_matches(".md").to_string(),
                installed: installed_templates.contains(t.name),
                bundled: true,
            });
        }

        // Add custom (non-bundled) installed templates
        for name in &installed_templates {
            if !bundled_names.contains(name) {
                items.push(TemplateEntry {
                    name: name.trim_end_matches(".md").to_string(),
                    installed: true,
                    bundled: false,
                });
            }
        }

        let result = ListResult {
            list_type: "templates".to_string(),
            count: items.len(),
            items,
        };
        JsonResponse::success(result).print();
        return Ok(());
    }

    println!(
        "{} ({})",
        "Templates".green().bold(),
        template_dir.display().to_string().dimmed()
    );

    // Categorize bundled templates
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
        println!("\n  {} Bundled templates:", "Bundled".cyan());
        for (category, templates) in &categories {
            println!("\n    {}:", category.yellow());
            for name in templates {
                let status = if installed_templates.contains(*name) {
                    "installed".green()
                } else {
                    "not installed".dimmed()
                };
                let name_display = name.trim_end_matches(".md");
                println!("      {name_display} [{status}]");
            }
        }

        // Show custom templates (not in bundled list)
        let custom: Vec<_> = installed_templates
            .iter()
            .filter(|n| !bundled_names.contains(*n))
            .collect();

        if !custom.is_empty() {
            println!("\n  {} Custom templates:", "Custom".cyan());
            for name in &custom {
                let name_display = name.trim_end_matches(".md");
                println!("    {name_display}");
            }
        }
    } else {
        // Simple list
        if installed_templates.is_empty() {
            println!("  {}", "No templates installed".dimmed());
            println!(
                "  {} Run 'sldr init' to install bundled templates",
                "Tip:".blue()
            );
        } else {
            for name in &installed_templates {
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
        installed_templates.len().to_string().bold(),
        TEMPLATES.len().to_string().bold()
    );

    Ok(())
}
