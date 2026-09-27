---
name: use-sldr
description: Create, build, and share presentations with the sldr CLI — modular markdown slides, swappable layouts and flavors, self-contained HTML output. Use when making slides, building a talk or deck, working with sldr playlists/flavors/layouts, or asked to create a presentation.
---

# Using sldr effectively

## Quick start

```bash
sldr init                                                 # once: creates ~/sldr/{slides,playlists,flavors,layouts}
# write slide files in ~/sldr/slides/ (frontmatter + markdown body)
echo '{"name":"talk","title":"My Talk","slides":["intro","point","end"]}' | sldr playlist create
sldr build talk --flavor fjord                            # → ~/sldr/presentations/talk/ (self-contained)
sldr open talk
```

A slide file (`~/sldr/slides/point.md`):

```markdown
---
title: The one idea
subtitle: why it matters
layout: statement
---
This is **the** point.
```

## The mental model — the leverage

sldr factors a deck into four orthogonal things; that separation *is* the power:

- **Slide** — one markdown file (content). Choose its structure with `layout:` in frontmatter.
- **Layout** — structure (cover, two-cols, image-right, framed, …). Data files you can author yourself.
- **Flavor** — style (colors, fonts, background, logos). Swap the whole deck's look with one `--flavor`.
- **Playlist** — which slides, in what order: the deck definition.

Author a slide once; reuse it in any deck, restyle with any flavor, rebuild byte-identically.

## Effective use

