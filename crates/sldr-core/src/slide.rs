//! Slide management - individual markdown slide files

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Metadata from a slide's YAML frontmatter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlideMetadata {
    /// Slide title
    #[serde(default)]
    pub title: Option<String>,

    /// Brief description of the slide content
    #[serde(default)]
    pub description: Option<String>,

    /// Topic or category
    #[serde(default)]
    pub topic: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Template to use for this slide
    #[serde(default)]
    pub template: Option<String>,

    /// Preferred layout
    #[serde(default)]
    pub layout: Option<String>,

    /// Research area this slide belongs to
    #[serde(default)]
    pub research_area: Option<String>,

    /// Author of the slide
    #[serde(default)]
    pub author: Option<String>,

    /// Creation date
    #[serde(default)]
    pub created: Option<String>,

    /// Last modified date
    #[serde(default)]
    pub modified: Option<String>,
}

/// Represents a single slide file
#[derive(Debug, Clone)]
pub struct Slide {
    /// Absolute path to the slide file
    pub path: PathBuf,

    /// Relative path from slide directory
    pub relative_path: String,

    /// Slide name (filename without extension)
    pub name: String,

    /// Parsed metadata from frontmatter
    pub metadata: SlideMetadata,

    /// Raw markdown content (without frontmatter)
    pub content: String,
}

impl Slide {
    /// Load a slide from a file path
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let (metadata, content) = parse_frontmatter(&content);

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(Self {
            path: path.to_path_buf(),
            relative_path: path.to_string_lossy().to_string(),
            name,
            metadata,
            content,
        })
    }

    /// Load a slide and set its relative path from a base directory
    pub fn load_with_base(path: &Path, base: &Path) -> Result<Self> {
        let mut slide = Self::load(path)?;

        if let Ok(relative) = path.strip_prefix(base) {
            slide.relative_path = relative.to_string_lossy().to_string();
        }

        Ok(slide)
    }
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str) -> (SlideMetadata, String) {
    let content = content.trim();

    if !content.starts_with("---") {
        return (SlideMetadata::default(), content.to_string());
    }

    // Find the closing ---
    let rest = &content[3..];
    if let Some(end_idx) = rest.find("\n---") {
        let yaml_content = &rest[..end_idx].trim();
        let markdown_content = &rest[end_idx + 4..].trim();

        let metadata: SlideMetadata =
            serde_yaml_ng::from_str(yaml_content).unwrap_or_default();

        (metadata, markdown_content.to_string())
    } else {
        (SlideMetadata::default(), content.to_string())
    }
}

/// Collection of slides from a directory
#[derive(Debug)]
pub struct SlideCollection {
    /// All slides in the collection
    pub slides: Vec<Slide>,

    /// Base directory for the collection
    pub base_dir: PathBuf,
}

impl SlideCollection {
    /// Load all slides from a directory (recursively)
    pub fn load_from_dir(dir: &Path) -> Result<Self> {
        let mut slides = Vec::new();

        if !dir.exists() {
            return Ok(Self {
                slides,
                base_dir: dir.to_path_buf(),
            });
        }

        for entry in WalkDir::new(dir)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
        {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "md") {
                match Slide::load_with_base(path, dir) {
                    Ok(slide) => slides.push(slide),
                    Err(e) => {
                        tracing::warn!("Failed to load slide {:?}: {}", path, e);
                    }
                }
            }
        }

        // Sort by relative path for consistent ordering
        slides.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(Self {
            slides,
            base_dir: dir.to_path_buf(),
        })
    }

    /// Get all slide names for fuzzy matching
    pub fn names(&self) -> Vec<String> {
        self.slides.iter().map(|s| s.relative_path.clone()).collect()
    }

    /// Find a slide by name or path
    pub fn find(&self, name: &str) -> Option<&Slide> {
        let name_normalized = name.trim_end_matches(".md");

        self.slides.iter().find(|s| {
            s.name == name_normalized
                || s.relative_path == name
                || s.relative_path.trim_end_matches(".md") == name_normalized
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_frontmatter() {
        let content = r#"---
title: Test Slide
tags:
  - test
  - example
---

# Hello World

This is the content.
"#;

        let (metadata, content) = parse_frontmatter(content);
        assert_eq!(metadata.title, Some("Test Slide".to_string()));
        assert_eq!(metadata.tags, vec!["test", "example"]);
        assert!(content.contains("# Hello World"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just Markdown\n\nNo frontmatter here.";
        let (metadata, parsed_content) = parse_frontmatter(content);
        assert!(metadata.title.is_none());
        assert!(parsed_content.contains("# Just Markdown"));
    }
}
