# Issues

## Open

### [trx-eeg7.4] [task] Image collage: shape-cutout employee/team grid (image-portraits) (P1, task)
Layout name: `image-portraits`

Composition: portrait grid for employee directories, team pages, contributor walls. Each image clipped to a shape (circle by default; configurable via per-flavor token --sldr-portrait-shape: circle | squircle | hexagon | square). 1 row for small teams, multiple rows for larger groups, auto-wrapping.

Each cell:
...


### [trx-eeg7.3] [task] Image collage: row with subtitles (image-row) (P1, task)
Layout name: `image-row`

Composition: single horizontal row of images, each with a subtitle/caption below. The 'press feature row', 'three pillars', 'brand showcase' pattern. 2-5 images per row recommended.

Each image cell:
...


### [trx-eeg7.1] [task] Image collage: tidy grid layout (image-grid) (P1, task)
Layout name: `image-grid`

Composition: equal cells in a CSS Grid auto-fit/auto-fill arrangement. 1 image fills the slide; 2 side-by-side; 3 in a row; 4 in a 2x2; 6 in a 3x2; 9 in 3x3. Use `grid-template-columns: repeat(auto-fit, minmax(180px, 1fr))` with `aspect-ratio: 4/3` on cells.

Captions optional: if alt text is non-empty, render small mono caption below each image (matching the editorial-serif eyebrow/page style). If alt is empty, no caption.
...


### [trx-eeg7] [epic] Image collage layouts — 1-to-N images with multiple arrangements (P1, epic)
User request: 'i want templates for image collages for 1-n images. different layouts: skewed overlapping, tidy grid, row with subtitles under each image. circular (or other shapes) cutout in 1 or multiple rows with optional subtitles (think employee overview).'

Goal: ship a small family of slide layouts that accept a markdown list of images (with optional captions/subtitles) and arrange them in editorial-quality compositions. Same content shape, different visual treatment per layout.

Markdown convention (proposed): each image is a markdown image with optional alt-as-caption. Order in markdown determines render order. Captions come from frontmatter `captions:` array OR are pulled from the alt text. 1-N images supported.
...


### [trx-jbpj] [epic] Visual flavor builder + agent-driven slide pipeline (P1, epic)
Sldr's two near-term moats: (1) a visual flavor builder that shows ALL templates with placeholder content, with live-update previews so humans and agents can dial in style; (2) an agent-driven slide pipeline where external skills (URL-to-slide, screenshot-to-slide, PPTX import, etc.) drive sldr via a stable HTTP/CLI API.

Architectural split: sldr exposes APIs and a self-describing skill; agents bring content. Sldr does NOT fetch URLs, OCR images, or summarize articles — those are external agent jobs. Sldr does: slide CRUD, asset intake, deterministic rendering, annotation loop.

Sub-issues track the foundation pieces in dependency order.

### [trx-pg1a] Agent-friendly CLI improvements (P1, epic)
Improve sldr CLI for agent/LLM use with JSON input/output support, batch operations, and better ergonomics

### [trx-3f4w] sldr Octo Integration (P1, epic)
Complete sldr integration with Octo for visual presentation building

## Components

### 1. sldr-server crate (HTTP API)
...


### [trx-eeg7.2] [task] Image collage: skewed/overlapping layout (image-stack) (P2, task)
Layout name: `image-stack`

Composition: asymmetric overlapping arrangement. Each image absolutely-positioned with rule-of-thirds offsets and slight rotation (~-3deg to +3deg). Front image largest, back images smaller. Suggests a curated mood-board / 'these things informed the design' feel. 2-5 images recommended.

Implementation:
...


### [trx-jbpj.6] [task] Flavor builder UI — gallery view via sample-deck iframe + full token coverage (P2, task)
Replace the current flavor-builder.html's hand-rolled mock preview slide with an iframe of the real sample deck (trx-jbpj.1). Same renderer, same templates, zero drift between builder preview and 'sldr build' output.

