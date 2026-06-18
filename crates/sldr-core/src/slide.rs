//! Slide management - individual markdown slide files

use crate::error::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Input structure for creating slides via JSON
/// Used by agents/LLMs to create one or more slides in a single operation
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[schemars(
    title = "sldr slide input schema",
    description = "JSON schema for creating slides via sldr CLI"
)]
pub struct SlideInputBatch {
    /// List of slides to create
    pub slides: Vec<SlideInput>,

    /// Optional subdirectory within slides folder (applies to all slides)
    #[serde(default)]
    pub directory: Option<String>,
}

/// Input for creating a single slide
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SlideInput {
    /// Filename for the slide (without .md extension)
    pub name: String,

    /// Slide title (shown in frontmatter and as H1 if content doesn't start with heading)
    pub title: String,

    /// Brief description of the slide content
    #[serde(default)]
    pub description: Option<String>,

    /// Tags for categorization and search
    #[serde(default)]
    pub tags: Vec<String>,

    /// Layout to use (default, two-cols, cover, center, image-right, etc.)
    #[serde(default = "default_layout")]
    pub layout: String,

    /// Horizontal alignment override: "left" | "center" | "right".
    /// Optional — when omitted, the layout's default applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,

    /// Vertical alignment override: "top" | "center" | "bottom".
    /// Optional — when omitted, the layout's default applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valign: Option<String>,

    /// The markdown content of the slide (without frontmatter)
    pub content: String,

    /// Optional subdirectory (overrides batch-level directory)
    #[serde(default)]
    pub directory: Option<String>,
}

fn default_layout() -> String {
    "default".to_string()
}

impl SlideInput {
    /// Convert to markdown file content with frontmatter
    pub fn to_markdown(&self) -> String {
        use std::fmt::Write;

        let mut output = String::from("---\n");
        let _ = writeln!(output, "title: \"{}\"", self.title);

        if let Some(ref desc) = self.description {
            let _ = writeln!(output, "description: \"{desc}\"");
        } else {
            output.push_str("description: \"\"\n");
        }

        if self.tags.is_empty() {
            output.push_str("tags: []\n");
        } else {
            let _ = writeln!(output, "tags: [{}]", self.tags.join(", "));
        }

        let _ = writeln!(output, "layout: {}", self.layout);

        if let Some(ref a) = self.align {
            let _ = writeln!(output, "align: {a}");
        }
        if let Some(ref v) = self.valign {
            let _ = writeln!(output, "valign: {v}");
        }

        output.push_str("---\n\n");
        output.push_str(&self.content);

        if !self.content.ends_with('\n') {
            output.push('\n');
        }

        output
    }

    /// Get the effective directory for this slide
    pub fn effective_directory(&self, batch_dir: Option<&str>) -> Option<String> {
        self.directory
            .clone()
            .or_else(|| batch_dir.map(String::from))
    }
}

/// Metadata from a slide's YAML frontmatter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlideMetadata {
    /// Slide title
    #[serde(default)]
    pub title: Option<String>,

    /// Brief description of the slide content
    #[serde(default)]
    pub description: Option<String>,

    /// Subheadline — the smaller line under the headline in a framed
    /// layout's chrome zone (exposed as the `{{subheadline}}` slot).
    #[serde(default)]
    pub subtitle: Option<String>,

    /// Source attribution for web-clipping slides — display text shown as
    /// a "Source: …" line via the `{{source}}` slot. Pair with `source_url`
    /// to make it a link.
    #[serde(default)]
    pub source: Option<String>,

    /// Optional link target for `source`. When set, the source line becomes
    /// a hyperlink (still self-contained; only resolved if the viewer clicks).
    #[serde(default)]
    pub source_url: Option<String>,

    /// Per-slide override for the deck footer line (the `{{footer}}` slot).
    /// Falls back to the flavor's `footer` when omitted.
    #[serde(default)]
    pub footer: Option<String>,

    /// Topic or category
    #[serde(default)]
    pub topic: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Preferred layout
    #[serde(default)]
    pub layout: Option<String>,

    /// Horizontal alignment of slide content: "left", "center", "right".
    /// Overrides the layout's default. Applied as `data-align` on the
    /// slide section so CSS can pin alignment without changing markup.
    #[serde(default)]
    pub align: Option<String>,

    /// Vertical alignment of slide content: "top", "center", "bottom".
    /// Overrides the layout's default. Applied as `data-valign` on the
    /// slide section.
    #[serde(default)]
    pub valign: Option<String>,

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

    /// Per-language overrides for the framed-chrome fields, keyed by language
    /// code (e.g. `de`, `fr`). The top-level fields are the default language;
    /// a `translations.<lang>` block overrides the chrome for that language,
    /// and any omitted field falls back to the top-level value. This is the
    /// frontmatter analog of the body's `::lang:xx::` markers — so a deck
    /// built with `--lang de` translates the headline/subtitle/source, not
    /// just the body. A slide with no `translations` block is unchanged.
    #[serde(default)]
    pub translations: HashMap<String, ChromeTranslation>,
}

