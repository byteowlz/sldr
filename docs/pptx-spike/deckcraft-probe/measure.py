# /// script
# requires-python = ">=3.11"
# dependencies = ["pypdf==6.20.1", "pillow==12.3.0"]
# ///
"""Local probe: normalize slide geometry, then reuse WordCraft's exact comparator."""
from pathlib import Path
import itertools
import json
import subprocess
import argparse
from pypdf import PdfReader, PdfWriter
from PIL import Image, ImageDraw

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path, help='Evaluation directory containing chrome.pdf, deckcraft.pdf, lo/probe.pdf')
parser.add_argument('--wordcraft-xtask', type=Path, required=True, help='Pinned WordCraft xtask executable with visual-diff')
args = parser.parse_args()
root = args.output.expanduser().resolve()
sources = {'chrome': root/'chrome.pdf', 'deckcraft': root/'deckcraft.pdf', 'libreoffice': root/'lo/probe.pdf'}
manifest = {}
for engine, source in sources.items():
    reader = PdfReader(source)
    assert len(reader.pages) == 3, f'{engine}: unexpected page count'
    original = []
    writer = PdfWriter()
    for page in reader.pages:
        w, h = float(page.mediabox.width), float(page.mediabox.height)
        assert abs(w/h - 16/9) < 0.001, f'{engine}: unexpected aspect ratio'
        original.append([w, h])
        page.scale_to(960, 540)
        writer.add_page(page)
    dst = root/f'{engine}-normalized.pdf'
    with dst.open('wb') as stream:
        writer.write(stream)
    manifest[engine] = {'source_page_sizes_pt': original, 'normalized_size_pt': [960, 540]}
(root/'normalization.json').write_text(json.dumps(manifest, indent=2))
metric = args.wordcraft_xtask.expanduser().resolve()
summary = {}
for reference, candidate in [('chrome','chrome'), *itertools.combinations(sources, 2)]:
    name = f'{reference}-vs-{candidate}'
    out = root/'scores'/name
    r = subprocess.run([str(metric), 'visual-diff', str(root/f'{reference}-normalized.pdf'), str(root/f'{candidate}-normalized.pdf'), str(out), '--dpi', '100'], check=True, capture_output=True, text=True, timeout=120)
    data = json.loads((out/'result.json').read_text())
    summary[name] = data['summary']
    if reference == candidate:
        assert data['summary']['ssim_mean'] == 1
        assert data['summary']['same_break_pct'] == 100
        assert all(p.get('dx', 0) == 0 and p.get('dy', 0) == 0 for p in data['pages'])
(root/'summary.json').write_text(json.dumps(summary, indent=2))
print(json.dumps(summary, indent=2))
# Standardized 960x540 images for inspectable evidence (no comparison score here).
canvas = Image.new('RGB', (960*3, 580*3), '#eeeeee')
draw = ImageDraw.Draw(canvas)
for col, engine in enumerate(sources):
    prefix = root/f'{engine}-view'
    subprocess.run(['pdftoppm','-scale-to-x','960','-scale-to-y','540','-png',str(root/f'{engine}-normalized.pdf'), str(prefix)], check=True, capture_output=True, timeout=120)
    for row in range(3):
        draw.text((col*960+10,row*580+8), f'{engine} - slide {row+1}', fill='black')
        with Image.open(root/f'{engine}-view-{row+1}.png') as im:
            canvas.paste(im, (col*960, row*580+30))
canvas.save(root/'comparison.png')
