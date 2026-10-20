# /// script
# requires-python = ">=3.11"
# dependencies = ["playwright==1.55.0", "pillow==12.3.0", "pymupdf==1.26.4"]
# ///
"""Real installed-Chrome regression lane. Synthetic/private output, no model calls."""

import argparse
import io
import json
import os
from pathlib import Path
import shutil
import sys
import zipfile

from PIL import Image
import fitz
from playwright.sync_api import sync_playwright

sys.dont_write_bytecode = True
from rendering_fixture import Fixture, prepare


def corners(image):
    image = image.convert("RGB")
    w, h = image.size
    actual = [
        image.getpixel((int(w * x), int(h * y)))
        for x, y in [(0.02, 0.02), (0.98, 0.02), (0.02, 0.98), (0.98, 0.98)]
    ]
    assert actual == [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0)], actual


def capture(f):
    for scale in [1, 0.5]:
        out = f.root / f"png-{scale}"
        f.run(
            f"capture-{scale}",
            "export",
            "markers",
            "--format",
            "png",
            "--scale",
            scale,
            "-o",
            out,
        )
        with Image.open(out / "01.png") as image:
            assert image.size == (int(1920 * scale), int(1080 * scale)), image.size
            corners(image)
    out = f.root / "region.pptx"
    f.run("region", "export", "markers", "--format", "pptx", "--allow-lossy", "-o", out)
    with zipfile.ZipFile(out) as package:
        full = []
        for name in package.namelist():
            if name.startswith("ppt/media/") and name.endswith(".png"):
                image = Image.open(io.BytesIO(package.read(name)))
                if image.size == (1920, 1080):
                    full.append(image)
        assert len(full) == 1, len(full)
        corners(full[0])
    return {"png_scales": [1, 0.5], "native_region_corners": "passed"}


def body_bounds(page):
    return page.locator(
        '.sldr-deck > .sldr-slide[data-layout="statement"] .sldr-content'
    ).evaluate("""e => {
        const transform = e.style.transform;
        e.style.transform = 'none';
        const region = e.getBoundingClientRect();
        e.style.transform = transform;
        const range = document.createRange(); range.selectNodeContents(e);
        const text = range.getBoundingClientRect();
        return {region: [region.top, region.bottom], text: [text.top, text.bottom], transform};
    }""")


def screen_fit(f):
    f.run("build-fit", "build", "statement", "--single-file")
    rows = []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=str(f.browser), headless=True)
        for width, height in [(800, 600), (1920, 1080)]:
            for alignment in ["center", "safe center", "flex-end"]:
                page = browser.new_page(viewport={"width": width, "height": height})
                page.goto((f.root / "out/statement/index.html").as_uri())
                page.evaluate("document.fonts.ready")
                page.add_style_tag(
                    content=f".sldr-content {{justify-content:{alignment}!important}}"
                )
                page.evaluate("window.__sldrFitAll()")
                bounds = body_bounds(page)
                assert bounds["text"][0] >= bounds["region"][0] - 2, (
                    alignment,
                    width,
                    bounds,
                )
                assert bounds["text"][1] <= bounds["region"][1] + 2, (
                    alignment,
                    width,
                    bounds,
                )
                rows.append(
                    {"viewport": [width, height], "alignment": alignment, **bounds}
                )
                page.close()
        browser.close()
    return rows


def pdf_words(path):
    with fitz.open(path) as pdf:
        assert len(pdf) == 1, (path, len(pdf))
        assert abs(pdf[0].rect.width - 1440) < 1 and abs(pdf[0].rect.height - 810) < 1
        text = pdf[0].get_text()
        assert text.count("Complete") == 5, (path, text)
        assert text.count("declaration") == 5, (path, text)


def print_fit(f):
    f.run("build-print", "build", "statement", "--single-file")
    rows = []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=str(f.browser), headless=True)
        for width, height in [(800, 600), (1920, 1080)]:
            page = browser.new_page(viewport={"width": width, "height": height})
            page.goto((f.root / "out/statement/index.html").as_uri())
            page.evaluate("document.fonts.ready")
            before = body_bounds(page)
            # Deliberately do NOT emulate print first: native beforeprint precedes
            # print-media activation on real Chrome. This is the reported seam.
            page.evaluate("""window.addEventListener('beforeprint', () => {
                const s=document.querySelector('.sldr-deck > .sldr-slide');
                window.qaPrint = {width:s.clientWidth,height:s.clientHeight};
            })""")
            path = f.root / f"print-{width}.pdf"
            page.pdf(path=str(path), prefer_css_page_size=True, print_background=True)
            measured = page.evaluate("window.qaPrint")
            assert measured == {"width": 1920, "height": 1080}, measured
            pdf_words(path)
            after = body_bounds(page)
            assert before == after, (
                "screen geometry changed after printing",
                before,
                after,
            )
            rows.append({"viewport": [width, height], "beforeprint": measured})
            page.close()
        browser.close()
    f.run("cli-pdf", "export", "statement", "--format", "pdf", "-o", f.root / "cli.pdf")
    pdf_words(f.root / "cli.pdf")
    return rows