/// Per-language overrides for the translatable framed-chrome fields. Any
/// field left unset falls back to the slide's top-level (default-language)
/// value — so a translator only fills the fields that actually differ.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChromeTranslation {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_url: Option<String>,
    #[serde(default)]
    pub footer: Option<String>,
}

/// The framed-chrome fields resolved for one language, plus a gap signal.
#[derive(Debug, Clone, Default)]
pub struct ResolvedChrome {
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub source: Option<String>,
    pub source_url: Option<String>,
    pub footer: Option<String>,
    /// `Some(lang)` when a non-default language was requested but this slide
    /// carries no `translations.<lang>` block while it *does* have chrome —
    /// i.e. its headline/subtitle/source show in the default language. The
    /// build turns this into a loud warning; a translation gap is never
    /// silent (mirrors the body's `LanguageOutcome::Fallback`).
    pub untranslated_to: Option<String>,
}

impl SlideMetadata {
    /// Resolve the framed-chrome fields for one language.
    ///
    /// `target` is `requested` when given, else `deck_default`. Each field
    /// takes the `translations.<target>` override when present, otherwise the
    /// top-level (default-language) value. When a non-default language is
    /// requested but the slide carries no override block for it *and* has
    /// chrome to show, `untranslated_to` is set so the build warns loudly —
    /// a translation gap must never be silent.
    pub fn chrome_for(&self, requested: Option<&str>, deck_default: &str) -> ResolvedChrome {
        let default = deck_default.to_lowercase();
        let target = requested.unwrap_or(deck_default).to_lowercase();
        let block = self.translations.get(&target);

        let pick = |over: Option<&String>, base: &Option<String>| -> Option<String> {
            over.cloned().or_else(|| base.clone())
        };

        let has_chrome =
            self.title.is_some() || self.subtitle.is_some() || self.source.is_some();
        let untranslated_to = if target != default && block.is_none() && has_chrome {
            Some(target.clone())
        } else {
            None
        };

        ResolvedChrome {
            title: pick(block.and_then(|b| b.title.as_ref()), &self.title),
            subtitle: pick(block.and_then(|b| b.subtitle.as_ref()), &self.subtitle),
            source: pick(block.and_then(|b| b.source.as_ref()), &self.source),
            source_url: pick(block.and_then(|b| b.source_url.as_ref()), &self.source_url),
            footer: pick(block.and_then(|b| b.footer.as_ref()), &self.footer),
            untranslated_to,
        }
    }
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
        let (metadata, content) = parse_frontmatter(&content, &path.display().to_string());

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

    /// Construct a slide from an in-memory markdown string.
    ///
    /// Used for bundled sample slides (compiled into the binary via
    /// `include_str!`) and for tests that don't want to touch the filesystem.
    /// `name` is the slide's logical name (filename without extension).
    /// `virtual_path` is the path that will be reported in `path` and
    /// `relative_path` — useful for media resolution if the slide references
    /// images alongside it.
    pub fn from_str(name: impl Into<String>, virtual_path: impl Into<PathBuf>, content: &str) -> Self {
        let path = virtual_path.into();
        let (metadata, body) = parse_frontmatter(content, &path.display().to_string());
        Self {
            relative_path: path.to_string_lossy().to_string(),
            path,
            name: name.into(),
            metadata,
            content: body,
        }
    }
}

