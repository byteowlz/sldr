# Optional multi-engine parity

`parity.py matrix` measures the same PPTX through explicit optional providers and,
optionally, a supplied Chrome/PowerPoint reference PDF. No renderer is voted the
winner. Normal sldr builds do not depend on these tools; nothing installs office
software, invokes a model, changes the source deck or enables an editor round trip.

## Export parity and recreation parity

```bash
# Export parity: generate these separately with sldr, using the same playlist/flavor.
sldr export probe --format pdf -o ~/sldr-lab/chrome.pdf
sldr export probe --format pptx --strict -o ~/sldr-lab/probe.pptx
just parity-matrix --pptx ~/sldr-lab/probe.pptx \
  --reference-pdf ~/sldr-lab/chrome.pdf --reference-engine chrome \
  --reference-version '<version that produced this PDF>' \
  --config ~/.config/sldr-parity/config.toml --output ~/sldr-lab/runs/export-01

# Recreation parity: inspect engine disagreement on the SAME original source.
just parity-matrix --pptx ~/sldr-lab/original.pptx \
  --reference-pdf ~/sldr-lab/powerpoint-reference.pdf --reference-engine powerpoint \
  --reference-version '<observed PowerPoint version>' --output ~/sldr-lab/runs/source-01
```

Omit `--reference-pdf` for a two-engine comparison. Supplied reference labels are
operator claims, not verified origin. Default engines: `deckcraft,libreoffice`;
`--engines deckcraft` explicitly selects one. At least two successful inputs are
needed for a complete comparison. The supplied reference comes first, so text
recall is measured **from** that reference; pairwise recall is directional.

For 4:3 decks use `--width 960 --height 720` (default 960x540 pt, 100 dpi).
Source package and PDF slide counts/aspect ratios must agree. Physical PDF sizes
are recorded and normalized, never compared at incompatible scales. Linked
external assets are rejected (ordinary hyperlinks are allowed); embed assets
before comparing. Limits: 200 slides, 100 MiB PDF, 16 MiB extracted text XML,
and bounded exact line matching. Raster dimensions must match the normalized
page and recorded DPI, not merely agree with each other.

Exit 0 means **measurements completed**, not visual equivalence or Office
compatibility. Exit 1 means a configured provider/tool, input or comparison failed.
Exit 2 means incomplete evidence, including an unavailable optional provider.
Runtime failures produce JSON; argparse usage errors retain standard diagnostics.
A failed provider never silently falls back to a different executable or engine.

## Discovery, configuration and trust

Requires Python packages managed by `uv` plus Poppler's `pdftoppm`, `pdftotext`,
`pdffonts`. Optional CLIs are separately installed trusted code, not sandboxed by
this harness. Treat local configuration selecting executables as trusted code,
not an authority grant. On POSIX, timeouts/cancellation kill an isolated process
group (including child processes), not the calling tmux/herdr session. Elsewhere only the direct process
is supervised. The timeout is per external command, not the entire matrix.

Configuration: `--config FILE` selects that file; otherwise local `./parity.toml`
overlays `$XDG_CONFIG_HOME/sldr-parity/config.toml` (default `~/.config/...`). The
first non-explicit-config run creates a default global TOML. The checked-in
[`config.schema.json`](config.schema.json) supplies LSP/validation metadata.

Executable precedence:

1. `--deckcraft-bin` / `--libreoffice-bin` (ingest: `--renderer-bin`).
2. A nonempty binary in an explicitly selected `--config` file.
3. `PARITY_DECKCRAFT_BIN`, `PARITY_LIBREOFFICE_BIN`, or `PARITY_PDFTOPPM_BIN`,
   `PARITY_PDFTOTEXT_BIN`, `PARITY_PDFFONTS_BIN`.
4. Nonempty local/global config defaults.
5. PATH: `pptxr` then `deckcraft-cli`, or `soffice`; Poppler uses its own names.

A selected missing/non-executable override is an error, not permission to fall
back. Paths expand `~`/environment variables for executables and font files;
relative paths use the invoking directory. No shell interprets provider arguments.
Provider HOME supports `~`; it is explicit runtime state, not a font installer.

```toml
#:schema https://raw.githubusercontent.com/byteowlz/sldr/main/tools/parity/config.schema.json
timeout_seconds = 90

[providers.deckcraft]
binary = "/path/to/pinned/deckcraft-cli"
revision = "3afb16761e47014d8077918211383634e9e4bf27"
sha256 = "" # optional required binary SHA-256; obtain from your actual build
home = "~/sldr-lab/provider-home" # optional isolated runtime/font directory

[providers.libreoffice]
binary = "soffice"
revision = "operator-supplied-build-label"

[fonts]
files = ["/path/to/open-fonts/DejaVuSans.ttf", "/path/to/open-fonts/DejaVuSans-Bold.ttf"]
expected = ["DejaVuSans", "DejaVuSans-Bold"]
```

