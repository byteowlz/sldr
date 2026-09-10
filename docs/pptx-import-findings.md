# PPTX → sldr: findings from a real-deck port

Source: a 46-slide Fraunhofer IEM talk (*Generative AI*, Goerdeler-Gymnasium,
April 2026) ported by hand into 44 sldr slides + 5 new framed layouts.
The port is the feasibility test for "replace PowerPoint"; this document
records what the port taught us about a future **automated** `pptx → sldr`
importer (and what an exporter must preserve for the round trip).

Companion: `docs/pptx-spike/FINDINGS.md` covers the export direction
(hand-built OOXML opens cleanly).

## 1. What was in the deck

| Aspect | Count | Notes |
|---|---|---|
| Slides | 46 | 2 dropped in the port (duplicate section divider, empty spacer) |
| Slide layouts referenced | 26 | Mostly numbered copies (`7_Text 1-spaltig`, `2_Benutzerdefiniertes Layout`…) of **4 real** designs: title, section, one-column body, blank |
| Media parts | 142 | 72 png · 33 jpeg · 31 svg · 5 mp4 · 1 wdp (HD Photo) |
| Slides with `svgBlip` | 11 | Vector logos/icons; PPTX stores a PNG fallback **and** the SVG |
| Slides with groups (`grpSp`) | 17 | Hand-assembled diagrams (Venn, bell curve, pipeline boxes) |
| Slides with `custGeom` | 6 | Freeform shapes — all decorative |
| Slides with video | 6 | `videoFile` rel + poster `blip` |
| Speaker notes | ~10 | Free-form; some were the source lines |

The theme (`ppt/theme/theme1.xml`) carries the corporate palette; the master
carries logos, footer text and the dune-photo background — i.e. exactly the
things a sldr **flavor** owns. The port used `flavors/fraunhofer-ea`, which
already mirrored the master (background image, 4 logos, © footer).

## 2. Mapping that worked (deterministic)

| PPTX | sldr | Confidence |
|---|---|---|
| `theme1.xml` `clrScheme` + fonts | `flavor.toml` `[colors]` / `[fonts]` | high — 1:1 |
| Master background picture / logos / footer text | `flavor.toml` `background`, `[[logos]]`, `footer` | high |
| Title placeholder (`ph type="title"`) | frontmatter `title` | high |
| Subtitle / `body idx=3` at the head zone | frontmatter `subtitle` | high (by geometry: y < 22 %) |
| Body placeholder paragraphs with `buChar` / `buAutoNum` / `buNone` | `- ` / `1. ` / plain paragraph, `lvl` → indent | high |
| `b="1"` run at paragraph start | `**lead**` (becomes card/timeline label) | high |
| Single full-width picture (+ optional `<a:hlinkClick>` or notes URL) | `framed-full` + `source` / `source_url` | high |
| Picture right of a text placeholder | `framed-image` (`::content::` / `::image::`) | high |
| 2–4 pictures of equal size on one row | `framed-strip` (caption = picture `descr` or nearby text box) | high |
| Portrait picture + quote text box + name line | `framed-quote` | medium (needs the "quote" heuristic: `“…”` or `»…«`) |
| Portrait + name + role + e-mail + link (last slide) | `framed-contact` | high (last slide, `mailto:` hyperlink) |
| 3–6 equal-size rounded rectangles with bold first line | `framed-cards` | medium |
| Line + N text boxes with a year as first run | `framed-timeline` | medium |
| `videoFile` rel + poster `blip` | `![alt](clip.mp4 "poster.png")` | high |
| Slide notes (`notesSlide`) | `<!-- notes: … -->` | high |
| `svgBlip` | copy the `.svg`, ignore the PNG fallback | high |
| `.wdp` (HD Photo) | convert to PNG at import (no browser support) | high |

The **geometry-first** approach is the key: every framed layout already
declares its zones (`<!-- sldr:zone name=… x y w h -->`). Classifying a slide
= scoring each candidate layout by how well the slide's shapes cover that
layout's zones (IoU per zone, penalty for uncovered shapes). This is the same
zone table the PPTX *exporter* writes, so the round trip is symmetric by
construction.

## 3. What did not map — and what we did instead

