"""PDF preparation and attributed metrics, not a visual-quality verdict.

SSIM/LCS mechanics port WordCraft xtask visual_diff.rs at 4126dce (MIT OR
Apache-2.0); see WORDCRAFT-LICENSE-MIT. Unlike its common-area fallback we reject
unequal page counts/dimensions and fail loudly when the LCS work bound is exceeded.
"""

from pathlib import Path
from dataclasses import dataclass
import math
import re
import xml.etree.ElementTree as ET

import numpy as np
from PIL import Image
from pypdf import PdfReader, PdfWriter

ALGORITHM = "wordcraft-4126dce-ssim8-lcs/pdf-normalized-v1"


class PdfError(ValueError):
    def __init__(self, code, detail):
        super().__init__(detail)
        self.code = code


def parse_lines(xml):
    result = []
    try:
        root = ET.fromstring(xml)
    except ET.ParseError as exc:
        raise PdfError("text_layer", f"Invalid Poppler text layer: {exc}") from exc
    local = lambda el: el.tag.rsplit("}", 1)[-1]
    for page, body in enumerate(e for e in root.iter() if local(e) == "page"):
        for line in (e for e in body.iter() if local(e) == "line"):
            words = [e for e in line.iter() if local(e) == "word"]
            if words:
                x, y = float(words[0].attrib["xMin"]), float(words[0].attrib["yMin"])
                if not math.isfinite(x + y):
                    raise ValueError("Non-finite PDF text coordinates")
                result.append(
                    {
                        "page": page,
                        "x": x,
                        "y": y,
                        "text": " ".join("".join(w.itertext()) for w in words),
                    }
                )
    if len(result) > 50000:
        raise ValueError("PDF text line limit exceeded (50000)")
    return result


def parse_fonts(text):
    records = []
    for row in text.splitlines():
        fields = row.split()
        if len(fields) < 8 or not fields[-1].isdigit() or not fields[-2].isdigit():
            continue
        records.append(
            {
                "name": re.sub(r"^[A-Z]{6}\+", "", fields[0]),
                "embedded": fields[-5] == "yes",
                "subset": fields[-4] == "yes",
                "unicode": fields[-3] == "yes",
            }
        )
    return records


@dataclass(frozen=True)
class PageSpec:
    count: int | None = None
    width: float = 960
    height: float = 540
    dpi: int = 100

    def __post_init__(self):
        if (
            not (
                1 <= self.width <= 5000
                and 1 <= self.height <= 5000
                and 36 <= self.dpi <= 300
            )
            or self.width * self.height * (self.dpi / 72) ** 2 > 32000000
        ):
            raise PdfError(
                "geometry", "Invalid dimensions/DPI or raster exceeds 32 million pixels"
            )
        if self.count is not None and (
            type(self.count) is not int or not 1 <= self.count <= 200
        ):
            raise PdfError("page_count", "Expected 1..200 target pages")


def _normalize(pdf, destination, spec):
    expected_pages, width, height = spec.count, spec.width, spec.height
    if Path(pdf).stat().st_size > 100 * 1024 * 1024:
        raise PdfError("pdf_limit", "PDF exceeds 100 MiB input limit")
    reader = PdfReader(pdf)
    if (
        not 1 <= len(reader.pages) <= 200
        or expected_pages is not None
        and len(reader.pages) != expected_pages
    ):
        raise PdfError(
            "page_count",
            f"Unexpected PDF page count: {len(reader.pages)}; expected {expected_pages or '1..200'}",
        )
    writer, sizes = PdfWriter(), []
    for page in reader.pages:
        page.transfer_rotation_to_content()
        w, h = float(page.mediabox.width), float(page.mediabox.height)
        if not math.isfinite(w + h) or h <= 0 or abs(w / h - width / height) > 0.001:
            raise PdfError(
                "aspect_ratio",
                f"PDF aspect ratio disagrees: {w}x{h} vs {width}x{height}",
            )
        sizes.append([w, h])
        page.scale_to(width, height)
        writer.add_page(page)
    with destination.open("wb") as stream:
        writer.write(stream)
    return sizes


