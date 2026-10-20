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

These are browser/export regressions, not general PowerPoint compatibility or
pixel equivalence certification across different font/renderer versions. Raw
HTML, PNG, PDF and logs stay in the private output directory.
