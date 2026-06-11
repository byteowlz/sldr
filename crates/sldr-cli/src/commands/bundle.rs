//! `.sldr` bundle — the exchange format for the work itself (ADR-0006/0007).
//!
//! A bundle is a plain zip (rename or `unzip` works anywhere) carrying the
//! transitive closure of one playlist as a mini-library:
//!
//! ```text
//! manifest.toml      resolved build parameters (pins the axes)
//! playlist.toml      the playlist, unchanged — still restylable
//! slides/...         the slides the playlist references
//! flavors/<name>/    the resolved flavors (when they exist on disk)
//! layouts/<name>.html user layouts referenced by the slides
//! media/...          media at library-relative paths (relative refs survive)
//! ```
//!
//! Nothing of the sender's machine leaks: every path inside is
//! bundle-relative, and no timestamps are written (same sources + same
//! axes → byte-identical content, per the determinism contract).

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use sldr_core::config::Config;
use sldr_core::flavor::FlavorCollection;
use sldr_core::fuzzy::SldrMatcher;
use sldr_core::presentation::Playlist;
use sldr_core::slide::SlideCollection;
use sldr_renderer::{HtmlRenderer, RenderConfig};

/// Resolved build parameters carried beside the sources. The manifest pins
/// the axes so a bundle rebuilds identically; the bundled playlist stays
/// clean so the deck remains restylable on arrival (ADR-0007).
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub sldr_version: String,
    pub playlist: String,
    /// Resolved flavor embed set; first is active.
    pub flavors: Vec<String>,
    /// Resolved language embed set; empty = deck default only.
    pub languages: Vec<String>,
    pub default_language: String,
}