def _rasters(folder, tool, run, spec, count):
    for prefix, extra in [("rgb", []), ("gray", ["-gray"])]:
        run(
            [
                tool,
                "-r",
                str(spec.dpi),
                "-png",
                *extra,
                folder / "normalized.pdf",
                folder / prefix,
            ]
        )
        files = sorted(
            folder.glob(prefix + "-*.png"), key=lambda p: int(p.stem.split("-")[-1])
        )
        if len(files) != count:
            raise PdfError("page_count", "Raster page count disagrees with PDF")
        # Canonical numbering avoids Poppler's page-number padding differences.
        for i, p in enumerate(files):
            destination = folder / f"{prefix}{i}.png"
            p.rename(destination)
            if destination.is_symlink() or not destination.is_file():
                raise PdfError("provider_output", "Non-regular raster output rejected")
            expected = (
                math.ceil(spec.width * spec.dpi / 72),
                math.ceil(spec.height * spec.dpi / 72),
            )
            with Image.open(destination) as image:
                if image.size != expected:
                    raise PdfError(
                        "geometry",
                        f"Raster dimensions disagree with normalized PDF/DPI: {image.size} vs {expected}",
                    )


def prepare_pdf(pdf, folder, tools, run, spec):
    import json

    normalized = folder / "normalized.pdf"
    sizes = _normalize(pdf, normalized, spec)
    font_text, _ = run([tools["pdffonts"]["path"], normalized])
    run([tools["pdftotext"]["path"], "-bbox-layout", normalized, folder / "text.xml"])
    text_file = folder / "text.xml"
    if (
        text_file.is_symlink()
        or not text_file.is_file()
        or text_file.stat().st_size > 16 * 1024 * 1024
    ):
        raise PdfError(
            "text_layer",
            "Text layer must be a regular file <= 16 MiB; compare a smaller deck",
        )
    lines = parse_lines(text_file.read_text())
    (folder / "lines.json").write_text(json.dumps(lines, ensure_ascii=False))
    _rasters(folder, tools["pdftoppm"]["path"], run, spec, len(sizes))
    return {
        "algorithm": ALGORITHM,
        "page_count": len(sizes),
        "source_sizes_pt": sizes,
        "normalized_size_pt": [spec.width, spec.height],
        "dpi": spec.dpi,
        "fonts": parse_fonts(font_text),
        "text_lines": len(lines),
    }