/// Parse YAML frontmatter from markdown content
fn parse_frontmatter(content: &str, source: &str) -> (SlideMetadata, String) {
    let content = content.trim();

    if !content.starts_with("---") {
        return (SlideMetadata::default(), content.to_string());
    }

    // Find the closing ---
    let rest = &content[3..];
    if let Some(end_idx) = rest.find("\n---") {
        let yaml_content = &rest[..end_idx].trim();
        let markdown_content = &rest[end_idx + 4..].trim();

        // Invalid YAML must not silently become default metadata — a slide
        // quietly losing its layout/title is exactly the kind of failure
        // that ships unnoticed. Surface it loudly on stderr.
        let metadata: SlideMetadata = match serde_yaml_ng::from_str(yaml_content) {
            Ok(m) => m,
            Err(e) => {
                eprintln!(
                    "  ! {source}: invalid frontmatter (using defaults — layout, title etc. are LOST): {e}"
                );
                SlideMetadata::default()
            }
        };

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
        self.slides
            .iter()
            .map(|s| s.relative_path.clone())
            .collect()
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
        let content = r"---
title: Test Slide
tags:
  - test
  - example
---

# Hello World

This is the content.
";

        let (metadata, content) = parse_frontmatter(content, "test");
        assert_eq!(metadata.title, Some("Test Slide".to_string()));
        assert_eq!(metadata.tags, vec!["test", "example"]);
        assert!(content.contains("# Hello World"));
    }

    #[test]
    fn chrome_for_parses_translations_block() {
        let content = r#"---
title: Hello
subtitle: World
footer: "© Acme"
layout: framed
translations:
  de:
    title: Hallo
    subtitle: Welt
---
body
"#;
        let (meta, _) = parse_frontmatter(content, "test");
        let de = meta.chrome_for(Some("de"), "en");
        assert_eq!(de.title.as_deref(), Some("Hallo"));
        assert_eq!(de.subtitle.as_deref(), Some("Welt"));
        // Omitted field falls back to the top-level value, no gap warning.
        assert_eq!(de.footer.as_deref(), Some("© Acme"));
        assert!(de.untranslated_to.is_none());
    }

    #[test]
    fn chrome_for_default_language_uses_top_level() {
        let mut meta = SlideMetadata {
            title: Some("Hello".into()),
            ..Default::default()
        };
        meta.translations
            .insert("de".into(), ChromeTranslation { title: Some("Hallo".into()), ..Default::default() });
        // Requesting the deck default (or nothing) → top-level, never a gap.
        let en = meta.chrome_for(Some("en"), "en");
        assert_eq!(en.title.as_deref(), Some("Hello"));
        assert!(en.untranslated_to.is_none());
        let none = meta.chrome_for(None, "en");
        assert_eq!(none.title.as_deref(), Some("Hello"));
    }

    #[test]
    fn chrome_for_missing_block_flags_untranslated() {
        let meta = SlideMetadata {
            title: Some("Hello".into()),
            ..Default::default()
        };
        // Non-default language requested, no translations block, slide has
        // chrome → loud-warn signal, content falls back to default language.
        let de = meta.chrome_for(Some("de"), "en");
        assert_eq!(de.title.as_deref(), Some("Hello"));
        assert_eq!(de.untranslated_to.as_deref(), Some("de"));
    }

    #[test]
    fn chrome_for_no_chrome_never_warns() {
        // A language-neutral slide with no chrome should not warn even when a
        // non-default language is requested — nothing to translate.
        let meta = SlideMetadata::default();
        let de = meta.chrome_for(Some("de"), "en");
        assert!(de.untranslated_to.is_none());
        assert!(de.title.is_none());
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "# Just Markdown\n\nNo frontmatter here.";
        let (metadata, parsed_content) = parse_frontmatter(content, "test");
        assert!(metadata.title.is_none());
        assert!(parsed_content.contains("# Just Markdown"));
    }
}
