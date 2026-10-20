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

These are browser/export regressions, not general PowerPoint compatibility or
pixel equivalence certification across different font/renderer versions. Raw
HTML, PNG, PDF and logs stay in the private output directory.