def overview(f):
    f.run("build-overview", "build", "all", "--lang", "en,de", "--single-file")
    rows = []
    with sync_playwright() as p:
        browser = p.chromium.launch(executable_path=str(f.browser), headless=True)
        page = browser.new_page(viewport={"width": 1920, "height": 1080})
        page.goto((f.root / "out/all/index.html").as_uri())
        page.evaluate("document.fonts.ready")
        original = page.locator(".sldr-deck > .sldr-slide").evaluate_all(
            "es => es.map(e=>e.getAttribute('style'))"
        )
        page.keyboard.press("o")
        for width, height in [(1920, 1080), (1280, 720), (800, 600)]:
            page.set_viewport_size({"width": width, "height": height})
            page.evaluate("document.fonts.ready")
            # Dispatch synchronously so layout assertions do not race resize delivery.
            page.evaluate("window.dispatchEvent(new Event('resize'))")
            metrics = page.locator(".sldr-overview-thumb").evaluate_all("""es => es.map(e=>{
                const s=e.querySelector('.sldr-slide'),r=s.getBoundingClientRect(),b=e.getBoundingClientRect();
                return {layout:s.dataset.layout,lang:s.dataset.lang,width:s.clientWidth,height:s.clientHeight,
                    box:[r.left,r.top,r.right,r.bottom],button:[b.left,b.top,b.right,b.bottom],
                    logos:s.querySelectorAll('.sldr-logo-on').length,
                    direction:getComputedStyle(s).flexDirection,
                    family:getComputedStyle(s).fontFamily};
            })""")
            page.screenshot(path=str(f.root / f"overview-{width}.png"))
            (f.root / f"overview-{width}.json").write_text(
                json.dumps(metrics, indent=2)
            )
            assert len(metrics) == 6, len(metrics)
            for row in metrics:
                assert row["lang"] == "en", row
                assert (row["width"], row["height"]) == (width, height), row
                assert row["direction"] == "column" and "Probe" in row["family"], row
                box, button = row["box"], row["button"]
                assert box[0] >= button[0] and box[1] >= button[1], row
                assert box[2] <= button[2] + 1 and box[3] <= button[3] + 1, row
                assert row["logos"] == (
                    1 if row["layout"] in ("framed", "framed-cols") else 0
                ), row
            rows.append({"viewport": [width, height], "thumbnails": metrics})
        page.keyboard.press("Escape")
        # Return to the initial viewport before comparing the live deck's styles.
        page.set_viewport_size({"width": 1920, "height": 1080})
        page.evaluate("window.dispatchEvent(new Event('resize'))")
        after = page.locator(".sldr-deck > .sldr-slide").evaluate_all(
            "es => es.map(e=>e.getAttribute('style'))"
        )
        assert original == after, ("overview mutated live slides", original, after)
        page.keyboard.press("o")
        page.locator(".sldr-overview-thumb").nth(2).click()
        page.wait_for_function(
            "document.querySelectorAll('.sldr-deck > .sldr-slide.active').length === 1"
        )
        assert (
            page.locator(".sldr-deck > .sldr-slide.active").get_attribute("data-layout")
            == "framed"
        )
        assert page.locator(".sldr-overview").get_attribute("aria-hidden") == "true"
        page.keyboard.press("l")
        page.keyboard.press("o")
        assert (
            page.locator('.sldr-overview-thumb .sldr-slide[data-lang="de"]').count()
            == 6
        )
        for allowed, expected in [
            ("framed*", [0, 0, 1, 1, 1, 1]),
            ("all", [0, 1, 1, 1, 1, 1]),
        ]:
            page.keyboard.press("Escape")
            page.locator(".sldr-deck > .sldr-logos .sldr-logo").evaluate_all(
                '(es, value) => es.forEach(e=>e.setAttribute("data-logo-layouts",value))',
                allowed,
            )
            page.keyboard.press("o")
            counts = page.locator(".sldr-overview-thumb").evaluate_all(
                'es => es.map(e=>e.querySelectorAll(".sldr-logo-on").length)'
            )
            assert counts == expected, (allowed, counts)
        browser.close()
    return rows


def check_selected(f, selected):
    report = {}
    if selected in ("all", "capture"):
        report["capture"] = capture(f)
    if selected in ("all", "screen-fit"):
        report["screen-fit"] = screen_fit(f)
    if selected in ("all", "print-fit"):
        report["print-fit"] = print_fit(f)
    if selected in ("all", "overview"):
        report["overview"] = overview(f)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output", type=Path, help="NEW directory outside git; never overwritten"
    )
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "target/debug/sldr",
    )
    parser.add_argument(
        "--font-dir",
        type=Path,
        required=True,
        help="Open DejaVu Sans regular/bold faces",
    )
    parser.add_argument("--browser", type=Path, default=None)
    parser.add_argument(
        "--case",
        choices=["all", "capture", "screen-fit", "print-fit", "overview"],
        default="all",
    )
    args = parser.parse_args()
    browser = (
        args.browser
        or os.environ.get("CHROME_BIN")
        or shutil.which("google-chrome")
        or shutil.which("chromium")
        or Path("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
    )
    browser = Path(browser).resolve()
    assert browser.is_file(), (
        "Install Chrome/Chromium or pass --browser; no automatic downloads"
    )
    root = args.output.expanduser().resolve()
    assert not any((p / ".git").exists() for p in [root, *root.parents]), (
        "Evidence must stay outside git"
    )
    root.mkdir(parents=True, exist_ok=False)
    prepare(root, args.font_dir.resolve())
    f = Fixture(root, args.binary.resolve(), browser)
    report = check_selected(f, args.case)
    (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": "passed", "report": str(root / "report.json")}))


if __name__ == "__main__":
    main()