Changes to crates/sldr-renderer/assets/flavor-builder.html:
- Replace .preview-slide div with an iframe pointing at /sample.html on the same origin.
- Live updates via postMessage: when the user moves a slider, send {type:'flavor-update', tokens:{...}} into the iframe; iframe injects/updates a <style data-flavor-live> block. No iframe reload needed for token changes.
...


### [trx-jbpj.5] [task] Annotation loop — comment intake + sldr apply-comments (P2, task)
Human-in-the-loop iteration on agent-generated slides. Adapted from open-slide's click-to-comment → /apply-comments pattern, but kept inside sldr's deterministic-render world.

Mechanism:
- In presenter.js, add 'C' keybinding for comment mode. Click any element → inline textarea.
- Comments persist as <!-- @sldr-comment id=abc anchor=src-range body="..." --> markers inserted into the source markdown right next to the relevant block (anchor uses source-mapping from trx-jbpj.4).
...


### [trx-jbpj.4] [task] Source-mapped HTML output (data-src-file, data-src-range) (P2, task)
Renderer emits data-src-file and data-src-range attributes on every block-level element in the generated HTML, mapping each rendered region back to a byte range in its source markdown file.

What:
- Track byte offsets through the pulldown-cmark event stream in crates/sldr-renderer/src/markdown.rs.
- Emit data-src-file="slides/foo.md" and data-src-range="123-145" on <h1>, <p>, <li>, <pre>, <blockquote>, <section>, etc.
...


### [trx-jbpj.3] [task] sldr skill — emit self-describing agent skill file (P2, task)
Single command that emits an evergreen skill file an agent can load wholesale into context.

CLI: 'sldr skill > SKILL.md' (or 'sldr skill --json' for structured output).

Contents:
...


### [trx-3f4w.9] Octo frontend: TemplateEditor component (P2, task)
React component for visual template editing

Features:
- Iframe to slidev in edit mode
- Instructions overlay (double-click to drag)
...


### [trx-3f4w.8] Octo frontend: PreviewPane component (P2, task)
React component for live slidev preview

Features:
- Iframe to running slidev instance
- Start/stop preview
...


### [trx-3f4w.7] Octo frontend: SkeletonBuilder component (P2, task)
React component for building presentations via drag-drop

Features:
- Drag-drop reordering of slides
- Drop zone for adding slides from library
...


### [trx-3f4w.4] Populate flavor definitions (P2, task)
Create complete flavor.toml files with assets for each brand

Flavors:
- BrandA (partner BrandA colors, logo)
- byteowlz (byteowlz branding)
...


### [trx-3f4w.3] Templates with v-drag anchors (P2, task)
Create reusable slide templates with positioned v-drag elements

Templates to create:
- title-slide.md (logo, title, subtitle, decoration)
- two-column.md (left content, right content, header)
...


### [trx-jbpj.10] [task] sldr corpus dump — emit slides + flavors + templates as in-context bundle for agents (P3, task)
Single command that emits the entire slide/flavor/template corpus as a structured bundle (markdown sections or JSON) suitable for in-context agent consumption.

CLI: 'sldr corpus' or 'sldr corpus --json' or 'sldr corpus --filter slides:tag=ai'.

Use cases:
...


### [trx-jbpj.9] [task] Author 4 more seed flavors — minimal-light, technical-dark, brutalist-mono, swiss-grid (P3, task)
Forcing function for the rich-token schema. If 4 visually distinct flavors can be authored using only [tokens] (no flavor.css escape hatch), the schema is right. If any of them needs the escape hatch, expand the schema before mass-authoring.

editorial-serif already exists. Author:
- minimal-light: Helvetica-adjacent sans, generous whitespace, low contrast, subtle radius.
- technical-dark: dark engineering-doc style, monospace headers, compact density, code-first feel.
...


### [trx-jbpj.8] [task] Thread typography tokens into per-layout heading rules in base.css (P3, task)
The new typography tokens (heading_weight, heading_tracking, heading_leading, heading_transform) are wired into the global h1-h6 rule in base.css, but per-layout rules like .sldr-slide[data-layout="cover"] h1 still hardcode font-size, font-weight, letter-spacing.

