# DeckCraft as an optional parity engine

Measured 2026-10-15 on macOS arm64. Follow-up: **trx-4s9s.24**, related to
trx-4s9s.13, trx-4s9s.22, trx-thdw and oqto-mpar.5.

**Decision:** use a pinned, separately installed DeckCraft CLI as an optional
comparison provider, following tmpltr's external WordCraft transport. Do not link
its crates, replace sldr's HTML presenter, adopt its JSON as our source format, or
claim PowerPoint fidelity from these results. `pptxr` is the approved distribution
direction, not an executable or package shipped by this spike.

## Inputs and provenance

- sldr binary: `0.10.0 (8fbbf2a7a 2026-10-06)`; checkout `8067b327d` differs
  from that source revision only in tracker migration files. This run is attributed
  to the installed binary, not a freshly rebuilt sldr.
- DeckCraft: `0.7.0`, source `9859da39255c0262cd592bb57101d7f2fa961f72`,
  `cargo build --locked -p deckcraft-cli`, unchanged source. Binary SHA-256:
  `7fde76537f9d6defa926e62dfcabceeac790a58d850095b222efa4ce99c3bca1`.
  Built without optional craft-fonts; isolated HOME supplies the explicit open faces.
- Chrome `155.0.8059.79`; LibreOffice `26.8.0.3` build
  `bce0998afefdbc355585ca324285661a2170ba77`; Poppler `26.06.0`.
- Comparator: unchanged WordCraft `xtask visual-diff` from
  `4126dce8943eb9ed3e7e9fe3dea54bcdeee5083b`. This is **not Microsoft Word**.
  It uses 8x8 grayscale SSIM windows at 100 dpi, normalized-line LCS and PDF
  text coordinates. These scores are not interchangeable with scikit-image's
  SSIM defaults in `tools/parity/parity.py`.
- Original synthetic fixture: three slides using built-in `framed`, `framed-cols`
  and `framed-image`, German text, emphasis, bullets, a wrapping paragraph and an
  original Pillow-generated three-circle PNG. No private deck or vendor assets.
  Exported with `--strict`; no flattening or allowed-loss fallback.
- Text faces: DejaVu Sans regular/bold/oblique/bold-oblique, version 2.37,
  open DejaVu/Bitstream Vera licenses. Sourced locally from LibreOffice's bundled
  fonts; no fonts redistributed here. Regular SHA-256:
  `7da195a74c55bef988d0d48f9508bd5d849425c1770dba5d7bfc6ce9ed848954`
  (see the generated `font.json` for all exact face hashes).
- Probe PPTX SHA-256:
  `293d5a7c32f2c02697a55c06311d76e0042e54cbde52a743f3b91a25faec1c21`.

The first JetBrains Mono trial was excluded: Chrome and DeckCraft selected the
requested face but this LibreOffice installation substituted unrelated fonts,
even in an explicit-font control deck. The final DejaVu pass embeds local fonts
in Chrome and exposes the same files to DeckCraft via isolated HOME. `pdffonts`
confirmed the expected text faces in all three PDFs; LibreOffice additionally
uses OpenSymbol for list glyphs. No silent font matching is assumed.

Raw PDF page sizes were Chrome **1440x810 pt**, DeckCraft **960x540 pt**, and
LibreOffice **960.009x540 pt**. The scorer requires three pages and matching 16:9
aspect ratios, then scales content and boxes to **960x540 pt** before comparing.
Without this normalization, line coordinates and raster differences are misleading.

## Results

| Reference vs candidate | Mean SSIM | Same normalized line breaks | Extracted lines |
|---|---:|---:|---:|
| Chrome vs itself (control) | 1.0000 | 100.0% | 30 / 30 |
| Chrome vs DeckCraft | 0.8515 | 83.3% | 30 / 30 |
| Chrome vs LibreOffice | 0.8516 | 80.0% | 30 / 41 |
| DeckCraft vs LibreOffice | 0.9445 | 96.7% | 30 / 41 |

The line metric is directional reference-line recall, not a symmetric identity
score. List-glyph extraction can produce extra PDF lines; more lines do not alone
prove content loss. All renders have three slides. The comparison sheet was
visually inspected: both PPTX engines show similar heading-scale, vertical-placement,
paragraph-spacing and footer-color differences from Chrome. Agreement suggests
investigating shared native mapping first; it neither diagnoses the root cause
nor establishes which engine matches PowerPoint.

For Chrome vs DeckCraft, maximum matched-line horizontal error per slide was
**7.2 / 18.3 / 56.4 pt**, median vertical error **4.1 / 18.0 / 29.9 pt** after
normalization. Slide 3 has 100% same-break recall despite major placement drift.
This is why both coordinate and raster evidence are needed.

Controls and capability checks:

- Repeated direct DeckCraft PNG renders: **3/3 byte-identical** on this platform.
- Self-comparison: SSIM 1, all line coordinates equal.
- Negative geometry controls: one-page input and 4:3 input are rejected before
  scoring, with no successful summary emitted.
- One-sentence mutation with `text.replace`: **one replacement**, SSIM **0.9972**,
  same-break **96.7%**; only slide 1 differs, slides 2 and 3 have SSIM 1. A high
  aggregate SSIM is not a safe content-correctness gate.
