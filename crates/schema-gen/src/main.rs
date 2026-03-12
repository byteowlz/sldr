/// JSON Schema Generator for sldr
///
/// This binary generates JSON schemas and example TOML configurations for sldr:
///
/// **Schemas** (for IDE autocompletion):
/// - sldr.config.schema.json (main configuration)
/// - sldr.flavor.schema.json (flavor/theme configuration)
/// - sldr.skeleton.schema.json (presentation skeleton configuration)
/// - sldr.slide-input.schema.json (JSON input for batch slide creation)
/// - sldr.skeleton-input.schema.json (JSON input for skeleton creation)
///
/// **Example Configs** (showing defaults with comments):
/// - config.toml (main configuration)
/// - example-flavor.toml (flavor configuration)
/// - example-skeleton.toml (skeleton configuration)
use schemars::schema_for;
use sldr_core::presentation::SkeletonInput;
use sldr_core::slide::SlideInputBatch;
use sldr_core::{Config, Flavor, Skeleton};
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("Generating JSON schemas and example configs for sldr...");

    // Get the directories
    let schemas_dir = get_schemas_dir();
    let examples_dir = get_examples_dir();
    fs::create_dir_all(&schemas_dir).expect("Failed to create schemas directory");
    fs::create_dir_all(&examples_dir).expect("Failed to create examples directory");

    // Generate schemas
    println!("\n=== Generating JSON Schemas ===");
    generate_config_schema(&schemas_dir);
    generate_flavor_schema(&schemas_dir);
    generate_skeleton_schema(&schemas_dir);
    generate_slide_input_schema(&schemas_dir);
    generate_skeleton_input_schema(&schemas_dir);

    // Generate example configs
    println!("\n=== Generating Example Configs ===");
    generate_config_example(&examples_dir);
    generate_flavor_example(&examples_dir);
    generate_skeleton_example(&examples_dir);

    println!("\n✓ All schemas and examples generated successfully!");
    println!("  Schemas:   {:?}", schemas_dir);
    println!("  Examples:  {:?}", examples_dir);
}

/// Get the schemas directory: examples/schemas/
fn get_schemas_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = manifest_dir
        .parent()
        .expect("Failed to get workspace directory")
        .parent()
        .expect("Failed to get workspace parent directory");
    workspace_dir.join("examples/schemas")
}

/// Get the examples directory: examples/
fn get_examples_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_dir = manifest_dir
        .parent()
        .expect("Failed to get workspace directory")
        .parent()
        .expect("Failed to get workspace parent directory");
    workspace_dir.join("examples")
}

/// Generate schema for main config.toml
fn generate_config_schema(schemas_dir: &PathBuf) {
    let schema = schema_for!(Config);

    let output_path = schemas_dir.join("sldr.config.schema.json");
    let json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    fs::write(&output_path, json).expect("Failed to write config schema");
    println!("  ✓ Generated config schema: {:?}", output_path);
}

/// Generate schema for flavor.toml
fn generate_flavor_schema(schemas_dir: &PathBuf) {
    let schema = schema_for!(Flavor);

    let output_path = schemas_dir.join("sldr.flavor.schema.json");
    let json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    fs::write(&output_path, json).expect("Failed to write flavor schema");
    println!("  ✓ Generated flavor schema: {:?}", output_path);
}

/// Generate schema for skeleton.toml
fn generate_skeleton_schema(schemas_dir: &PathBuf) {
    let schema = schema_for!(Skeleton);

    let output_path = schemas_dir.join("sldr.skeleton.schema.json");
    let json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    fs::write(&output_path, json).expect("Failed to write skeleton schema");
    println!("  ✓ Generated skeleton schema: {:?}", output_path);
}

