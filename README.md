# sldr

Modular markdown presentations powered by [Slidev](https://sli.dev).

## What it does

sldr separates **content**, **layout**, and **style** for presentations:

- **Slides** - Individual markdown files with YAML frontmatter
- **Templates** - Reusable layouts with positioned elements (via Slidev's v-drag)
- **Flavors** - Brand themes (colors, fonts, logos) that can be swapped at build time
- **Skeletons** - Presentation definitions that reference which slides to include

Build a presentation once, export with different branding. Create a slide once, reuse across presentations.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Slide Library                        │
│  ~/sldr/slides/                                             │
│  ├── intro.md                                               │
│  ├── ai/transformers.md                                     │
│  ├── ai/llm-basics.md                                       │
│  └── conclusion.md                                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                         Skeleton                            │
│  ~/sldr/skeletons/my-talk.toml                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ name = "my-talk"                                    │    │
│  │ slides = ["intro", "ai/transformers", "conclusion"] │    │
│  │ flavor = "IEM"                                      │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     sldr build my-talk                      │
│                              +                              │
│                    Flavor (IEM branding)                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Slidev Presentation                      │
│  ~/sldr/presentations/my-talk/                              │
│  ├── slides.md                                              │
│  ├── style.css                                              │
│  ├── package.json                                           │
│  └── public/assets/                                         │
└─────────────────────────────────────────────────────────────┘
```

## Installation

```bash
# Build from source
cargo install --path crates/sldr-cli

# Initialize directories
sldr init --global
```

## Usage

### Create a slide

```bash
sldr new my-slide
sldr new ai/transformers --template two-column
```

### List slides, skeletons, flavors

```bash
sldr ls slides
sldr ls skeletons
sldr ls flavors
```

### Build a presentation

```bash
sldr build my-talk                    # Build with default flavor
sldr build my-talk --flavor IEM       # Build with specific flavor
sldr build my-talk --flavor IEM --pdf # Build and export to PDF
```

### Add slides to a skeleton

```bash
sldr add my-talk "intro, ai/transformers, conclusion"
```

### Preview a presentation

```bash
sldr open my-talk
```

### Search slides

```bash
sldr search "machine learning"
sldr search --tags "AI,intro"
```

## Slide Format

Slides are markdown files with YAML frontmatter:

```markdown
---
title: Introduction to Transformers
description: Overview of transformer architecture
tags: [AI, deep-learning, transformers]
topic: Machine Learning
layout: two-column
---

# Transformers

The transformer architecture revolutionized NLP...

::right::

![Transformer diagram](/assets/transformer.png)
```

## Visual Positioning with v-drag

sldr leverages Slidev's built-in draggable elements. Templates can define positioned anchors:

```markdown
---
title: My Slide
dragPos:
  logo: 850,30,80,_,0
  main-image: 100,200,400,_,0
---

<img v-drag="'logo'" src="/assets/logo.svg" class="h-12">

<v-drag pos="main-image">
  ![Diagram](/assets/diagram.png)
</v-drag>

# Content here
```

**To edit positions visually:**
1. Run `sldr open <presentation>`
2. Double-click any v-drag element
3. Drag to reposition
4. Click outside to confirm
5. Slidev automatically updates `dragPos` in the source file

## Flavors

Flavors define visual theming separate from content:

```toml
# ~/.config/sldr/flavors/IEM/flavor.toml
name = "IEM"
display_name = "Fraunhofer IEM"

[colors]
primary = "#179c7d"
secondary = "#005b7f"
background = "#ffffff"
text = "#1f2937"

[typography]
heading_font = "Source Sans Pro, sans-serif"
body_font = "Source Sans Pro, sans-serif"

[background]
background_type = "image"
value = "/assets/iem-background.svg"
```

## Configuration

```toml
# ~/.config/sldr/config.toml
[config]
template_dir = "~/.config/sldr/templates"
flavor_dir = "~/.config/sldr/flavors"
default_flavor = "default"
slidev_port = "3030"

[presentations]
slide_dir = "~/sldr/slides"
output_dir = "~/sldr/presentations"
skeleton_dir = "~/sldr/skeletons"

[matching]
threshold = 50.0
max_suggestions = 6
```

## Project Structure

```
sldr/
├── crates/
│   ├── sldr-core/      # Library: slides, skeletons, flavors, presentations
│   ├── sldr-cli/       # CLI binary
│   └── sldr-server/    # HTTP API (planned)
├── examples/
│   └── config.toml     # Example configuration
└── history/            # Development notes
```

## Roadmap

- [x] Core library (slides, skeletons, flavors, fuzzy matching)
- [x] CLI (build, ls, new, add, rm, search, preview, open)
- [ ] HTTP API for Octo integration
- [ ] Slidev process manager with dragPos sync
- [ ] Template library with v-drag layouts
- [ ] Octo frontend components

See `.trx/` for detailed issue tracking.

## License

Proprietary
