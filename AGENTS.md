# sldr - Markdown based presentations with self-contained HTML output

**IMPORTANT**: This document is meant for the development of sldr. For using sldr, please refer to AGENTS_USE.md

## Overview

sldr is an application for creating, managing and updating presentations. The goal is to define every new slide once at the most and reuse it in any way imaginable without having to manually copy anything.
One important aspect is the concept of flavors. Every slide is defined in markdown first, including references for images to be used. The background can then be dynamically applied by specifying which "flavor" to use.
A flavor can be as simple as a background color or background image along with the color scheme to be used for different elements of a slide. It can also be an svg containing vector logos. This means that we strictly separate content (e.g. slides/slide_1.md) from style (flavor/flavorXY/assets.*) and layout (e.g. templates/full_image.md or templates/multi_image.md) although we define which layout template we want to use within our slide_1.md.

sldr compiles markdown slides into self-contained HTML files with zero runtime dependencies. No node, npm, or bun needed - a single Rust binary produces a single HTML file with all CSS and JS inlined. The HTML includes a built-in presenter engine with keyboard navigation, transitions, speaker notes, overview grid, dark/light mode, flavor switching, and inline editing.

Each slide is usually an individual markdown file. This file can still contain multiple slides that should never be separated (concepts building on each other, for example) but each slide file should be reusable and mix-and-matchable into a *presentation*. It is therefore important that we provide as much context for a given slide as possible. A yaml header of a slide(set) should contain valuable information on the slide content, topic, research area etc. We can optionally use AI agents to enrich this.

Presentations are a subset of our collective slide set that are created by selecting which slides to use. We can define presentations in skeletons or interactively via the CLI.

By creating this modular scaffold, we also enable the use of AI Agents (e.g. Claude Code, OpenAI Codex, or opencode) for easily creating new slides and presentations.

## CLI

The primary interface for creating a full presentation from our slides is the CLI.

```bash
sldr build name_of_skeleton                          # Build HTML with default flavor
sldr build name_of_skeleton --flavor name_of_flavor  # Build with specific flavor
sldr build name_of_skeleton --pdf                    # Build and export to PDF
sldr watch name_of_skeleton                          # Dev server with live-reload
sldr watch name_of_skeleton --flavor dark --port 8080
sldr open name_of_presentation                       # Open built HTML in browser
sldr export name_of_skeleton --format pdf            # Export to PDF via headless Chrome
sldr export name_of_skeleton --format pptx           # Export to PPTX (slide screenshots)
sldr preview slide_name                              # Quick single-slide preview
sldr add name_of_presentation slide_names            # Append slides to a skeleton
sldr ls slides                                       # List available slides
sldr ls skeletons                                    # List available skeletons
sldr ls flavors                                      # List available flavors
sldr new slide_name --template two-cols              # Create a new slide
sldr init                                            # Initialize sldr directories
```

Slides can be in subdirs and nested subdirs. Fuzzy matching works without full paths.

**IMPORTANT**: file extensions can be used but are not required. The CLI automatically resolves them. Also, names are fuzzily matched so that typos or incomplete names are robustly resolved. In case of multiple matches, the CLI outputs the names of the found matches.

## Keyboard Shortcuts (in presenter)

| Key | Action |
|-----|--------|
| Arrow keys / Space / Enter | Navigate slides |
| O | Overview grid |
| S | Speaker notes window |
| F | Fullscreen |
| D | Toggle dark/light mode |
| T | Flavor selector (multi-flavor only) |
| E | Toggle edit mode (contenteditable) |
| Ctrl+S (in edit mode) | Download modified HTML |
| Home / End | First / last slide |

## Configuration

sldr is configured via a config.toml file in $XDG_CONFIG_HOME/sldr/ (defaults to ~/.config/sldr/). If the file doesn't exist yet, it is created on first run of the sldr CLI.

```toml
"$schema" = "https://raw.githubusercontent.com/byteowlz/schemas/refs/heads/main/sldr/sldr.config.schema.json"

[config]
template_dir = "~/.config/sldr/templates"
flavor_dir = "~/.config/sldr/flavors"
default_flavor = "default"
dev_port = "3030"           # Port for sldr watch dev server
agent = "opencode"          # AI agent: "opencode", "claude code", "codex"

[presentations]
slide_dir = "~/sldr/slides"
output_dir = "~/sldr/presentations"
skeleton_dir = "~/sldr/skeletons"

[matching]
resolution_order = ["anchor", "exact", "fuzzy", "index", "interactive"]
threshold = 50.0
max_suggestions = 6
```

## Architecture

### Crates

| Crate | Purpose |
|-------|---------|
| `sldr-core` | Config, slide parsing, flavor loading, fuzzy matching, skeleton management |
| `sldr-renderer` | Markdown-to-HTML compiler (pulldown-cmark + syntect), template engine, PPTX writer |
| `sldr-cli` | CLI commands (build, watch, export, open, preview, add, etc.) |
| `sldr-server` | HTTP API for programmatic access |
| `schema-gen` | JSON schema and example config generator |

### Rendering Pipeline

```
Slide .md -> pulldown-cmark -> syntect -> HTML fragment
                                              |
Flavor .toml -> to_css_variables() -> <style data-flavor>
                                                \
                                                 +-> HtmlRenderer::render()
                                                /            |
base.css + presenter.js (include_str!)  -------+             v
                                                    Single .html file
```

### Key Assets (embedded in binary via include_str)

- `crates/sldr-renderer/assets/base.css` - Layouts, transitions, toolbar, edit mode, print styles
- `crates/sldr-renderer/assets/presenter.js` - Navigation, overview, notes, dark mode, flavors, editing
- `crates/sldr-renderer/templates/*.html` - 12 layout templates (default, cover, center, two-cols, etc.)

## Technology Stack

sldr is built in Rust using:

- `pulldown-cmark` - CommonMark markdown parsing
- `syntect` - Syntax highlighting (TextMate grammars)
- `clap` - CLI argument parsing
- `axum` / `tokio` - HTTP server (watch mode, server API)
- `notify` - File system watching (live-reload)
- `serde` / `toml` - Configuration
- `schemars` - JSON schema generation from Rust types
- `fuzzy-matcher` - Fuzzy slide name resolution
- `zip` - PPTX generation
- `colored` / `dialoguer` / `console` - Terminal UI

## Issue Tracking with trx

This project uses `trx` (git-backed issue tracker) for task management. Issues are stored in `.trx/` directory and committed to git.

### Quick trx commands

```bash
trx list                    # List all open issues
trx show <id>               # Show issue details
trx create "Description"    # Create new issue (use prefixes like [bug], [feature])
trx close <id>              # Mark issue as done/closed
trx update <id>             # Edit issue description/title
```

### Finding work

```bash
trx ready                   # Show unblocked issues (no unresolved deps)
trx list | grep -i bug      # Find bugs to fix
```

### Naming convention

- Use descriptive prefixes: `[bug]`, `[feature]`, `[task]`, `[epic]`
- For complex features, use `.` sub-issues (e.g., `feat-123.1`, `feat-123.2`)
- Link dependencies with `trx dep add <parent> <child>`
