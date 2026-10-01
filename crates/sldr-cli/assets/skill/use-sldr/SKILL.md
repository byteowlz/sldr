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
- Escape hatch: `freeform` / `framed-freeform` place `::block x= y= w= h=::` markdown blocks at exact spots; for the one slide no layout fits, never the default.
- Branded (persistent chrome): `framed` `framed-cols` `framed-image` `framed-figure` `framed-gallery` `framed-scatter` `framed-cover` `framed-section` `framed-full` — and the diagram bodies `framed-cards` (N options as cards, optional logos on top) `framed-rows` (N questions/claims/terms as an editorial table: lead left, context right) `framed-flow` (a process as boxes and arrows) `framed-timeline` (dated milestones on an axis) `framed-quote` (one big quotation) `framed-strip` (2–5 captioned images side by side) `framed-contact` (closing slide)

These four are the `sldr ls layouts` **categories** (Title&section / Body / Image / Branded). A second axis is **register**: most layouts are `classic` (predictable, boring-but-effective placement); `statement` `hero-stat` `quote` `versus` `split-accent` `terminal` `framed-scatter` `framed-quote` are `expressive` (dramatic). The list above can lag the binary — `sldr ls layouts` is the truth. Pick by *content shape*; use register as the taste filter (e.g. keep a corporate deck classic). `sldr ls layouts` prints them grouped with these tags; `--json` carries `category`+`tags`.

## A deck worth watching — not a template

Most agent decks fail the same way: every slide is title + five bullets. Avoid it deliberately.

- **Headline = the claim, not the topic.** "A classifier is not a firewall", not "Security results". The subtitle carries the context (who, when, sample size).
- **Map the content's shape to a layout, every slide:** a process → `framed-flow` (mark the key step `- [x] **Step**`) · dated events → `framed-timeline` · N questions, claims or terms with context → `framed-rows` · N players/options with logos → `framed-cards` · one number → `hero-stat` · one thesis → `statement` · two approaches → `versus` · one quote → `framed-quote` · an article as evidence → `framed-image` (clip right, 3–4 bullets *interpreting* it left) · several articles → `framed-scatter` · commands/demo → `terminal`. Plain `framed` bullets are the fallback, never three in a row.
- **No decoration for its own sake:** don't add colored accent stripes, thick borders or boxes around text that reads fine as a list — text lists want `framed-rows`, not cards. Fill the slide with type (`type_scale: 1.2` on a sparse slide) rather than leaving a floating band in a sea of empty space.
- **Give it rhythm:** section dividers (`framed-section`) between acts; alternate dense and sparse; each act gets one piece of evidence, one punch (stat or statement) and one diagram; end on questions (`framed-cards`), then contact.
- **Every number carries its source** (`source` + `source_url`), and the honest caveat sits on the same slide ("BM25 gets 51% at 85× the speed"). Credibility is the style.
- **Clips (article screenshots):** capture at 2× — `agent-browser set viewport 1440 900 2`, then `open URL` and `screenshot /absolute/path.png` (relative paths fail silently). Crop to headline + the paragraph or figure that matters (`magick in.png -crop WxH+X+Y +repage out.png`). **Open every PNG and look**: cookie banners and "Just a moment…" bot walls produce a screenshot that is useless — dismiss (`find text "Reject" click`) or pick another source.
- **Logos:** dark flavor → light variants (svgl `*_dark`/`*_white` files, raw from `github.com/pheralb/svgl/…/static/library/<name>.svg`; simple-icons as fallback — recolor unfilled/`currentColor` marks). Rasterize to PNG (`rsvg-convert -h 256`) — PowerPoint export skips SVG. Put ≤ 3 logos at the start of a `framed-cards` item.
- **Length budgets** (they are what keeps layouts intact): headline ≤ ~45 characters (PowerPoint does not shrink it) · timeline item ≤ ~70 · card body ≤ ~35 words · terminal lines ≤ ~60 · `framed-scatter` right side 3 images looks best.
- **Reuse and translate, don't copy:** `sldr search`, then add `translations.<lang>` + `::lang::` blocks to the existing slide so the library gains a language.
- **Verify every slide as images, not just the build:** `sldr export deck --format pdf`, then `pdftoppm -r 60 -png deck.pdf s && magick montage s-*.png -tile 3x4 -geometry +4+4 sheet.png` and read the sheet. For PowerPoint: `soffice --headless --convert-to pdf deck.pptx` and look again. Common defects: text overflowing into the footer, tiny collage images, logos wrapping, a clip that is a cookie wall.

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
