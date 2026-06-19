# Changelog

All notable changes to this project will be documented in this file.

## [0.6.1] - 2026-06-19

### Fixed
- Framed headlines no longer collide with a top-corner logo. The headline zone's width is now reserved from the flavor's own logo coordinates — for any logo in the top band (y < 25%) on the right half, the headline stops before its left edge (with a small gap) — so a long headline wraps instead of running under the brand mark. Derived per flavor (via a `--sldr-head-width` token), not hardcoded.

### Added
- Shrink-to-fit body content: if a slide's body overflows its region, the presenter now scales that region down so the content fits — at any viewport size, with no fixed canvas. The body region is scaled directly (transform is visual-only), so the chrome (headline/subheadline/footer/source) and the deck-level logos live in separate elements and are never scaled. Runs on slide change, on resize, and across every page of a PDF export. Only ever shrinks; a slide that already fits is untouched.
- `image-center` layout — a single image shown *whole* (uncropped, `object-fit: contain`), centered on the flavor background, no body text. Fills the gap between `image` (edge-to-edge, fills and may crop) and `feature-image` (forces a caption rail): the layout for a diagram, chart, or screenshot that *is* the slide.
- `framed-figure` layout — the framed-family counterpart of `image-center`: a single whole/centered image in the body with the persistent deck chrome (headline/subheadline/footer + flavor logos/background). For a branded diagram/screenshot slide. (To show logos on it, add `"framed-figure"` to the flavor's `[[logos]]` `layouts` lists.)

### Fixed
- PDF export geometry: PDFs now come out as **frameless 16:9 landscape** pages (one slide per page) instead of portrait Letter with a thick white margin and Chrome's header/footer metadata. The print CSS gained an `@page { size: 1920px 1080px; margin: 0 }` rule, each slide is sized to exactly one page while keeping its on-screen flex layout, and `print-color-adjust: exact` forces the flavor background and colors to print. The 1920×1080 print viewport (rather than a cramped 1280×720) keeps the `vw`/`vh`/`clamp()`-based type close to a maximized-laptop browser, so type stops shrinking and content stops overflowing. (Output is vector, so the larger page costs no sharpness. Full WYSIWYG between an arbitrary browser window and the fixed 16:9 page still requires a fixed slide canvas — tracked separately.)
- `sldr export` now accepts `--lang` (it was silently unsupported) — and `sldr build --pdf` passes the build's `--lang` through. A PDF can't toggle language at view time, so multiple languages export **one file per language** (`deck.de.pdf`, `deck.en.pdf`) rather than a single combined document.
- `sldr export` PDF now **fails loud** when the headless browser exits cleanly but writes no file (e.g. a sandbox/permission denial), surfacing Chrome's own error instead of falsely printing "Success!".
- PDF export now renders **logos on every page**. Logos are a single deck-level overlay (so they don't flicker between slides on screen), which in paged media only landed on the first page. The print-prep step now clones each slide's matching logos (by `data-logo-layouts`) into the slide itself, so every printed page carries exactly its layout's logos. On-screen rendering is unchanged (this runs only in the export/print path).
- The built-in "Source:" chrome label is now localized by `--lang` (e.g. "Quelle:" in German), not just the frontmatter `source` text. It was a hardcoded string in the renderer, so a translated deck kept an English "Source:" prefix. A small shipped label table (en/de/fr/es/it/pt/nl) is resolved by the slide's active language — falling back to the deck default's label, then English — and works for both static `--lang` builds and the live `L` switch. Add a language by appending one row to `SOURCE_LABELS`.

## [0.6.0] - 2026-06-18

### Changed
- Playlist TOML: the rendering-options table is now `[render]` instead of the legacy `[slidev_config]` (a leftover from the pre-rewrite slidev days). Existing playlists with `[slidev_config]` still load — the key is read via a serde alias — and re-saving migrates them to `[render]`. The `SlidevConfig` type alias is removed (`RenderOpts` is the type). Templates, schemas, and examples regenerated.

### Fixed
- Framed chrome (headline/subtitle/source/footer) is now translated by `--lang`, not just the body. Previously `--lang de` swapped the `::lang:de::` body but left the frontmatter-fed chrome in the base language — a German deck kept an English headline. Slides gain an optional per-language `translations:` frontmatter block (the analog of the body's `::lang:xx::`): top-level fields are the default language, `translations.<lang>` overrides the chrome for that language, and an omitted field falls back to the top-level value. A non-default language with chrome but no translation block warns loudly and falls back — never a silent wrong-language headline. Works for both static `--lang` builds and the live `L` switcher (each language is already a full `data-lang` slide section, so no presenter change). trx-2zb8.

## [0.5.0] - 2026-06-17

### Added
- `sldr show layout|flavor <name>` — print the raw source a name resolves to (the authored layout `.html` / flavor `.toml`), honoring the build's resolution order (user library/config dirs override built-ins). Source to stdout (pipeable), origin to stderr, `--json` for both. Fuzzy-matched and fail-loud with the available set. Makes both extension axes legible on a user machine: flavors already lived on disk after `init`, but layouts were binary-only — `show` reads them and reports which copy actually wins (trx-9wqc).
- `builtin_layout_source` / `builtin_layout_names` (sldr-renderer) and `builtin_flavor_files` / `builtin_flavor_slugs` (sldr-cli) accessors backing `show`.

### Fixed
- `sldr watch` now live-reloads flavor and layout edits, not just slides. It watches every dir that feeds a rebuild — all flavor and layout search dirs (library and configured-extra, de-duped) — and re-resolves flavors from disk on each rebuild, so token/background/logo edits take effect instead of rebuilding from the flavor resolved once at startup (trx-ksse).

## [0.4.0] - 2026-05-04

### Added
- Bundled sample deck (`crates/sldr-renderer/samples/sample/`) with 10 placeholder slides covering every major layout — compiled into the binary via `include_str!` (trx-jbpj.1)
- `sldr sample [--flavor X]` CLI command — renders the bundled sample deck against any flavor, opens in browser. Works offline with no slide files (trx-jbpj.1)
- `HtmlRenderer::render_sample(flavor, &[])` public helper in `sldr-renderer` (trx-jbpj.1)
- `Slide::from_str(name, virtual_path, content)` for in-memory slide construction (trx-jbpj.1)
- `sldr serve [--port N]` HTTP daemon for external agents (trx-jbpj.2). Endpoints:
  - `GET /api/health` — version + liveness
  - `GET /api/sample` — bundled markdown sources (agent slide catalog)
  - `GET /sample.html?flavor=X` — sample deck rendered against any flavor
  - `GET /api/flavors`, `GET /api/flavors/{name}` — flavor list + full schema as JSON
  - `GET /api/slides`, `GET /api/slides/{name}` — slide library introspection
  - `POST /api/slides` — create slides from `SlideInputBatch` JSON spec
  - `POST /api/assets` — base64 image upload, returns stable filename
  - `POST /api/build/{skeleton}` — trigger a build, returns output path
- Rich flavor token schema in `crates/sldr-core/src/flavor.rs` — extended from ~10 knobs to ~30 (trx-jbpj.11):
  - `ColorScheme`: surface, surface2, border, border_bright, text_dim, accent_dim, muted
  - `Typography`: heading_weight, body_weight, heading_tracking, body_tracking, heading_leading, body_leading, heading_transform, eyebrow_transform
  - New sections: `Spacing`, `Shape`, `Shadow`, `Motion`, `Decoration`, `Code`
- Per-flavor `flavor.css` escape hatch — loaded automatically alongside `flavor.toml`, inlined after generated tokens. Reserved for visual ideas tokens cannot express (decorative SVGs, magazine layouts, frame ornaments). (trx-jbpj.11)
- `editorial-serif` seed flavor with editorial flourishes via `flavor.css` (trx-jbpj.12):
  - Warm cream paper (#f5f1e8), GT Sectra serif, Solarized-light syntax theme
  - Hairline accent rule above content titles (32×2px), thicker rule above section dividers (48×3px)
  - Mono page numbers (decimal-leading-zero) in bottom-right corner, skipped on cover
  - Italic dim subtitles for cover/section, mono small-caps quote attribution
  - Bullet markers in accent color
- `MarkdownOutput::ContentImage` variant + parsing for `::content::` / `::image::` markers used by image-left/image-right layouts (trx-jbpj.13)
- Honor `[code].syntax_theme` from flavor.toml in renderer — light flavors get light code blocks (trx-jbpj.7)
- `--sldr-atmosphere` token (driven by `decoration.intensity`) — opacity multiplier for the deck-wide background glow. Flat flavors set 0; gradient/dark flavors keep the default

### Changed
- All layouts now use `justify-content: center` for content blocks — title + body flow as one centered composition (visual-explainer / Stripe Press editorial pattern). Title→body gap kept tight (8-20px margin-bottom + 16-28px flex gap). (trx-jbpj.15)
- Cover and section both left-aligned at the same column (was: cover centered, section left). Editorial coherence across hero slides.
- Two-cols layout: title and columns now flow as one centered block with tight 18px gap (was: `align-content: center` pushed cards far below the title).
- Atmospheric backgrounds unified across all slides — was rotating 3 different radial gradients per slide via `:nth-child(3n)`. Now one consistent gradient, opacity-controlled.
- Code-block frame consumes `--sldr-radius`, `--sldr-border-width`, `--sldr-border-style`, `--sldr-shadow-md` (was hardcoded 12px radius + chunky shadow).
- Default presenter nav (`.sldr-nav`) hidden in editorial-serif — replaced by editorial corner page counter.

### Fixed
- Presenter slides ghost/stuck on rapid keypress — `animationend` event doesn't fire when CSS animations are interrupted. Replaced with `setTimeout`-based cleanup that always fires. (trx-jbpj.14)
- `image-left` / `image-right` layouts emitted `::content::` and `::image::` markers as raw text — now parsed into proper column structure. (trx-jbpj.13)
- Cover slide `::after` accent glow no longer overridden by editorial page-number rule (scoped via `:not([data-layout="cover"])`).
- Two-cols `.sldr-columns` width regression — explicit `width: 100%` restored after removing `flex: 1`.

## [0.2.0] - 2026-02-17

### Added
- JSON schema generation for all config files (config.toml, flavor.toml, skeleton.toml)
- Auto-generated example configs with inline documentation and $schema references
- `schema-gen` binary for regenerating schemas and examples from Rust structs
- `just schemas` and `just copy-schemas` recipes
- Schema copy script for publishing to byteowlz/schemas repository
- `JsonSchema` derive on all config, flavor, skeleton, and slidev structs
- IDE autocompletion support via Even Better TOML extension
- `slides derive` command for AI-assisted slide generation from input files
- JSON output support (`--json` flag) for CLI commands
- Skeleton management commands (create, add, remove slides)
- Enhanced fuzzy matching with configurable resolution order
- Bundled default templates
- Background image copying for flavors during presentation build
- GitHub Actions release workflow with Homebrew and AUR publishing
- AGENTS_USE.md usage documentation

### Changed
- Workspace Cargo.toml now explicitly lists crate members
- Improved `ls` command with richer output for slides, skeletons, and flavors
- Better search with metadata and tag filtering

### Fixed
- Correct slidev frontmatter generation for per-slide layouts
- Background image paths resolved correctly during flavor asset copying

## [0.1.1] - 2025-01-30

### Added
- sldr-server crate with HTTP API endpoints for slides, skeletons, flavors, builds, and previews
- Slidev session manager with preview and template edit support