1. **Hand-drawn diagrams** (groups of shapes, connectors, freeforms). 17 slides.
   Options, in order of fidelity vs. maintainability:
   - re-author as **mermaid** (flowcharts, sequence diagrams) — done for 4 slides;
   - re-author as an **inline `svg` fence** using flavor variables
     (`var(--sldr-accent)`) so it re-themes — done for 4 slides (Venn, bell
     curve, T-shaped vs rake-shaped skills);
   - re-author as an **`html` fence** with scoped CSS — done for the
     next-token-prediction slide;
   - fallback for an importer: **rasterise the group** via the PPTX export
     renderer and embed as an image, keeping the original group XML as
     provenance (`<!-- sldr:pptx-shape sha=… -->`) so an exporter can put the
     editable original back. This is the ADR-0002 "edits flow back along
     provenance" case.
2. **Mixed-size picture collages** (3–10 pictures scattered on a slide).
   `framed-scatter` covers the "one hero + N thumbnails" pattern; true
   free-form collages are not worth a layout — the importer should emit
   `framed-gallery` and accept the re-flow.
3. **Text boxes that are really captions** (below/next to a picture, no
   placeholder). Heuristic: a text box whose bounding box touches a picture's
   bottom edge and is ≤ 2 lines → image title/caption.
4. **Per-slide overrides** of master chrome (hidden logo, different footer).
   sldr expresses these as frontmatter (`footer:`), not per-slide shapes.
5. **Animations / build steps** — not present in this deck; out of scope
   (ADR-0004).
6. **German ↔ English** — the deck was German; the port is English. An
   importer must not translate; it should emit `lang` on the slide and let the
   multi-language build handle the rest.

## 4. Layout gaps this port closed

Five layouts were missing and now exist as built-ins (`framed-cards`,
`framed-timeline`, `framed-strip`, `framed-quote`, `framed-contact`), each with
zones so they export back to PPTX placeholders. Base CSS learned:

- images/video/mermaid inside `framed-cols` / `framed-image` columns now shrink
  to the body height instead of running over the footer;
- a `source:` line lowers the body bottom so it never overlaps content;
- mermaid diagrams fill the framed body (they rendered at 16 px type before).

Remaining ergonomics for authors coming from PowerPoint: a way to force a
column ratio (`framed-cols` is always 50/50), and an image-left variant of
`framed-image` (today: swap markers is not enough).

## 5. Recommended importer architecture

```
pptx ─► parse (roxmltree)  ─► ShapeInventory per slide
                                 │  (placeholder type, bbox %, text runs,
                                 │   picture rel, group tree, hyperlinks)
                                 ▼
                        classify(slide, layouts) ─► (layout, slot→shapes, score)
                                 │
                                 ▼
                     emit .md + media/ + provenance comments
                                 │
                                 ▼
             theme + master ─► flavor.toml / assets  (once per deck)
```

- **Phase A – lossless dump** (`sldr import deck.pptx --dump`): flavor from
  theme/master, one `.md` per slide with `layout: framed-full` and a rendered
  PNG of the slide. Useless for editing but proves the pipeline and gives a
  visual diff baseline.
- **Phase B – placeholder slides**: slides whose shapes are *only*
  placeholders + ≤ 1 picture → `framed`, `framed-image`, `framed-cover`,
  `framed-section`, `framed-full`. In this deck that is 24/46 slides.
- **Phase C – pattern slides**: the heuristics in §2 for strip/cards/
  timeline/quote/contact. Another 12/46 here.
- **Phase D – everything else**: rasterised group + provenance, flagged in the
  import report so a human (or an agent with the `use-sldr` skill) can
  re-author it as mermaid/svg. 10/46.

An import report (`import-report.md`) listing per slide: chosen layout, score,
dropped shapes, and "needs re-authoring" flags is the deliverable that makes
the importer usable at all; nobody will review 46 slides blind.

## 6. Round-trip contract (what the exporter must keep)

- Zone tables in layouts are the shared coordinate system — never let a
  layout drift from its zone comment.
- `source` / `source_url` export as a small text box at the source zone with
  the hyperlink on it; the importer reads it back by position.
- Media are emitted by content hash so re-import de-duplicates.
- Slides re-authored as mermaid/svg/html export as pictures with the fence
  text in the shape's `descr` (alt text) — enough to recover the source.