def ssim(a, b):
    if a.shape != b.shape or a.ndim != 2:
        raise ValueError("Raster dimensions disagree")
    h, w = (a.shape[0] // 8) * 8, (a.shape[1] // 8) * 8
    if not h or not w:
        raise ValueError("Raster smaller than SSIM window")

    def windows(v):
        return (
            v[:h, :w]
            .astype(np.float64)
            .reshape(h // 8, 8, w // 8, 8)
            .transpose(0, 2, 1, 3)
            .reshape(-1, 64)
        )

    x, y = windows(a), windows(b)
    mx, my = x.mean(1), y.mean(1)
    vx, vy, cov = (
        (x * x).mean(1) - mx * mx,
        (y * y).mean(1) - my * my,
        (x * y).mean(1) - mx * my,
    )
    c1, c2 = (0.01 * 255) ** 2, (0.03 * 255) ** 2
    return float(
        (
            ((2 * mx * my + c1) * (2 * cov + c2))
            / ((mx * mx + my * my + c1) * (vx + vy + c2))
        ).mean()
    )


def match_lines(a, b, max_cells=4000000):
    pre = 0
    while pre < min(len(a), len(b)) and a[pre] == b[pre]:
        pre += 1
    suffix = 0
    while suffix < min(len(a) - pre, len(b) - pre) and a[-1 - suffix] == b[-1 - suffix]:
        suffix += 1
    aa, bb = a[pre : len(a) - suffix], b[pre : len(b) - suffix]
    n, m = len(aa), len(bb)
    if (n + 1) * (m + 1) > max_cells:
        raise ValueError(
            "Exact line matching exceeds LCS work limit; compare a smaller deck"
        )
    table = np.zeros((n + 1, m + 1), dtype=np.uint32)
    for i in range(n - 1, -1, -1):
        for j in range(m - 1, -1, -1):
            table[i, j] = (
                table[i + 1, j + 1] + 1
                if aa[i] == bb[j]
                else max(table[i + 1, j], table[i, j + 1])
            )
    pairs = [(i, i) for i in range(pre)]
    i = j = 0
    while i < n and j < m:
        if aa[i] == bb[j]:
            pairs.append((pre + i, pre + j))
            i += 1
            j += 1
        elif table[i + 1, j] >= table[i, j + 1]:
            i += 1
        else:
            j += 1
    return pairs + [(len(a) - suffix + k, len(b) - suffix + k) for k in range(suffix)]


def rounded(value, places=4):
    k = 10**places
    return math.copysign(math.floor(abs(value) * k + 0.5) / k, value)


def compare_pages(reference, candidate, ref_lines, test_lines, out=None):
    if len(reference) != len(candidate) or not reference:
        raise ValueError("PDF page counts disagree or are empty")
    texts = lambda lines: [" ".join(line["text"].split()) for line in lines]
    a, b = texts(ref_lines), texts(test_lines)
    pairs = match_lines(a, b)
    matched = dict(pairs)
    rows = []
    raw_ssims = []
    for page, (r, t) in enumerate(zip(reference, candidate)):
        gray, other = r["gray"], t["gray"]
        value = ssim(gray, other)
        raw_ssims.append(value)
        ref_ix = [i for i, l in enumerate(ref_lines) if l["page"] == page]
        same = [(i, matched[i]) for i in ref_ix if i in matched]
        placed = [(i, j) for i, j in same if test_lines[j]["page"] == page]
        dy = sorted(abs(ref_lines[i]["y"] - test_lines[j]["y"]) for i, j in placed)
        first = matched.get(ref_ix[0]) if ref_ix else None
        rows.append(
            {
                "page": page + 1,
                "ssim": rounded(value),
                "diff_px": rounded(
                    float((np.abs(gray.astype(float) - other) > 64).mean() * 100), 2
                ),
                "color_error": rounded(
                    float(np.abs(r["rgb"].astype(float) - t["rgb"]).mean() / 255)
                ),
                "lines": [len(ref_ix), sum(l["page"] == page for l in test_lines)],
                "same_break": rounded(100 * len(same) / len(ref_ix), 1)
                if ref_ix
                else None,
                "dx": rounded(
                    max(
                        (
                            abs(ref_lines[i]["x"] - test_lines[j]["x"])
                            for i, j in placed
                        ),
                        default=0,
                    ),
                    1,
                ),
                "dy": rounded(dy[len(dy) // 2], 1) if dy else None,
                "page_start_same": first is not None
                and test_lines[first]["page"] == page
                and (first == 0 or test_lines[first - 1]["page"] != page),
            }
        )
        if out is not None:
            Image.fromarray(
                np.stack([other, np.minimum(gray, other), gray], axis=2)
            ).save(Path(out) / f"diff-{page + 1}.png")
    tokens = lambda lines: set(re.findall(r"\w+", " ".join(texts(lines)).lower()))
    want, have = tokens(ref_lines), tokens(test_lines)
    return {
        "algorithm": ALGORITHM,
        "summary": {
            "pages": [len(reference), len(candidate)],
            "lines": [len(a), len(b)],
            "ssim_mean": rounded(sum(raw_ssims) / len(rows)),
            "same_break_pct": rounded(100 * len(pairs) / len(a), 1) if a else None,
            "word_recall": rounded(len(want & have) / len(want)) if want else None,
            "missing_words": sorted(want - have)[:40],
            "page_starts_same": sum(r["page_start_same"] for r in rows),
        },
        "pages": rows,
    }


class RasterPages:
    """Load only the current page; a long deck must not allocate every raster at once."""

    def __init__(self, render):
        self.root = Path(render["directory"])
        self.count = render["page_count"]

    def __len__(self):
        return self.count

    def __getitem__(self, i):
        if not 0 <= i < self.count:
            raise IndexError(i)
        with (
            Image.open(self.root / f"gray{i}.png") as gray,
            Image.open(self.root / f"rgb{i}.png") as rgb,
        ):
            return {
                "gray": np.array(gray.convert("L")),
                "rgb": np.array(rgb.convert("RGB")),
            }


def compare_renders(first, second, out):
    import json

    lines = lambda render: json.loads(
        (Path(render["directory"]) / "lines.json").read_text()
    )
    return compare_pages(
        RasterPages(first), RasterPages(second), lines(first), lines(second), out
    )
