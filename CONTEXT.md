# sldr

A lean, bitter-lesson-resistant slide creation and curation engine. Content, layout, and style are strictly separated plain-text data; a deterministic compiler turns them into self-contained HTML. Judgment-heavy work belongs to humans or agents, never to the tool.

## Language

**Bitter-lesson resistant**:
The property that sldr's value survives better models. Two commitments: (A) the tool itself exercises no judgment — it is a deterministic compiler over plain-text data, and all intelligence is delegated to the agent or human operating it; (B) the durable assets are the curated slide library, flavors, and the deterministic rendering contract — including restyling an entire deck with one command — not any workflow built around them. Interactive features are thin, disposable convenience layers.
_Avoid_: future-proof, AI-native

**Library**:
The single root directory carrying all of a user's assets — slides, layouts, flavors, playlists, media. Self-sufficient: cloning the library onto any machine is enough to build everything in it.
_Avoid_: slide dir, workspace

**Axis**:
A dimension along which one deck builds into different outputs without touching content: flavor (style) and language (prose). Axes are build parameters — a playlist may suggest defaults, the build resolves them, the artifact records them.
_Avoid_: variant, mode

**Manifest**:
The record of resolved build parameters carried inside a [[bundle]] beside the sources. The manifest pins the axes so a bundle rebuilds identically; the bundled playlist stays clean so it remains restylable.
_Avoid_: lockfile, build config

**Podium test**:
The self-containedness criterion: a built presentation renders fully offline with zero runtime services — no network, no server, no installed runtime. Applies to every artifact tier; "one file" is a packaging option, not the test.
_Avoid_: self-contained (as a vague claim), portable

**Bundle**:
A `.sldr` file: a plain zip carrying a presentation's sources — playlist, slides, layouts, flavor, media — optionally with the baked HTML cached inside. The exchange format for the work itself: restylable and editable on arrival.
_Avoid_: archive, package, export

**One door**:
The principle that plain files plus the CLI are the single canonical interface for humans and agents alike. There is no privileged agent path: JSON input is a thin adapter that immediately converges to canonical markdown, the server is a [[satellite]], and every command is fully operable non-interactively.
_Avoid_: agent API, machine interface

**Core**:
The durable, judgment-free part of sldr: the plain-text formats (slides, layouts, flavors, playlists, scaffolds), the deterministic compiler, and the CLI. The core must remain fully usable with nothing but a text editor and the binary.
_Avoid_: engine, backend

**Satellite**:
Any interactive convenience layer over the core (server API, flavor builder, Octo). Satellites are disposable: deleting one loses no data and no capability, only convenience. Satellites never extend core formats — anything a satellite reads it derives from existing structure; anything it writes must be expressible in the format as it stands.
_Avoid_: frontend, plugin

**Slide**:
A single markdown file in the library; the unit of curation, reuse, deprecation, and [[playlist]] reference. A slide renders to one or more [[step]]s.
_Avoid_: page, slideset, slide file

**Step**:
One rendered page within a slide. Steps are an inseparable sequence (concepts building on each other) and are never curated, referenced, or deprecated individually.
_Avoid_: sub-slide, page, fragment

**Playlist**:
An ordered selection of slides from the library plus a flavor — the definition of a presentation. Like a musician's setlist over their repertoire: slides are curated once, played in many playlists.
_Avoid_: skeleton (retired), presentation definition

**Scaffold**:
A pre-filled markdown body that `sldr new` starts a slide from. Starting content only — nothing structural; the render structure is chosen by the slide's [[layout]] field.
_Avoid_: template (retired), starter, skeleton (retired)

**Layout**:
The structural unit a slide's content is wrapped in: an HTML file with named slots plus its scoped CSS. Layouts are user-authorable data, never code, and bind only to flavor [[token]]s.
_Avoid_: template (overloaded), theme

**Flavor**:
The style layer: a named set of design [[token]]s plus a confined custom-CSS escape hatch. Swapping flavors restyles an entire deck without touching content or structure — the one-command restyle guarantee.
_Avoid_: theme, brand, skin

**Token**:
A named style variable defined by a flavor and consumed by layouts. Tokens are the binding contract that makes any flavor work with any layout; the token vocabulary grows by promoting recurring custom-CSS patterns, never speculatively.
_Avoid_: CSS variable (the mechanism, not the concept)

**Chrome**:
The persistent framing repeated across a deck's slides: headline and subheadline zone, footer line, logos, permanent background, and the web-clipping [[source]] line. Chrome is split along the factoring — style chrome (background, logos, footer default) is [[flavor]], content chrome (title, subtitle, source, per-slide footer) is frontmatter — and both feed [[layout]] slots (`{{headline}}`, `{{subheadline}}`, `{{footer}}`, `{{source}}`). Not a master-slide layer.
_Avoid_: master slide, template (retired), frame

**Source**:
A web-clipping slide's attribution — `source` text plus optional `source_url` — rendered as a "Source: …" line via the `{{source}}` chrome slot. Self-contained: the link is inert until clicked.
_Avoid_: citation, reference

**Provenance**:
The single source of truth a rendered element traces back to: slide markdown (content), template HTML (structure), or flavor TOML (style). Edits flow back only along provenance; shared sources (templates, flavors) never accept silent writes — changing them is a deliberate act taken with their blast radius visible.
_Avoid_: origin, source mapping (the mechanism, not the concept)

**Curation**:
Keeping the slide library trustworthy over time. The default move is to adapt a slide in place — git history is the archive — never to fork a v2 copy. When a new slide genuinely supersedes a different one, the old slide is marked [[deprecated]]. sldr understands only a minimal curation vocabulary and applies it deterministically; all judgment-heavy librarian work (dedupe, enrichment, quality grading) belongs to agents or humans.
_Avoid_: versioning, archiving

**Deprecated**:
A slide marker meaning "do not offer this for new presentations." Deprecated slides remain resolvable for existing [[playlist]]s but are excluded from discovery by default.
_Avoid_: archived, v1/old
