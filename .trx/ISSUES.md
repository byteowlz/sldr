# Issues

## Open

### [trx-3f4w.5] Octo backend: mount sldr routes (P1, task)
Integrate sldr-server into Octo backend

- Add sldr-server as dependency to Octo backend
- Mount sldr routes at /api/sldr/*
- Pass workspace context (slides dir per workspace?)
...


### [trx-3f4w.2] Slidev process manager (P1, task)
Manage slidev instances for preview and template editing

- Spawn slidev on random available port
- Track running instances (HashMap<session_id, SlidevProcess>)
- Kill process on session end or timeout
...


### [trx-3f4w.1] sldr-server crate with HTTP API (P1, task)
Create sldr-server crate exposing sldr-core functionality via HTTP API

Routes:
- GET /slides - list all slides
- GET /slides/:name - get slide content + metadata
...


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


### [trx-3f4w.6] Octo frontend: SlideLibrary component (P2, task)
React component for browsing and searching slides

Features:
- Grid/list view toggle
- Search by title, tags, content
...


### [trx-3f4w.4] Populate flavor definitions (P2, task)
Create complete flavor.toml files with assets for each brand

Flavors:
- acme (ACME acme colors, logo)
- byteowlz (byteowlz branding)
...


### [trx-3f4w.3] Templates with v-drag anchors (P2, task)
Create reusable slide templates with positioned v-drag elements

Templates to create:
- title-slide.md (logo, title, subtitle, decoration)
- two-column.md (left content, right content, header)
...


