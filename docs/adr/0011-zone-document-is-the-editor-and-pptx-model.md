# The zone document is the one model behind the visual editor, the deck board, and PPTX round-trip

sldr needs a PowerPoint-like authoring surface: create, edit and tweak slides visually, manage many decks at once, and reuse older slides with far less friction than PowerPoint offers. It must stay lean, run standalone over a private network, and mount inside Oqto as an app. PPTX export and import remain desirable. The risk is that a visual editor grows its own private model of a slide in TypeScript, drifting from the core and colonising the formats (the failure ADR-0001 exists to prevent).

We resolve it by promoting one derived structure to a core primitive and making every visual surface a client of it.

## Decision

**The zone document.** For a given slide, its [[layout]], and a [[flavor]], the core computes a list of zones. Each zone carries the layout's `sldr:zone` data (name, representation policy, percent box) plus two things only the join can know:

- **Binding** — what fills the zone: a frontmatter field (`title`, `subtitle`, `footer`, `source`), a markdown slot with its source range, an image reference, or a flavor field (`footer`, `logos`).
- **Provenance** — the file an edit to that zone writes to: the slide's markdown, the flavor TOML, or (for geometry, always) the layout HTML. The document also reports the layout's usage count so blast radius is visible before an edit (ADR-0002).

The zone document is computed on request and never stored. It is exposed as CLI JSON and as an API route, so agents can ask "what is editable on this slide" with no studio running ([[one door]]).

**Three consumers, one shape.**

1. **The slide editor** renders the real compiler output in an iframe and overlays the zone boxes as pure percent math — no cross-frame DOM measuring. Clicking a zone routes the edit by provenance. Body text edits go through a markdown editor until source-mapped HTML exists; scalar fields edit in place. Moving or resizing a box is a layout edit: the editor shows "used by N slides" and offers *edit layout* or *save as new layout*. There is no per-slide pixel override, ever (ADR-0001).
2. **PPTX export** maps each zone to a placeholder, picture, shape, or baked raster in EMU — the table already documented in `sldr-pptx`.
3. **PPTX import** of sldr-generated decks runs the same table backwards: placeholder index → zone name → binding → markdown field. Import of arbitrary PPTX stays agent work outside the binary.

**The deck board.** Managing decks is a horizontal board of columns. The leftmost column is the *working deck* and the only column that writes a file. Every other column is a read-only source: another deck, a library folder, a search result, or a where-used result. Slides are references, so inserting from a source column appends one playlist line and never copies a file. Hovering a slide highlights it in every column that shares it. A deck becomes current by swapping it into the left slot. The board's state (open columns, order, selection) is UI state persisted per user, never a file the CLI can see.

**Reuse primitives live in the core.** Two derived queries back the board and the editor and are exposed one-door: *where-used* (which playlists reference a slide, derived from playlists plus git history) and *find* (fuzzy plus full-text search over title, tags, topic and body, using the same ranking the CLI uses). The studio never ranks, resolves, or diffs anything itself.

**Studio shape.** The frontend has one backend interface with two implementations — HTTP against `sldr-server` for standalone, the Oqto app SDK for the hosted app — and sections never know which one they run on. Types are generated from the Rust models. The studio owns only UI state.

## Considered Options

- **A freeform canvas with per-slide positions** (rejected): would require a per-slide geometry override, i.e. a new field in the slide format. Pixel-positioned slides also break across aspect ratios. Reuse of layouts is the whole point.
- **A native (gpui) editor** (rejected): every screen centres on the compiler's HTML output and hundreds of rendered thumbnails, so native would embed a webview anyway or reimplement the renderer. A Tauri wrapper around the same web app remains the cheap desktop path.
- **Editor-owned slide model in TypeScript** (rejected): the drift-and-spaghetti path. Every question the UI asks must be a Rust function first.
- **Zone document as a core primitive with three clients** (chosen).

## Consequences

- `sldr-core`/`sldr-renderer` gain a `zone_document(slide, layout, flavor)` function; the CLI gains a JSON command and `sldr-server` a route for it.
- Where-used and find become core functions with CLI and API surfaces; the existing composer search is replaced by them.
- The `sldr:zone` directive gains nothing: bindings are derived from slot names and frontmatter fields as they stand.
- ADR-0004 §3 is amended: "the core will never contain a PPTX importer" becomes "never an importer for arbitrary PPTX"; a deterministic inverse for sldr-generated decks lives in the `sldr-pptx` satellite crate.
- ADR-0009 is amended: standalone `sldr studio` is a first-class deliverable alongside the Oqto app, served by `sldr-server`, optionally over HTTPS (self-signed or a provided pair); the studio avoids secure-context-only browser APIs so plain HTTP on a private network still works.
- Studio work is re-sequenced: primitives first (zone document, where-used, find, media endpoints, generated types), then the board, the editor, the finder, the layout editor with blast radius, the flavor editor, and PPTX round-trip.