`--timeout-secs`, repeated `--font-file`, and repeated `--expect-font` override
these settings. Expected faces are exact PostScript names with PDF subset prefixes
removed, and must be faces actually used in this deck. Missing faces fail with a
possible-substitution diagnostic. All observed faces and embedding/subset flags
are retained; extra fallback faces are not magically certified or forbidden.
Use only fonts/assets you are entitled to use. Nothing redistributes them.

The executable hash is checked before launch when pinned. Revision labels are not
source authentication; executable hashes do not cover dynamic libraries. The
provider's version, executable/tool hashes, selected runtime environment and
operator-declared font-file hashes are recorded. A font pack declares controlled
inputs; it does not prove the complete system fallback inventory. Include all
used faces (including list glyph fallbacks) when reproducibility matters.

## Evidence and caching

Every run requires a **new** output directory. Evidence and caches must be outside
git working trees; default cache: `~/sldr-lab/cache/providers` (`PARITY_LAB` applies).
No automatic publication or original-reference overwrite exists. Reports can
contain private text/paths: keep the whole lab private, not just the source deck.

```
run/
  input/<original-name>.pptx           captured common input, never the live original
  report.json                         statuses, provenance, pairwise numbers
  chrome/                             supplied source.pdf, normalized.pdf, manifest
  deckcraft/                          same, plus RGB/grayscale rasters and PDF text
  libreoffice/
  chrome-vs-deckcraft/                 result.json, per-slide diff PNGs
  chrome-vs-libreoffice/
  deckcraft-vs-libreoffice/
```

The matrix freezes one common source snapshot for every engine and records its
hash. It detects changes during capture or a provider changing that snapshot;
provider commands never receive the live original PPTX path.

Cache keys include source bytes/name, provider/tool version and executable hashes,
revision labels, declared fonts/expected faces, relevant environment, platform,
Python/dependency versions, dimensions/DPI, and adapter/scorer source hashes.
Use `--no-cache` (or `cache_enabled = false`) for dynamic/date fields or other
runtime state outside the declared inputs. Such fields/system state are not
independently certified by a content hash.
Cached artifacts are hash-checked. Changed inputs during rendering fail, and a
failed/timeout/malformed render never publishes a cache entry. Corrupt existing
cache is an explicit error, not silently trusted or overwritten. Reuse is disabled
unless both font files and expected faces are declared. Cache entries are local
integrity records, not authorization or cryptographic provenance of an upstream
source revision.

The matrix stages a complete report and publishes it without replacing any prior
run. Provider failures still get structured status in that new report when
possible; `skipped` is never a passing comparison. The supplied PDF is copied
unchanged alongside its normalized derivative.

## Metrics and attribution

`pdf_metrics.py` ports the deterministic SSIM/LCS mechanics from WordCraft's
`xtask visual-diff` at `4126dce`. [MIT notice](WORDCRAFT-LICENSE-MIT) retained.
Each report names the algorithm; this is **not** the older scikit-image scoring
mode used by `parity.py score` (which is unchanged).

- Grayscale SSIM over non-overlapping 8x8 windows at the recorded DPI.
- Full-raster share of pixels differing by >64 levels; RGB color error.
- Exact normalized-line LCS, directional same-break recall, matched-line x/y
  offsets in normalized PDF points, and page-start agreement.
- Word recall and missing words, so lost content is visible independently.
- Per-slide reference-red/candidate-blue/shared-black difference images.

No threshold chooses the better renderer. High whole-slide SSIM can hide a missing
sentence or picture. Coordinate checks use same-page matches; equal text moved to
another slide is not treated as a valid position match. Unlike the upstream
comparator, unequal counts/dimensions and excessive LCS work fail loudly rather
than scoring a common area or silently using a partial match. Slides with no
reference text report null line/word recall, not a fabricated success.

## Existing ingest workflow

Default `ingest` remains LibreOffice. Optional selections:

```bash
uv run tools/parity/parity.py ingest deck.pptx --renderer deckcraft \
  --config /path/to/pinned-provider.toml --case-prefix probe-deckcraft
uv run tools/parity/parity.py ingest deck.pptx --reference-pdf observed.pdf \
  --reference-engine powerpoint --case-prefix probe-powerpoint
```

New case JSON carries actual provider provenance. Existing `original.pdf`
snapshots without metadata remain usable only via the default legacy path and
are labelled `legacy-unverified`, not mislabelled as newly verified LibreOffice.
Changing a source/provider/reference or its captured runtime/adapter identity
requires a new case prefix; `--force` recreates case folders, **not** the reference PDF. Never turn a supplied PowerPoint
reference into an unnamed headless substitute.

## Verification

```bash
just check-parity # fake CLIs/Poppler; no installed office apps needed
just check-parity-real /path/probe.pptx /path/chrome.pdf /path/config.toml \
  /path/wordcraft/target/debug/xtask ~/sldr-lab/runs/real-01
```

The opt-in real lane runs the matrix and compares summary/per-slide SSIM, line and
coordinate metrics with the separately built pinned WordCraft comparator, using
identical Poppler executables. It writes `metric-equivalence.json`; it does not
certify general Office compatibility. The original synthetic fixture and pinned
provider preservation patch are documented in
[the spike report](../../docs/pptx-spike/deckcraft-parity.md).