- **Pick the layout to the content's shape**, not the reverse. One big claim → `statement`/`hero-stat`; a comparison → `versus`/`two-cols`; image + caption → `image-right`/`feature-image`; a wall of images → `image-grid`; a process → `timeline`/`agenda`. Branded decks (persistent header/footer/logos/background) → the `framed-*` family. Full catalog with "use when" in [REFERENCE.md](REFERENCE.md).
- **Frontmatter carries the chrome; markers carry structure.** `title`/`subtitle`/`source`/`source_url`/`footer` populate framed-layout slots. `::left::`/`::right::` and `::content::`/`::image::` split the body. `::lang:en::`/`::lang:de::` keep multilingual slides in one file — and an image declared *above* the language blocks is **shared** across all of them (declare it once, don't duplicate).
- **Generating slides? Batch from JSON — don't hand-write markers.** `sldr slides create` takes `{"slides":[{name,title,layout,content,…}]}`; set chrome (`subtitle`/`source`/`footer`) and a `translations` map and it writes the `::lang::`/`::content::` markers and `translations.<lang>` frontmatter for you. A bilingual content+image slide becomes a flat object with no marker syntax to get wrong. (`sldr new --scaffold translated-figure` is the copy-a-template alternative.)
- **Diagrams and graphics render — don't screenshot them.** A ` ```mermaid ` fence becomes a real diagram (offline). For vector art use `![](chart.svg)` (embedded) or inline `<svg>`. A ` ```svg `/` ```html ` fence passes through rendered, not as code. Stray/lone `::content::` markers are stripped and reported, so they never leak as literal text.
- **Restyle, don't rewrite.** `sldr build deck --flavor X` re-skins everything. Embed several (`--flavor a,b,c`) for a runtime `T`-key switcher.
- **Flavors are editable files on disk — edit them freely.** The bundled flavors install to `~/.config/sldr/flavors/<name>/` on `sldr init`; your own live in `~/sldr/flavors/<name>/` (override the bundled ones by name). They are *seeds, not sacred*: edit, rename, copy, or delete any of them. To fork, copy a flavor dir (or `sldr show flavor X` into a new `flavor.toml`) and edit. Lost or want the originals back / refreshed after an sldr upgrade? `sldr init --force` re-installs the bundled set, overwriting in place. (Layouts work the other way: built-in, read with `sldr show layout X`; drop an HTML file in `~/sldr/layouts/` to add or override one.)
- **Make decks portable.** Outputs are self-contained (media embedded). Ship fonts *in the flavor* (local `font_imports`) so they render on any machine. Default output is a directory (media streams natively); `--single-file` inlines everything into one mailable HTML; `sldr bundle` packs the editable sources as a `.sldr`.
- **Read the error.** A missing slide/flavor/layout fails loud and lists what's available — the message *is* the fix. sldr never silently substitutes. A layout whose body lacks the markers it needs (e.g. `framed-image` with no `::content::`/`::image::`) also warns at build, naming the slide and expected markers — fix the markers or pick a layout matching the body.
- **Verify visually.** Open or screenshot the deck before declaring done; structure passing ≠ looks right.
- **Reuse before you write.** `sldr search "eval harness" --long` ranks the whole library (name, title, tags, topic, description, *body*) and shows where each hit matched. A slide that already says it is one playlist line away — `sldr add talk that-slide` — never a copy.
- **Look before you edit something shared.** `sldr where my-slide` lists every deck that uses it (editing it changes all of them — that is the point, but say so). `sldr where --layout framed` lists every slide on a layout before you touch its HTML. `sldr zones my-slide` shows what fills each region of the slide and which file an edit to it belongs in (slide markdown vs flavor). `sldr layouts-for my-slide --limit 5` ranks layouts by what each would hide, fold or leave empty — arithmetic, not taste; you still choose.
- **Media lives beside the slide.** `sldr media add my-slide ~/Downloads/chart.png` copies it into that slide's `media/` and prints the reference to paste (`![…](media/chart.png)`). `sldr media ls --unused` finds orphaned files.

## Layout cheat sheet

- Title/section: `cover` `section` `intro` `statement` `hero-stat` `contact` `end`
- Body: `default` `two-cols` `two-cols-header` `pillars` `agenda` `timeline` `versus` `quote` `terminal` `split-accent`
- Image: `image` `image-center` `image-left` `image-right` `feature-image` `image-grid` `image-row` `image-portraits` `image-stack`
- Branded (persistent chrome): `framed` `framed-cols` `framed-image` `framed-figure` `framed-gallery` `framed-scatter` `framed-cover` `framed-section` `framed-full`

These four are the `sldr ls layouts` **categories** (Title&section / Body / Image / Branded). A second axis is **register**: most layouts are `classic` (predictable, boring-but-effective placement); `statement` `hero-stat` `quote` `versus` `split-accent` `terminal` `framed-scatter` are `expressive` (dramatic). Pick by *content shape*; use register as the taste filter (e.g. keep a corporate deck classic). `sldr ls layouts` prints them grouped with these tags; `--json` carries `category`+`tags`.

## Build & share

```bash
sldr ls slides|playlists|flavors|layouts     # discover what exists (names)
sldr search "query" --long                   # full-text over the library, ranked
sldr where my-slide | --layout framed        # blast radius before editing shared things
sldr zones my-slide --json                   # regions → content → which file owns it
sldr show flavor aurora                      # read a flavor's/layout's actual source
sldr show layout framed > ~/sldr/layouts/mine.html  # …or fork it as a starting point
sldr build talk --flavor aurora --lang de    # build with a flavor + language
sldr build talk --single-file                # one portable HTML file
sldr watch talk --host 0.0.0.0               # live-reload preview, reachable on the LAN
sldr bundle talk                             # → talk.sldr (editable source bundle; open with `sldr open talk.sldr`)
sldr export talk --format pdf                # PDF exit door
sldr export talk --format pptx               # native EDITABLE PowerPoint (--flatten = screenshot fallback)
sldr export --template --format pptx --flavor X   # just theme + masters, to author in PowerPoint
sldr import deck.pptx --apply --dry-run       # PowerPoint edits → which slides/zones would change
sldr import deck.pptx --apply                 # write only the edited zones back into the ORIGINAL slides
```

## See also

- [REFERENCE.md](REFERENCE.md) — full CLI, frontmatter fields, the layout catalog with when-to-use, flavor authoring, output tiers, bundles, languages, fonts.
- [EXAMPLES.md](EXAMPLES.md) — complete worked decks (simple, branded/framed, multilingual, web-clipping).