/// Generate example config.toml
fn generate_config_example(examples_dir: &PathBuf) {
    let output_path = examples_dir.join("config.toml");
    let content = r##"# sldr configuration file
# Location: ~/.config/sldr/config.toml
#
# This file is automatically created on first run with default values.
# All paths support ~ expansion and environment variables like $HOME.
#
# Schema reference for IDE autocompletion (requires Even Better TOML extension)

"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.config.schema.json"

[config]
# Directory containing slide templates
template_dir = "~/.config/sldr/templates"

# Directory containing flavors (themes/styles)
flavor_dir = "~/.config/sldr/flavors"

# Default flavor to use when none is specified
default_flavor = "default"

# Port for slidev server
slidev_port = "3030"

# Preferred AI agent for slide generation
# Possible values: "opencode", "claude code", "codex"
agent = "opencode"

[presentations]
# Directory containing individual slide markdown files
slide_dir = "~/sldr/slides"

# Directory for generated/built presentations
output_dir = "~/sldr/presentations"

# Directory containing presentation skeletons
skeleton_dir = "~/sldr/skeletons"

[matching]
# Order in which to try resolution methods when finding slides
# Options: "anchor", "exact", "fuzzy", "index", "interactive"
resolution_order = [
    "anchor",
    "exact",
    "fuzzy",
    "index",
    "interactive",
]

# Minimum fuzzy match score (0-100)
# Lower values are more lenient, higher values require closer matches
threshold = 50.0

# Maximum number of suggestions to show for ambiguous matches
max_suggestions = 6
"##;
    fs::write(&output_path, content).expect("Failed to write config example");
    println!("  ✓ Generated config example: {:?}", output_path);
}

/// Generate example flavor.toml
fn generate_flavor_example(examples_dir: &PathBuf) {
    let output_path = examples_dir.join("example-flavor.toml");
    let content = r##"# Example flavor configuration for sldr
#
# Flavors define the visual appearance of presentations, separating
# content from style so the same slides can use different themes.
#
# Place this file in: ~/.config/sldr/flavors/<flavor-name>/flavor.toml

"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.flavor.schema.json"

# Unique identifier for this flavor
name = "example"

# Human-readable name
display_name = "Example Flavor"

# Description of this flavor
description = "An example flavor configuration"

[colors]
# Primary brand color (used for headings, links, etc.)
primary = "#3b82f6"

# Secondary color
secondary = "#6366f1"

# Background color
background = "#ffffff"

# Text color
text = "#1f2937"

# Accent color for highlights
accent = "#f59e0b"

# Code block background
code_background = "#f3f4f6"

# Dark mode color overrides (applied when dark mode is toggled)
# Only specify values you want to override; unset values keep the light defaults.
[dark_colors]
primary = "#60a5fa"
secondary = "#818cf8"
background = "#0f172a"
text = "#e2e8f0"
accent = "#fbbf24"
code_background = "#1e293b"
code_text = "#e2e8f0"

[typography]
# Font for headings (h1-h6)
heading_font = "Inter, sans-serif"

# Font for body text
body_font = "Inter, sans-serif"

# Font for code blocks
code_font = "JetBrains Mono, monospace"

# Base font size
base_size = "16px"

[background]
# Background type: "color", "image", "gradient", or "svg"
background_type = "color"

# Value depends on background_type:
# - color: hex color code (e.g., "#ffffff")
# - image: path to image file (e.g., "/assets/background.png")
# - gradient: CSS gradient (e.g., "linear-gradient(135deg, #667eea 0%, #764ba2 100%)")
# - svg: path to SVG file (e.g., "/assets/background.svg")
value = "#ffffff"

# Opacity overlay for backgrounds (0.0 - 1.0)
opacity = 1.0

# Optional: directory path for additional assets (logos, images)
# assets_dir = "~/.config/sldr/flavors/example/assets"
"##;
    fs::write(&output_path, content).expect("Failed to write flavor example");
    println!("  ✓ Generated flavor example: {:?}", output_path);
}

/// Generate example skeleton.toml
fn generate_skeleton_example(examples_dir: &PathBuf) {
    let output_path = examples_dir.join("example-skeleton.toml");
    let content = r##"# Example presentation skeleton for sldr
#
# Skeletons define which slides to include in a presentation and
# default settings for the presentation.
#
# Place this file in: ~/sldr/skeletons/<skeleton-name>.toml

"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.skeleton.schema.json"

# Name of the skeleton/presentation
name = "example-talk"

# Optional title for the presentation
title = "Example Presentation"

# Optional description
description = "An example presentation skeleton"

# List of slides to include (names or paths)
# Slides can be:
# - Simple names: "intro", "conclusion"
# - Paths with directories: "ai/transformers", "projects/mlops"
slides = [
    "intro",
    "getting-started",
    "conclusion",
]

# Default flavor to use for styling
flavor = "default"

# Slidev configuration overrides
[slidev_config]
# Theme to use (slidev theme name)
theme = "default"

# Enable/disable drawing feature
drawings = true

# Transition effect between slides
transition = "slide-left"

# Aspect ratio (e.g., "16/9", "4/3")
aspect_ratio = "16/9"

# Canvas width in pixels
canvas_width = 1280

# Enable dark mode
dark_mode = false

# Enable slide recording
record = false
"##;
    fs::write(&output_path, content).expect("Failed to write skeleton example");
    println!("  ✓ Generated skeleton example: {:?}", output_path);
}

