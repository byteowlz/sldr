# /// script
# dependencies = ["playwright==1.55.0", "pymupdf==1.26.4"]
# ///
"""Run with uv run scripts/check-print.py HTML OUTPUT_DIR. Uses installed Chrome."""
import json
import pathlib
import sys

import fitz
from playwright.sync_api import sync_playwright

html = pathlib.Path(sys.argv[1]).resolve()
out = pathlib.Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
results = []
with sync_playwright() as p:
    browser = p.chromium.launch(channel="chrome", headless=True)
    for width, height in [(800, 600), (1920, 1080)]:
        for active in [1, 2]:
            for lang in ["en", "de"]:
                page = browser.new_page(viewport={"width": width, "height": height})
                page.add_init_script(f"localStorage.setItem('sldr-lang', '{lang}')")
                page.goto(html.as_uri() + f"#{active}")
                page.evaluate("document.fonts.ready")
                screen = page.locator('.sldr-slide.active .sldr-frame-head').bounding_box()
                page.emulate_media(media="print")
                page.evaluate("window.dispatchEvent(new Event('beforeprint'))")
                metrics = page.evaluate("""() => [...document.querySelectorAll('.sldr-slide')]
                    .filter(s => getComputedStyle(s).display !== 'none').map(s => ({
                        lang: s.dataset.lang,
                        unit: getComputedStyle(s).getPropertyValue('--sldr-u'),
                        fonts: ['.sldr-headline', '.sldr-frame-body p', '.sldr-footer']
                            .map(sel => { const e = s.querySelector(sel); return e && getComputedStyle(e).fontSize; }),
                        logos: s.querySelectorAll('.sldr-logo').length
                    }))""")
                pdf = out / f"{width}-{active}-{lang}.pdf"
                page.pdf(path=str(pdf), prefer_css_page_size=True, print_background=True)
                with fitz.open(pdf) as doc:
                    assert len(doc) == 2, (pdf, 'page count', len(doc))
                    sizes = []
                    for sheet in doc:
                        assert abs(sheet.rect.width - 1440) < 1 and abs(sheet.rect.height - 810) < 1
                        spans = [span for block in sheet.get_text('dict')['blocks'] if 'lines' in block
                                 for line in block['lines'] for span in line['spans']]
                        title = 'Title sentinel' if lang == 'en' else 'Titelprüfung'
                        found = [s['size'] for s in spans if title in s['text']]
                        assert found, (pdf, title, spans)
                        sizes.append(found[0])
                    assert abs(sizes[0] - sizes[1]) < 0.1, (pdf, 'unequal titles', sizes)
                assert len(metrics) == 2 and all(m['lang'] == lang for m in metrics), metrics
                assert all(m['logos'] == 5 for m in metrics), metrics
                assert metrics[0]['fonts'] == metrics[1]['fonts'], metrics
                assert all(metrics[0]['fonts']), metrics
                page.emulate_media(media="screen")
                page.evaluate("window.dispatchEvent(new Event('afterprint'))")
                assert page.locator('.sldr-slide.active .sldr-frame-head').bounding_box() == screen
                results.append({'viewport': [width, height], 'active': active, 'lang': lang,
                                'metrics': metrics, 'title_pt': sizes, 'pdf': pdf.name})
                page.close()
    (out / 'evidence.json').write_text(json.dumps({'renderer': browser.version, 'runs': results}, indent=2))
    browser.close()
print(f"PASS: {len(results)} viewport/active-slide/language combinations; evidence in {out}")