/// Create a `.sldr` bundle from a playlist.
pub fn create(
    playlist_name: &str,
    flavor: Option<String>,
    lang: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let config = Config::load()?;

    println!(
        "{} bundle from playlist '{}'",
        "Creating".green().bold(),
        playlist_name.cyan()
    );

    let playlist = super::build::load_playlist(&config, playlist_name)?;

    // Resolve the axes exactly like `sldr build` (CLI > playlist > config).
    let flavor_arg = flavor
        .or(playlist.flavor.clone())
        .unwrap_or_else(|| config.config.default_flavor.clone());
    let flavor_names: Vec<String> = flavor_arg
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    // Resolve now so a bad flavor fails before an archive is written.
    let mut resolved_flavors = Vec::new();
    for name in &flavor_names {
        resolved_flavors.push(super::build::load_flavor(&config, name)?);
    }

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

    // Resolve the slide closure (fail loud — same rules as build).
    let slides = SlideCollection::load_from_dir(&config.slide_dir())?;
    let matcher = SldrMatcher::new(config.matching.clone());
    let mut resolved_slides = Vec::new();
    for slide_ref in &playlist.slides {
        match super::build::resolve_with_interactive(&matcher, slide_ref, &slides)? {
            Some(slide) => resolved_slides.push(slide),
            None => anyhow::bail!("Slide '{slide_ref}' skipped — cannot bundle a partial deck"),
        }
    }

    let out_path = output.map_or_else(
        || PathBuf::from(format!("{}.sldr", playlist.name)),
        |o| Config::expand_path(&o),
    );

    let file = std::fs::File::create(&out_path)
        .with_context(|| format!("Failed to create {}", out_path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        // Fixed timestamp: same sources + same axes -> byte-identical bundle.
        .last_modified_time(zip::DateTime::default());

    let manifest = Manifest {
        sldr_version: env!("CARGO_PKG_VERSION").to_string(),
        playlist: playlist.name.clone(),
        flavors: resolved_flavors.iter().map(|f| f.name.clone()).collect(),
        languages: languages.clone(),
        default_language,
    };
    zip.start_file("manifest.toml", opts)?;
    zip.write_all(toml::to_string_pretty(&manifest)?.as_bytes())?;

    zip.start_file("playlist.toml", opts)?;
    zip.write_all(toml::to_string_pretty(&playlist)?.as_bytes())?;

    // Slides + the media they reference.
    let slide_dir = config.slide_dir();
    let library = config.library();
    let mut media_files: BTreeSet<(PathBuf, String)> = BTreeSet::new();
    let mut layout_names: BTreeSet<String> = BTreeSet::new();

    for slide in &resolved_slides {
        zip.start_file(format!("slides/{}", slide.relative_path), opts)?;
        zip.write_all(std::fs::read_to_string(&slide.path)?.as_bytes())?;

        if let Some(layout) = &slide.metadata.layout {
            layout_names.insert(layout.clone());
        }

        let slide_parent = slide.path.parent().unwrap_or(Path::new(""));
        for media_ref in markdown_media_refs(&slide.content) {
            let resolved = slide_parent.join(&media_ref);
            let Ok(canonical) = resolved.canonicalize() else {
                continue; // URLs / missing files are the build's problem
            };
            // Mirror the file's place in the mini-library so the slide's
            // relative reference still resolves after extraction.
            let zip_path = if let Ok(rel) = canonical.strip_prefix(&slide_dir) {
                format!("slides/{}", rel.display())
            } else if let Ok(rel) = canonical.strip_prefix(&library) {
                rel.display().to_string()
            } else {
                // Outside the library: bring it under media/ and leave the
                // reference to the build to resolve.
                format!(
                    "media/{}",
                    canonical.file_name().unwrap_or_default().to_string_lossy()
                )
            };
            media_files.insert((canonical, zip_path));
        }
    }

    for (src, zip_path) in &media_files {
        zip.start_file(zip_path.clone(), opts)?;
        zip.write_all(&std::fs::read(src)?)?;
    }

    // Resolved flavors that exist on disk (built-in default has no dir).
    for flavor in &resolved_flavors {
        if let Some(dir) = &flavor.source_dir {
            add_dir_recursive(&mut zip, opts, dir, &format!("flavors/{}", flavor.name))?;
        }
    }

    // User layouts referenced by bundled slides (built-ins ship in every
    // binary; only user files need to travel).
    for name in &layout_names {
        for dir in config.layout_dirs() {
            let path = dir.join(format!("{name}.html"));
            if path.exists() {
                zip.start_file(format!("layouts/{name}.html"), opts)?;
                zip.write_all(&std::fs::read(&path)?)?;
                break;
            }
        }
    }

    zip.finish()?;

    println!(
        "\n{} {} ({} slides, {} flavors{})",
        "Bundled".green().bold(),
        out_path.display().to_string().cyan(),
        resolved_slides.len(),
        resolved_flavors.len(),
        if media_files.is_empty() {
            String::new()
        } else {
            format!(", {} media files", media_files.len())
        }
    );
    println!("  A .sldr is a plain zip — open with: sldr open {}", out_path.display());
    Ok(())
}

/// Build and open a `.sldr` bundle: extract to a temp mini-library, render
/// with the manifest's pinned axes, open the result.
pub fn present(bundle_path: &Path) -> Result<()> {
    println!(
        "{} bundle {}",
        "Opening".green().bold(),
        bundle_path.display().to_string().cyan()
    );

    let file = std::fs::File::open(bundle_path)
        .with_context(|| format!("Failed to open {}", bundle_path.display()))?;
    let mut archive = zip::ZipArchive::new(file).context("Not a valid .sldr bundle (zip)")?;

    let temp_dir = std::env::temp_dir().join(format!("sldr-bundle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp_dir);
    std::fs::create_dir_all(&temp_dir)?;
    archive
        .extract(&temp_dir)
        .context("Failed to extract bundle")?;

    let manifest: Manifest =
        toml::from_str(&std::fs::read_to_string(temp_dir.join("manifest.toml"))?)
            .context("Bundle has no valid manifest.toml")?;
    let playlist = Playlist::load(&temp_dir.join("playlist.toml"))?;

    let slides = SlideCollection::load_from_dir(&temp_dir.join("slides"))?;
    let matcher = SldrMatcher::new(sldr_core::config::MatchingConfig::default());
    let mut resolved = Vec::new();
    for slide_ref in &playlist.slides {
        match super::build::resolve_with_interactive(&matcher, slide_ref, &slides)? {
            Some(slide) => resolved.push(slide),
            None => anyhow::bail!("Bundle is missing slide '{slide_ref}'"),
        }
    }

    // Flavors: bundled first, then built-in default as the last stop.
    let collection = FlavorCollection::load_from_dirs(&[temp_dir.join("flavors")])?;
    let mut flavors = Vec::new();
    for name in &manifest.flavors {
        match collection.find(name) {
            Some(f) => flavors.push(f.clone()),
            None if name == "default" => flavors.push(sldr_core::flavor::Flavor::default()),
            None => anyhow::bail!("Bundle is missing flavor '{name}'"),
        }
    }
    if flavors.is_empty() {
        flavors.push(sldr_core::flavor::Flavor::default());
    }

    let render_config = RenderConfig {
        title: playlist.title.clone().unwrap_or_else(|| playlist.name.clone()),
        languages: manifest.languages.clone(),
        default_language: manifest.default_language.clone(),
        ..Default::default()
    };
    let mut renderer = HtmlRenderer::new(render_config).add_flavors(flavors);
    renderer.load_layouts(&temp_dir.join("layouts"))?;
    renderer.add_slides(&resolved)?;

    let html_path = temp_dir.join("index.html");
    renderer.render_to_file(&html_path)?;
    for warning in renderer.warnings() {
        eprintln!("  {} {}", "!".yellow(), warning.yellow());
    }

    println!(
        "  {} Rebuilt deterministically from bundled sources",
        "i".blue()
    );
    super::open::open_in_browser(&html_path.to_string_lossy())
}

/// Markdown image/media references: `![alt](path)`, skipping URLs and
/// data URIs.
fn markdown_media_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut rest = content;
    while let Some(start) = rest.find("![") {
        rest = &rest[start..];
        let Some(open) = rest.find("](") else { break };
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else { break };
        let target = after[..close].trim();
        if !target.is_empty()
            && !target.starts_with("http://")
            && !target.starts_with("https://")
            && !target.starts_with("data:")
            && !target.starts_with('/')
        {
            refs.push(target.to_string());
        }
        rest = &after[close + 1..];
    }
    refs
}

fn add_dir_recursive(
    zip: &mut zip::ZipWriter<std::fs::File>,
    opts: zip::write::SimpleFileOptions,
    dir: &Path,
    prefix: &str,
) -> Result<()> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort(); // deterministic archive order

    for path in entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            add_dir_recursive(zip, opts, &path, &format!("{prefix}/{name}"))?;
        } else {
            zip.start_file(format!("{prefix}/{name}"), opts)?;
            zip.write_all(&std::fs::read(&path)?)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_relative_media_refs_only() {
        let md = "![a](img/x.png) ![b](https://e.com/y.png) ![c](data:image/png;base64,xx) ![d](/abs.png) ![e](../media/z.jpg)";
        assert_eq!(markdown_media_refs(md), vec!["img/x.png", "../media/z.jpg"]);
    }
}