/// Generate schema for slide input JSON (used by agents)
fn generate_slide_input_schema(schemas_dir: &PathBuf) {
    let schema = schema_for!(SlideInputBatch);

    let output_path = schemas_dir.join("sldr.slide-input.schema.json");
    let json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    fs::write(&output_path, json).expect("Failed to write slide input schema");
    println!("  ✓ Generated slide input schema: {:?}", output_path);

    // Also generate example JSON
    generate_slide_input_example(&schemas_dir.parent().unwrap().to_path_buf());
}

/// Generate schema for skeleton input JSON (used by agents)
fn generate_skeleton_input_schema(schemas_dir: &PathBuf) {
    let schema = schema_for!(SkeletonInput);

    let output_path = schemas_dir.join("sldr.skeleton-input.schema.json");
    let json = serde_json::to_string_pretty(&schema).expect("Failed to serialize schema");
    fs::write(&output_path, json).expect("Failed to write skeleton input schema");
    println!("  ✓ Generated skeleton input schema: {:?}", output_path);

    // Also generate example JSON
    generate_skeleton_input_example(&schemas_dir.parent().unwrap().to_path_buf());
}

/// Generate example slide input JSON
fn generate_slide_input_example(examples_dir: &PathBuf) {
    let output_path = examples_dir.join("example-slide-input.json");
    let content = r##"{
  "$schema": "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.slide-input.schema.json",
  "directory": "my-topic",
  "slides": [
    {
      "name": "intro",
      "title": "Introduction to My Topic",
      "description": "An overview slide introducing the key concepts",
      "layout": "cover",
      "tags": ["intro", "overview"],
      "content": "# Introduction to My Topic\n\nWelcome to this presentation.\n\n- Key point 1\n- Key point 2\n- Key point 3"
    },
    {
      "name": "concepts",
      "title": "Core Concepts",
      "description": "Explaining the fundamental concepts",
      "layout": "default",
      "tags": ["concepts", "fundamentals"],
      "content": "# Core Concepts\n\n## What is it?\n\nA brief explanation of the concept.\n\n## Why does it matter?\n\nThe importance and applications."
    },
    {
      "name": "code-example",
      "title": "Code Example",
      "description": "A practical code demonstration",
      "layout": "two-cols",
      "tags": ["code", "example"],
      "content": "# Code Example\n\n::left::\n\n```python\ndef hello_world():\n    print(\"Hello, World!\")\n```\n\n::right::\n\n**Explanation:**\n\n- Line 1: Define a function\n- Line 2: Print a message"
    }
  ]
}
"##;
    fs::write(&output_path, content).expect("Failed to write slide input example");
    println!("  ✓ Generated slide input example: {:?}", output_path);
}

/// Generate example skeleton input JSON
fn generate_skeleton_input_example(examples_dir: &PathBuf) {
    let output_path = examples_dir.join("example-skeleton-input.json");
    let content = r##"{
  "$schema": "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.skeleton-input.schema.json",
  "name": "my-presentation",
  "title": "My Awesome Presentation",
  "description": "A presentation about interesting topics",
  "slides": [
    "intro",
    "my-topic/concepts",
    "my-topic/code-example",
    "conclusion"
  ],
  "flavor": "default",
  "slidev_config": {
    "theme": "default",
    "drawings": true,
    "transition": "slide-left",
    "aspect_ratio": "16/9"
  }
}
"##;
    fs::write(&output_path, content).expect("Failed to write skeleton input example");
    println!("  ✓ Generated skeleton input example: {:?}", output_path);
}
