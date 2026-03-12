---
name: sldr-presentations
description: |
  Create markdown-based presentations using sldr CLI. Use this skill when asked to:
  create slides, build presentations, manage presentation skeletons, or work with
  self-contained HTML presentations. Triggers: "create a presentation", "make slides",
  "build a talk", "presentation about X", "sldr".
---

# sldr Presentations Skill

sldr is a CLI tool for creating modular, reusable markdown presentations rendered as self-contained HTML files. No runtime dependencies (no node/npm/bun) - a single binary produces a single HTML file.

## Key Concepts

- **Slides**: Individual markdown files in `~/sldr/slides/` (can be in subdirectories)
- **Skeletons**: TOML files in `~/sldr/skeletons/` that define which slides to include
- **Flavors**: Style definitions in `~/.config/sldr/flavors/` (colors, fonts, backgrounds, dark mode)
- **Templates**: Slide templates in `~/.config/sldr/templates/` (layouts like cover, two-cols, code)

## Agent Workflow

### Creating a Presentation

1. Create slides using JSON input
2. Create a skeleton referencing those slides
3. Validate the skeleton
4. Build the presentation

### Step 1: Create Slides

Use `sldr slides create` with JSON input:

```bash
sldr slides create --file /tmp/slides.json --json
```

**JSON Schema** (write to file, then pass via `--file`):
```json
{
  "directory": "topic-name",
  "slides": [
    {
      "name": "slide-filename",
      "title": "Slide Title",
      "description": "Optional description",
      "layout": "default",
      "tags": ["tag1", "tag2"],
      "content": "# Heading\n\nMarkdown content here."
    }
  ]
}
```

**Available layouts**: `default`, `cover`, `two-cols`, `two-cols-header`, `image`, `image-left`, `image-right`, `center`, `quote`, `section`, `intro`, `end`

**Flags**:
- `--file PATH`: Read JSON from file (recommended over stdin)
- `--json`: Output results as JSON
- `--dry-run`: Preview without creating files
- `--force`: Overwrite existing slides

### Step 2: Create Skeleton

Use `sldr skeleton create` with JSON input:

```bash
sldr skeleton create --file /tmp/skeleton.json --json
```

**JSON Schema**:
```json
{
  "name": "presentation-name",
  "title": "Presentation Title",
  "description": "Optional description",
  "slides": [
    "topic-name/slide-1",
    "topic-name/slide-2",
    "conclusion"
  ],
  "flavor": "default"
}
```

**Flags**: Same as slides create (`--file`, `--json`, `--dry-run`, `--force`)

### Step 3: Validate Skeleton

```bash
sldr skeleton validate presentation-name --json
```

Returns list of found/missing slides.

### Step 4: Build Presentation

```bash
sldr build presentation-name --flavor default
```

Creates `index.html` in `~/sldr/presentations/presentation-name/`. Open it directly in any browser - no server needed.

### Additional Commands

```bash
# Dev server with live-reload (rebuilds on file changes)
sldr watch presentation-name --flavor default

# Export to PDF (requires Chrome/Chromium)
sldr export presentation-name --format pdf

# Export to PPTX (screenshots each slide)
sldr export presentation-name --format pptx

# Open built presentation in browser
sldr open presentation-name

# Quick single-slide preview
sldr preview slide-name
```

## JSON Output Format

All `--json` commands return:

```json
{
  "success": true,
  "dry_run": false,
  "data": { ... }
}
```

On error:
```json
{
  "success": false,
  "error": "Error message",
  "cause": "Detailed cause"
}
```

## Useful Commands

```bash
# List available resources
sldr ls slides          # List all slides
sldr ls skeletons       # List all skeletons  
sldr ls templates       # List available templates
sldr ls flavors         # List available flavors

# Create missing slides from skeleton
sldr slides derive skeleton-name --template default
```

## Slide Content Tips

- Start content with `# Title` for headings
- Use `::left::` and `::right::` for two-column layouts
- Code blocks: Use standard markdown fenced code blocks
- Speaker notes: Add `<!-- notes -->` followed by note text, or `<!-- notes: inline note -->`

## Presenter Shortcuts (in the generated HTML)

| Key | Action |
|-----|--------|
| Arrow keys / Space | Navigate slides |
| O | Overview grid |
| S | Speaker notes window |
| F | Fullscreen |
| D | Dark/light mode toggle |
| T | Flavor selector (multi-flavor) |
| E | Edit mode (inline text editing) |
| Ctrl+S (edit mode) | Download modified HTML |

## Example: Complete Workflow

```bash
# 1. Create slides JSON
cat > /tmp/slides.json << 'EOF'
{
  "directory": "rust-intro",
  "slides": [
    {
      "name": "title",
      "title": "Introduction to Rust",
      "layout": "cover",
      "tags": ["rust", "intro"],
      "content": "# Introduction to Rust\n\nA systems programming language"
    },
    {
      "name": "why-rust",
      "title": "Why Rust?",
      "layout": "default",
      "tags": ["rust", "benefits"],
      "content": "# Why Rust?\n\n- Memory safety without GC\n- Zero-cost abstractions\n- Fearless concurrency"
    }
  ]
}
EOF

# 2. Create slides
sldr slides create --file /tmp/slides.json --json

# 3. Create skeleton JSON
cat > /tmp/skeleton.json << 'EOF'
{
  "name": "rust-talk",
  "title": "Introduction to Rust",
  "slides": ["rust-intro/title", "rust-intro/why-rust"],
  "flavor": "default"
}
EOF

# 4. Create skeleton
sldr skeleton create --file /tmp/skeleton.json --json

# 5. Validate
sldr skeleton validate rust-talk --json

# 6. Build (produces ~/sldr/presentations/rust-talk/index.html)
sldr build rust-talk

# 7. Open in browser
sldr open rust-talk

# Or use dev server with live-reload
sldr watch rust-talk
```
