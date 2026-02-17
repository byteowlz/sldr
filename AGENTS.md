# sldr - Markdown based presentations powered by slidev

**IMPORTANT**: This document is meant for the development of sldr. For using sldr, please refer to AGENTS_USE.md

## Overview

sldr is an application for creating, managing and updating presentations. The goal is to define every new slide once at the most and reuse it in any way imaginable without having to manually copy anything.
One important aspect is the concept of flavors. Every slide is defined in markdown first, including references for images to be used. The background can then be dynamically applied by specifying which "flavor" to use.
A flavor can be as simple as a background color or background image along with the color scheme to be used for different elements of a slide. It can also be an svg containing vector logos. This means that we strictly separate content (e.g. slides/slide_1.md) from style (flavor/flavorXY/assets.*) and layout (e.g. templates/full_image.md or templates/multi_image.md) although we define which layout template we want to use within our slide_1.md.

slidr leverages the full power of slidev but makes it easier to create, manage and update a presentation. The important distinction is that each slide is usually an individual markdown file. This file can still contain multiple slides that should never be separated (concepts building on each other, for example) but each slide file should be reusable and mix-and-matchable into a *presentation*. It is therefore important that we provide as much context for a given slide as possible. A yaml header of a slide(set) should contain valuable information on the slide content, topic, research area etc. We can optionally use AI agents to enrich this.

Presentations are a subset of our collective slide set that are created by selecting which slides to use. We can define presentations in presentations/skeletons/ or interactively via the CLI

By creating this modular scaffold, we also enable the use of AI Agents (e.g. Claude Code, OpenAI Codex, or my personal favorite: opencode) for easily creating new slides and presentations

## CLI

The primary interface for creating a full presentation from our slides is the CLI. Here we can create a new presentation, export existing presentations with a flavor of our choice again (as slidev, pdf or pptx)

the fastest way to create a presentation is by providing the skeleton to be used

```bash
sldr build name_of_skelleton

```

this will build the slidev presentation with the default flavor

```bash
sldr build name_of_skelleton --flavor name_of_flavor

```

this will build the slidev presentation with the specified flavor

```bash
sldr build name_of_skelleton --flavor name_of_flavor --pdf

```

```bash
sldr add name_of_presentation name_of_slides # multiple slides can be added by separating via comma i.e. "slide_foo, slide_bar, slide_baz"
```

this will append slides to an existing presentation.  

```bash
sldr add name_of_presentation subdirectory1/name_of_slides # multiple slides can be added by separating via comma i.e. "slide_foo, slide_bar, slide_baz"
```

slides can be in subdirs and nested subdirs as well (fuzzy matching should also work without specified dirs).  

```bash
sldr open name_of_presentation
```

starts a slidev presentation (rebuilds presentation if changes are detected)

**IMPORTANT**: file extensions can be used but are not required. The CLI automatically resolves them. Also, names are fuzzily matched so that typos or incomplete names are robustly resolved. In case of multiple matches, the CLI outputs the names of the found matches.

## Working with sldr

The usual workflow would be:

1. create a new slide under the slides directory (sub-directories are also allowed)

## Configuration

sldr is configured via a config.toml file in $XDG_HOME (note: on Mac, always use ~/.config/sldr/config.toml) which on Mac and Linux usually resolves to ~/.config. On Windows its a different directory. If the file doesn't exist yet, it is created on first run of the sldr CLI. Here is an overview of the current configuration:

```toml
[config]
# template and flaor dirs should be portable across devices, hence we default to the dotfiles
template_dir = "~/.config/templates/" # Default value for the folder containing slide templates. We ship a number of templates that can be expanded. 
flavor_dir = "~/.config/sldr/flavors" # 
default_flavor = "default" # We ship a few base flavors
slidev_port = "3030" # For running presntatinos via slidev
agent = "opencode" # Possible values: "claude code", "codex", "opencode"

[presentations]
slide_dir = "~/sldr/slides/"
output_dir = "~/sldr/presenations/"

[matching]
resolution_order = [
    "anchor",
    "exact",
    "fuzzy",
    "index",
    "interactive",
threshold = 50.0
max_suggestions = 6
]
```

## Technology Stack

sldr is build in Rust using the following libraries (among others):

- serde
- sqlx
- clap
- fuzzy-matcher
- colored
- dialoguer
- console

The application is build in such a way that we can add a Tauri user interface at a later stage. We use a cargo workspace and separate our functionality into crates.

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
