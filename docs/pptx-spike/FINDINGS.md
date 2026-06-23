# PPTX spike (trx-4s9s.1) — does hand-built OOXML open?

**Result: YES.** A 16-part hand-built `.pptx` (theme from `example-flavor`, one
slideMaster, two slideLayouts — `framed` + `two-cols` with EMU-positioned
placeholders — and one filled slide) opened in LibreOffice Impress with **no
repair prompt** and rendered correctly: dark theme background, white title at
the framed head zone, body bullets at the body zone. See `rendered.png`.

`gen.py` is the generator; `parts/` is the extracted, well-formed package
(the template the Phase-1 Rust generator emits). The mapping is confirmed
deterministic: flavor colors → `clrScheme`, `%` → EMU, sldr zones → positioned
placeholders.

## Validators used
- `xmllint` — all 16 parts well-formed.
- `unzip -t` — zip OK.
- `soffice --headless --convert-to pdf` — opened & rendered (proxy for "opens
  without repair"). **Still pending: open in actual PowerPoint on macOS** —
  PowerPoint is stricter than LibreOffice on a few points (below).

## Checklist — what OOXML is strict about (de-risk notes for Phase 1)
- `[Content_Types].xml` must declare a content type for **every** part:
  `Default` for `rels`+`xml`, an `Override` per slide/layout/master/theme/
  presentation/presProps/core/app. A missing Override → repair.
- Every part with relationships needs its `_rels/<name>.rels`; the
  `Relationship Id`s must match every `r:id` reference exactly.
- `presentation.xml`: `sldMasterIdLst` + `sldIdLst` + `sldSz` (use
  `type="screen16x9"`, cx=12192000 cy=6858000) + `notesSz`.
- **Theme must carry a complete `fmtScheme`** — 3 each of fill/line/effect/
  bgFill styles. LibreOffice tolerates a minimal one; **PowerPoint is stricter
  — verify on the Mac test.**
- `slideMaster`: `clrMap`, `sldLayoutIdLst`, and `txStyles`
  (title/body/other) are all required.
- `slideLayout`: `type` attr (`obj`, `twoObj`, …), `clrMapOvr`, and
  placeholders with `<p:ph type=… idx=…>`. The `idx` must be consistent
  between the layout placeholder and the slide that fills it.
- Shape ids (`cNvPr id`) unique within a part; id `1` is the group, content
  starts at `2`.
- A slide inherits placeholder geometry from its layout — the slide's filling
  `<p:sp>` can omit `spPr` and just provide the `txBody`.
- EMU positioning is `<a:xfrm><a:off x/y><a:ext cx/cy></a:xfrm>`; 1 inch =
  914400 EMU, slide width = 12192000 EMU.

## Verdict
Generation risk is **low** — the structure is proven and deterministic. The
remaining unknown is PowerPoint-specific strictness (mainly a complete
`fmtScheme` and possibly `app.xml`/`docProps` fields); confirm with one open
in real PowerPoint before Phase 1 builds on top.

## Bullets (finding)
OOXML bullets aren't automatic — a paragraph shows one only if its `<a:pPr>` defines it: `<a:buChar char="&#8226;"/>`+`<a:buFont>` (bulleted), `<a:buAutoNum/>` (numbered), or `<a:buNone/>` (none); `marL`/`indent` give the hanging indent, `lvl` the nesting. The markdown->OOXML converter (trx-4s9s.4) sets this per paragraph: `- item`->buChar, `1. item`->buAutoNum, indent->lvl, plain text/heading->buNone. Do it per-paragraph, not via master defaults, since a body mixes bullets and plain text. The spike now demonstrates all three.
