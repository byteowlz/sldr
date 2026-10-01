---
name: sldr-parity
description: Recreate one real PowerPoint slide in sldr as closely as possible, measure the result with parity_score, and record every remaining difference with parity_gap. Use when working inside a parity case folder (a directory with case.json, original.png and extract.json).
---

# Recreating a slide in sldr (parity case)

You are in a **case folder**. Your job: make sldr render `original.png` as closely as you can, then record what you could not match. The result is not the slide; it is the **measured gap list** that shows what sldr is missing.

## What is here

- `original.png`: the slide as it should look (1920×1080). Look at it first.
- `extract.json`: the slide's structure from the PowerPoint file. Shapes with boxes in **% of the slide** (`x`, `y`, `w`, `h`), text with runs (`size_pt`, `bold`, `font`, `color`), fills, pictures (`image.file` in `media/`), tables, the layout and master shapes (`layout_shapes`), and the theme colors and fonts. Use the exact text, sizes and colors from here; do not retype from the image.
- `media/`: the slide's own pictures, exactly as embedded.
- `lib/`: **your** sldr library. Write `lib/slides/slide.md` (the playlist `case` contains just `slide`). sldr in this session sees only this library.
- `lib/flavors/` and `lib/layouts/` are **shared by every slide of this deck**. The deck's house style (background, fonts, colors, logos, footer) belongs in one flavor there, built once and reused. Layouts you add there are available to the deck's other slides.

## Procedure

1. **Read before you build.** Look at `original.png`; read `extract.json`; run `sldr ls layouts` and `sldr show layout <name>` for candidates. Decide what the slide *is* (title, bullets, image + text, diagram, chart, table, logo wall) and which layout matches its geometry.
2. **The deck's house style: reuse it, or build it once.** If `lib/flavors/` already has a flavor, use it (`flavor = "<name>"` in `lib/playlists/case.toml`). Change it only to fix something that is wrong for the whole deck, and say so in your `parity_score` note; every slide of the deck uses it. If there is none, build it from what the slide inherits: `background` and `master_shapes`/`layout_shapes` in `extract.json` hold the background art, logos with exact % positions, and footer; the theme gives fonts and colors. Name it after the deck, not the slide. `sldr show flavor default` shows the full format. Slide-specific styling belongs in the slide (or a layout), never in the shared flavor.
3. **Build the slide.** `lib/slides/slide.md` with the right `layout:`, title/subtitle/source in frontmatter, and the body. Copy pictures from `media/` into `lib/slides/media/` and reference them.
4. **Score.** Call `parity_score` (with a short `note` saying what changed). Read the numbers and **look at the image**: red marks where the slides differ.
5. **Iterate** on the biggest red regions first. Typical levers: layout choice, `type_scale`, flavor tokens (sizes, spacing, colors), a custom layout in `lib/layouts/` when no built-in fits. Stop when SSIM stops improving for two attempts, or after about eight attempts.
6. **Record every remaining difference** with `parity_gap`, one call per difference:
   - `missing_feature`: sldr cannot express it at all (e.g. a triangle diagram with labels at the corners, a native chart, text on a rotated shape).
   - `parameter`: expressible, but a value is off and you could not find the knob (e.g. logo is smaller than the original, title size).
   - `agent_error`: you could have done it but did not get there.
   - `renderer`: the original's render itself is off (e.g. a font that is not installed).
   Use **short generic feature names** that will repeat across slides; put the specifics in `evidence`.

## Rules

- Never edit sldr's source, the user's library (`~/sldr`), or anything outside this case folder.
- No names, brands, client or company details in gap records: they leave this folder.
- A custom layout that only fits this one slide is fine as a probe, but record the gap it papers over (`missing_feature`), so the real fix lands in sldr.
- Prefer exact values from `extract.json` over guesses from the image.
- Finish with a two-line summary: final SSIM and text recall, and the three most important gaps.