Fix:
- Replace hardcoded values in per-layout heading rules with var(--sldr-heading-*) tokens where appropriate.
- Add a heading_scale token if needed (ratio for h1/h2/h3 sizes).
...


## Closed

- [trx-jbpj.16] [bug] Page number always shows '1' — CSS counter doesn't increment past hidden slides (closed 2026-05-04)
- [trx-jbpj.15] [task] Layout positioning unification across all 10 layouts (closed 2026-05-04)
- [trx-jbpj.14] [bug] Presenter slides ghost/stuck on rapid keypress (closed 2026-05-04)
- [trx-jbpj.13] [bug] image-left/image-right ::content:: ::image:: markers not parsed (closed 2026-05-04)
- [trx-jbpj.12] [task] Editorial-serif seed flavor with editorial flourishes (closed 2026-05-04)
- [trx-jbpj.11] [task] Rich flavor token schema + flavor.css escape hatch (closed 2026-05-04)
- [trx-jbpj.7] [bug] Honor [code].syntax_theme from flavor.toml in renderer (closed 2026-05-04)
- [trx-jbpj.2] [task] sldr serve daemon — agent HTTP API (slides, skeletons, assets, build) (closed 2026-05-04)
- [trx-jbpj.1] [task] Bundled sample deck + render_sample helper + sldr flavor sample CLI (closed 2026-05-04)
- [trx-8crj] [epic] Drop slidev - build custom HTML rendering engine (closed 2026-03-12)
- [trx-8crj.8] [task] PPTX export from rendered HTML (closed 2026-03-12)
- [trx-8crj.11] [task] Contenteditable slide editing mode (inline text editing, floating toolbar, save/download) (closed 2026-03-12)
- [trx-8crj.7] [task] PDF export via headless Chrome/Playwright (closed 2026-03-12)
- [trx-8crj.6] [task] Implement sldr watch with live-reload for HTML preview (closed 2026-03-12)
- [trx-8crj.10] [task] Update sldr-server to serve self-contained HTML instead of proxying slidev (closed 2026-03-12)
- [trx-8crj.9] [task] Remove slidev dependency (SlidevManager, npm deps, frontend/) (closed 2026-03-12)
- [trx-8crj.5] [task] Wire sldr build to output HTML instead of slidev markdown (closed 2026-03-12)
- [trx-8crj.2] [task] Create sldr-renderer crate with HTML slide compiler (markdown -> self-contained HTML) (closed 2026-03-12)
- [trx-8crj.4] [task] Port slide templates from slidev Vue layouts to HTML fragments (closed 2026-03-11)
- [trx-8crj.3] [task] Port flavor system to pure CSS custom properties (closed 2026-03-11)
- [trx-8crj.1] [task] Design HTML presenter engine (JS: keyboard nav, transitions, speaker notes, overview grid, progress bar, touch gestures) (closed 2026-03-11)
- [trx-pg1a.3] Auto-generate skeleton from slide directory (closed 2026-02-06)
- [trx-pg1a.7] Add JSON output for all list/show commands (closed 2026-02-06)
- [trx-9mwm] Flavor: code_background and code_text CSS variables not generated (closed 2026-02-06)
- [trx-pg1a.6] Add skeleton validation command (closed 2026-02-04)
- [trx-pg1a.8] Create JSON schema for slide input format (closed 2026-02-04)
- [trx-pg1a.5] Add templates to 'sldr ls' command (closed 2026-02-04)
- [trx-pg1a.4] Fix fuzzy matching for subdirectory paths (closed 2026-02-04)
- [trx-pg1a.2] Add --json flag for skeleton creation (closed 2026-02-04)
- [trx-pg1a.1] Add --json flag for batch slide creation (closed 2026-02-04)
- [trx-3f4w.6] Octo frontend: SlideLibrary component (closed 2026-01-28)
- [trx-3f4w.5] Octo backend: mount sldr routes (closed 2026-01-28)
- [trx-3f4w.2] Slidev process manager (closed 2026-01-28)
- [trx-3f4w.1] sldr-server crate with HTTP API (closed 2026-01-28)
