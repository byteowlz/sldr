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
    report = {"capture": capture(f)}
    (root / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"status": "passed", "report": str(root / "report.json")}))


if __name__ == "__main__":
    main()
