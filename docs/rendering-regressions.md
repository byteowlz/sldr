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

These are browser/export regressions, not general PowerPoint compatibility or
pixel equivalence certification across different font/renderer versions. Raw
HTML, PNG, PDF and logs stay in the private output directory.
