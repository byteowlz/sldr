# Issues

## Open

### [trx-pg1a] Agent-friendly CLI improvements (P1, epic)
Improve sldr CLI for agent/LLM use with JSON input/output support, batch operations, and better ergonomics

### [trx-3f4w] sldr Octo Integration (P1, epic)
Complete sldr integration with Octo for visual presentation building

## Components

### 1. sldr-server crate (HTTP API)
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
- IEM (Fraunhofer IEM colors, logo)
- byteowlz (byteowlz branding)
...


### [trx-3f4w.3] Templates with v-drag anchors (P2, task)
Create reusable slide templates with positioned v-drag elements

Templates to create:
- title-slide.md (logo, title, subtitle, decoration)
- two-column.md (left content, right content, header)
...


## Closed

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
