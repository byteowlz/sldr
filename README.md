# sldr

Modular markdown presentations rendered as self-contained HTML. No runtime dependencies - a single Rust binary produces a single HTML file with everything inlined.

## What it does

sldr separates **content**, **layout**, and **style** for presentations:

- **Slides** - Individual markdown files with YAML frontmatter
- **Templates** - Reusable layouts (cover, two-cols, image-left, etc.)
- **Flavors** - Brand themes (colors, fonts, backgrounds, dark mode overrides)
- **Skeletons** - Presentation definitions that reference which slides to include

Build a presentation once, export with different branding. Create a slide once, reuse across presentations.

## Architecture

```
+-------------------------------------------------------------+
|                        Slide Library                        |
|  ~/sldr/slides/                                             |
|  +-- intro.md                                               |
|  +-- ai/transformers.md                                     |
|  +-- ai/llm-basics.md                                       |
|  +-- conclusion.md                                          |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|                         Skeleton                            |
|  ~/sldr/skeletons/my-talk.toml                              |
|  +-----------------------------------------------------+   |
|  | name = "my-talk"                                     |   |
|  | slides = ["intro", "ai/transformers", "conclusion"]  |   |
|  | flavor = "acme"                                      |   |
|  +-----------------------------------------------------+   |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|            sldr build my-talk --flavor acme                  |
|                              |                              |
|  pulldown-cmark + syntect + flavor CSS + presenter.js       |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
|             ~/sldr/presentations/my-talk/index.html         |
|             (single self-contained file, open in browser)   |
+-------------------------------------------------------------+
```

## Installation

```bash
# Build from source
cargo install --path crates/sldr-cli

# Initialize directories and default config
sldr init --global
```

## Usage

### Create a slide

```bash
sldr new my-slide
sldr new ai/transformers --template two-cols
```

### List slides, skeletons, flavors

```bash
sldr ls slides
sldr ls skeletons
sldr ls flavors
```

### Build a presentation

```bash
sldr build my-talk                       # Build with default flavor
sldr build my-talk --flavor acme         # Build with specific flavor
sldr build my-talk --pdf                 # Build and export to PDF
```

### Dev server with live-reload

```bash
sldr watch my-talk                       # Default port (3030)
sldr watch my-talk --flavor dark --port 8080
```

Watches slide files, skeletons, and flavors for changes. Rebuilds and reloads the browser automatically via Server-Sent Events.

### Export

```bash
sldr export my-talk --format pdf         # PDF via headless Chrome
sldr export my-talk --format pptx        # PPTX (slide screenshots)
```

### Add slides to a skeleton

```bash
sldr add my-talk "intro, ai/transformers, conclusion"
```

### Open / Preview

```bash
sldr open my-talk           # Open built presentation in browser
sldr preview slide-name     # Quick single-slide preview
```

### Search slides

```bash
sldr search "machine learning"
sldr search --tags "AI,intro"
```

## Presenter Shortcuts

The generated HTML includes a full presenter engine:

| Key | Action |
|-----|--------|
| Arrow keys / Space / Enter | Navigate slides |
| O | Overview grid |
| S | Speaker notes window (with timer) |
| F | Fullscreen |
| D | Dark/light mode toggle |
| T | Flavor selector (multi-flavor presentations) |
| E | Edit mode (inline text editing) |
| Ctrl+S (edit mode) | Download modified HTML |
| Home / End | First / last slide |

## Slide Format

Slides are markdown files with YAML frontmatter:

```markdown
---
title: Introduction to Transformers
description: Overview of transformer architecture
tags: [AI, deep-learning, transformers]
topic: Machine Learning
layout: two-cols
---

# Transformers

::left::

The transformer architecture revolutionized NLP...

::right::

- Self-attention mechanism
- Parallel processing
- Scalable to billions of parameters
```

Speaker notes:
```markdown
# My Slide

Content here.

<!-- notes -->
Remember to mention the key insight about attention mechanisms.
```

## Flavors

Flavors define visual theming separate from content:

```toml
# ~/.config/sldr/flavors/acme/flavor.toml
"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.flavor.schema.json"

name = "acme"
display_name = "ACME Corp"

[colors]
primary = "#179c7d"
secondary = "#005b7f"
background = "#ffffff"
text = "#1f2937"

[dark_colors]
primary = "#2dd4a8"
background = "#0f172a"
text = "#e2e8f0"

[typography]
heading_font = "Inter, sans-serif"
body_font = "Inter, sans-serif"
code_font = "JetBrains Mono, monospace"

[background]
background_type = "color"
value = "#ffffff"
```

## Configuration

```toml
# ~/.config/sldr/config.toml
"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.config.schema.json"

[config]
template_dir = "~/.config/sldr/templates"
flavor_dir = "~/.config/sldr/flavors"
default_flavor = "default"
dev_port = "3030"            # Port for sldr watch
agent = "opencode"           # AI agent: "opencode", "claude code", "codex"

[presentations]
slide_dir = "~/sldr/slides"
output_dir = "~/sldr/presentations"
skeleton_dir = "~/sldr/skeletons"

[matching]
threshold = 50.0
max_suggestions = 6
```

### IDE Autocompletion

All config files support JSON Schema validation. Install the **Even Better TOML** extension in VS Code/editors for inline docs, autocompletion, and validation.

Example configs: `examples/config.toml`, `examples/example-flavor.toml`, `examples/example-skeleton.toml`

### Regenerating Schemas

```bash
just schemas        # Generate all schemas and examples from Rust types
just copy-schemas   # Copy to byteowlz/schemas repository
```

## Project Structure

```
sldr/
+-- crates/
|   +-- sldr-core/       # Library: config, slides, skeletons, flavors, fuzzy matching
|   +-- sldr-renderer/   # HTML compiler: markdown -> self-contained HTML
|   +-- sldr-cli/        # CLI binary (build, watch, export, open, preview, etc.)
|   +-- sldr-server/     # HTTP API for programmatic access
|   +-- schema-gen/      # JSON schema and example config generator
+-- examples/
|   +-- schemas/          # JSON schemas for IDE autocompletion
|   +-- templates/        # Markdown slide templates (30 layouts)
|   +-- config.toml       # Example configuration
|   +-- example-flavor.toml
|   +-- example-skeleton.toml
+-- skill/                # AI agent skill for creating presentations
```

## License

MIT
