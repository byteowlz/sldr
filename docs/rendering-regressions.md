# Browser and export regressions

Fast capture geometry tests live in `commands::brand::tests` and run with
`cargo test -p sldr-cli commands::brand::tests`. A fake Chrome models a dump-DOM
viewport that differs from screenshot mode. Unexpected PNG dimensions must fail,
not be cropped into a misleading successful export.

The opt-in real-browser lane needs installed Chrome/Chromium and two open DejaVu
Sans faces (`DejaVuSans.ttf`, `DejaVuSans-Bold.ttf`):

```bash
just check-rendering ~/sldr-lab/evaluations/rendering-RUN /path/to/dejavu
```

The output must be a new directory outside git; existing evidence is never
replaced. `CHROME_BIN` selects the browser for both CLI captures and browser
checks. The fixture creates its own library/configuration and uses only original
SVG/colored geometry plus the explicitly supplied open fonts. No private library,
model, automatic browser download or runtime service is needed.

## Capture (#2)

PNG at 1x and 0.5x and the native PPTX region raster must retain all four corner
markers. Image dimensions alone are insufficient: Chrome can produce the right
PNG surface while `vh` still reflects its earlier layout viewport. Export fixes
the pixel canvas before presenter initialization and checks the output dimensions.
Flattened PPTX uses the same fixed-canvas and dimension validation contract.

## Fitting and print (#3, #4)

An oversized statement must remain inside its region at 800×600 and 1920×1080,
with center, safe-center and end alignment. Measurement temporarily starts the
content at the scroll origin so negative overflow is not lost; authored alignment
is restored before scaling. Font readiness is part of fitting.

Native `page.pdf()` is tested **without** pre-emulating print media: `beforeprint`
can precede activation of print CSS. The callback must already see 1920×1080;
root-derived padding and slide units use that box. Actual print-media activation
refits again for print-specific rules. All five repeated sentence starts/ends must
survive in the single 1440×810 pt PDF page, in both browser viewports and CLI PDF
export. Screen geometry must be identical after the print cycle. The existing
`just check-print` additionally covers eight viewport/active-slide/language cases.

## Overview (#1)

The existing grid, controls, palette and tile shape are unchanged. Each preview
uses the live deck's unscaled canvas and contains it inside its tile, rather than
inheriting stale units into an unrelated 4x wrapper. Fixed dimensions preserve
framed absolute children and root-derived flavor tokens. Preview content is inert;
the existing navigation button remains operable. Applicable logos share the same
layout matching/bare-slide exclusion mechanism as print.

The browser lane captures synthetic evidence at 1920×1080, 1280×720 and 800×600;
checks full canvas bounds, column flow, fonts and exact/prefix/all logo scope;
checks live-slide state restoration, thumbnail navigation and language switching.

Studio coverage lookup returned no `sldr` app/target. This is a bounded repair of
the incumbent overview, not selection of a new visual design. The coverage gap and
bug-repair-only scope were sent to the Studio owner on the agent messageboard
(message `f0ea6adb-c04f-4418-b031-d779fbcac943`). Synthetic before/after evidence is
kept with the regression output; no private presentation is used.

## Language semantics (#5)

Both freeform layouts have Rust regression coverage for English/German variants;
their section `lang` matches `data-lang` and is inherited by spatial blocks. The
browser lane verifies `:lang(de)` on real freeform text and language attributes in
preview clones. Attribute-escaping has a negative injection control. Navigation
still uses `data-lang`; source markdown and the language-selection contract are
unchanged.

## SVG freeform pictures (#6)

Local SVG-only blocks are validated in Chrome's image context, then their real
layout is rendered with non-target blocks and chrome excluded from the transparent
crop. Original DOM indices and layout are preserved; remote resources are blocked
in this view. The CLI adapter supplies PNG bytes to the existing pure PPTX writer.
No new source syntax, writer image format or model service is involved.

The `svg_raster` finding is `baked`, not native editable vector conversion. Strict
export rejects that loss without replacing an existing destination; `--allow-lossy`
is required. Missing/invalid SVGs or failed/unavailable Chrome produce an
`unsupported` picture binding, not a successful text conversion. Mixed text/image
blocks, remote SVGs and SVGs in other native picture slots retain their prior
unsupported behavior.