- `info`, `slide.inspect`, PDF conversion and direct PNG rendering work headlessly.
- `.deckcraft` save/reopen and PPTX re-export work; native envelope has `version: 1`.
  This does not prove future schema migration or arbitrary-feature preservation.
- MCP stdio initializes and lists **25 tools**; `new_presentation` and
  `run_command` / `slide.inspect` succeed without GUI or file-access grants.
- Source workspace declares `MIT OR Apache-2.0`, with both license files present.
  This is not a dependency/asset audit, independent clean-room verification or
  public-distribution/IP clearance. WASM persistence was not tested.

## Editor round-trip gate: failed

A no-edit `deckcraft-cli convert probe.pptx resaved.pptx` preserves
`customXml/item1.xml`, its properties and relationships **byte-for-byte**. It
preserves the three slide identities but drops **all 17 shape identities** under
`p:cNvPr` (identity namespace `https://sldr.dev/pptx/identity/v1`). Total slide-part
identity nodes fall from 20 to 3.

- Original `sldr import probe.pptx --apply --dry-run`: exit 0, no edits.
- Resaved `sldr import resaved.pptx --apply --dry-run`: exit 1, **17
  missing_element conflicts** and **17 unsupported/unmapped objects**.

No original Markdown was written. Keep this editor lane blocked until a provider
fix preserves identities and sldr's structural checks accept a qualified save.
Never reconstruct identity by display name, position or model guess; never weaken
fail-closed import to make a vendor round-trip appear supported.

## Reproduce the comparison

The two small scripts are **fixture-specific experiment helpers**, not the
production multi-provider adapter. They invoke a separately built comparator;
normal sldr builds/tests do not discover, build or depend on either provider.
Run against trusted, separately reviewed binaries and an isolated output directory.

```bash
# Build pinned upstream checkouts (DeckCraft revision and WordCraft comparator above).
# No desktop build, launchd service, craft-fonts download or global font install.
LAB="$HOME/sldr-lab/evaluations/deckcraft-9859da3"
D="/path/to/deckcraft/target/debug/deckcraft-cli"
X="/path/to/wordcraft/target/debug/xtask"
FONTS="/path/to/open-dejavu-fonts" # four DejaVuSans*.ttf faces

# In the respective pinned checkouts:
# cargo build --locked -p deckcraft-cli
# cargo build --locked -p xtask

# From sldr's repo:
just probe-deckcraft-prepare "$LAB" "$FONTS"
XDG_CONFIG_HOME="$LAB/config" sldr export probe --format pdf -o "$LAB/chrome.pdf"
XDG_CONFIG_HOME="$LAB/config" sldr export probe --format pptx --strict \
  --report-json "$LAB/export-report.json" -o "$LAB/probe.pptx"
HOME="$LAB/deckcraft-home" "$D" convert "$LAB/probe.pptx" "$LAB/deckcraft.pdf"
mkdir -p "$LAB/lo"
soffice "-env:UserInstallation=file://$LAB/lo-profile" --headless \
  --convert-to pdf --outdir "$LAB/lo" "$LAB/probe.pptx"
for pdf in "$LAB/chrome.pdf" "$LAB/deckcraft.pdf" "$LAB/lo/probe.pdf"; do
  pdfinfo "$pdf"
  pdffonts "$pdf" # inspect actual fonts before accepting a score
done
just probe-deckcraft-score "$LAB" "$X"
# summary.json, scores/*/result.json, diff maps, normalization.json, comparison.png

# Negative text control: compare normalized mutated.pdf with DeckCraft's baseline.
HOME="$LAB/deckcraft-home" "$D" run --in "$LAB/probe.pptx" \
  --cmd 'text.replace={"text":"First native bullet stays editable.","with":"Changed control sentence.","matchCase":true}' \
  --print --save "$LAB/mutated.pptx" --export "$LAB/mutated.pdf"

# Original must pass and no-edit resave must currently fail; dry-run never writes sources.
HOME="$LAB/deckcraft-home" "$D" convert "$LAB/probe.pptx" "$LAB/resaved.pptx"
XDG_CONFIG_HOME="$LAB/config" sldr import "$LAB/probe.pptx" --apply --dry-run
XDG_CONFIG_HOME="$LAB/config" sldr import "$LAB/resaved.pptx" --apply --dry-run \
  --report-json "$LAB/resave-import-report.json"
```

Raw evidence and validation scripts are local under the LAB directory above
(`validation.json`, `mcp.jsonl`, `identity-counts.json`, font inventories, logs,
PDFs, PNGs, original inputs and inspection output). They are not a cross-machine
artifact URL. Only the safe fixture generator, scorer and this summary are checked in.

## Adapter plan

trx-4s9s.24 should add optional subprocess providers to the existing parity lab,
not reimplement the office editor: input/version/font-keyed caches, explicit skipped
engines, structured failures/timeouts, inspectable references and pairwise scores.
Keep sldr-source-to-PPTX export parity separate from source-PPTX recreation parity.
Report engine disagreements, never majority-vote them into truth. Add independent
semantic/geometry checks and opt-in PowerPoint reference evidence before broad
compatibility claims. Packaging/license review and the editor-identity failure
remain separate gates; rendering tests do not clear them.