The browser lane verifies the actual package picture/relationship, original block2
identity, placement within one source pixel (fractional crop quantization), expected
colored pixels, transparent empty area and editable neighboring text. An overlapping
headline/neighbor must not leak into the PNG. A malformed SVG is the negative
control. Fast tests cover local/single-image classification, absent browser and a
successful browser exit without decode evidence.

## Timeline inline markup (trx-62m5)

In `framed-timeline` an item such as `- **2001** *A Title In Italics*: and the rest`
used to render the emphasis as its own block, with the remainder on a new line.
Each item was a flex column, where every element child is a separate flex item.
An item with inline markup after the lead is now a plain block held against the
axis by block-level `align-content` (Chrome 123, Firefox 125, Safari 17.4). Items
without inline markup keep the flex column, so older browsers render them as before;
there an item with inline markup above the axis reads in one flow but starts at the
top of its half.

The `timeline` case measures, at 1920×1080 and 1280×720, that the `<em>` and the
text node after it share one line with the colon continuing where the italics end,
for an item below and an item above the axis. The plain neighbors must keep the
lead on its own line and stay against the axis.

## Logo size (trx-50bb)

`.sldr-logo` caps a logo's height at 15% of the slide. With an explicit width that
cap shrank the painted mark inside a box that still reported the requested width:
a 400×100 logo at `width = "50%"` on a 1600×900 slide painted 540×135, not 800×200.
A logo whose flavor sets a `width` is now emitted as `sldr-logo sldr-logo-sized`
and is not capped; a logo left at the default width keeps the cap. `sldr-core`
stores the width as a plain string with a `100px` default, so an explicit
`width = "100px"` cannot be told apart from an unset one and keeps the cap too.

The `logo` case measures painted pixels, not the `img` box, on a 1600×900 cover:
the live slide, native `page.pdf()` and CLI PDF export. The explicit 50% logo must
paint 800×200 at x=25%, y=60%; an 80px footer logo keeps its size and anchor; a
tall logo with no width stays capped and centered in its default box.

## SVG and HTML fence pictures (trx-xcn1)

A body segment that is exactly one ` ```svg ` or ` ```html ` fence (the languages
`sldr_core::media::fence_renders_markup` names; trailing speaker notes ignored)
is a figure in HTML. Native PPTX export used to print its markup as body text.
It now places a picture of the real render in that zone (`content`, `heading`,
`left` or `right`) and keeps every neighboring zone editable text. The fence
source is never rewritten.

The picture comes from a probe render of the same slide: comment markers around
the fence body let an injected script tag the fence's nodes, and everything else
is made invisible, not removed, so the fence keeps its real layout. The cut is
the zone box grown to everything the fence painted. A fence that stays inside
its zone lands pixel-exact; one that overflows it is scaled into the zone by the
writer instead of being clipped; a small figure is never scaled up.

The finding is `region_raster` with disposition `baked`, and the zone is owned by
the render in the provenance manifest, so `sldr import --apply` leaves it alone.
Like every baked region, strict export rejects it without publishing;
`--allow-lossy` is required. The probe render has no network access. A fence
that names a remote URL (XML namespaces excepted) gets an additional
`remote_reference` finding (`unsupported`): whatever it would load is missing
from the picture. Without Chrome/Chromium, on a failed render, or when the
fence paints nothing, the zone keeps its source text and the existing
`code_block_structure` finding; no blank picture stands in.

Unchanged: a fence beside other text in the same segment, ` ```mermaid `
(`diagram_raster`), ordinary code fences (native text) and `rep=bake` layout
regions.

`cargo test -p sldr-cli --test interchange graphic_fence` drives the public CLI
with a stand-in browser: package pictures per zone, no markup text, report
dispositions, strict rejection, an ordinary code fence as text, and an unedited
`import --apply --dry-run --strict`. `commands::export::fences::tests` covers
classification, marker placement, the cut geometry and remote-reference
accounting. Pixel equality with the PNG export, per-language renders, twin
column fences and the absence of network requests were verified with installed
Chrome on synthetic slides; that evidence stays outside git.

These are browser/export regressions, not general PowerPoint compatibility or
pixel equivalence certification across different font/renderer versions. Raw
HTML, PNG, PDF and logs stay in the private output directory.
