# /// script
# requires-python = ">=3.11"
# dependencies = ["pillow==12.3.0", "fonttools==4.67.0"]
# ///
"""Generate original synthetic sldr input; no private library or vendor assets."""
from pathlib import Path
import hashlib
import json
import shutil
import argparse
from PIL import Image, ImageDraw
from fontTools.ttLib import TTFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('output', type=Path, help='Dedicated local evaluation directory')
parser.add_argument('--font-dir', type=Path, required=True, help='Directory containing the four open DejaVu Sans faces')
args = parser.parse_args()
root = args.output.expanduser().resolve()
lib = root / 'library'
for sub in ['slides/media', 'playlists', 'flavors/probe', 'layouts', 'scaffolds']:
    (lib / sub).mkdir(parents=True, exist_ok=True)
font_dir = args.font_dir.expanduser().resolve()
font_path = font_dir / 'DejaVuSans.ttf'
font = TTFont(font_path)
family = 'DejaVu Sans'
font_assets = lib / 'flavors/probe/assets'
font_assets.mkdir(exist_ok=True)
private_fonts = root / 'deckcraft-home/Library/Fonts'
private_fonts.mkdir(parents=True, exist_ok=True)
faces = [('DejaVuSans.ttf', 400, 'normal'), ('DejaVuSans-Bold.ttf', 700, 'normal'), ('DejaVuSans-Oblique.ttf', 400, 'italic'), ('DejaVuSans-BoldOblique.ttf', 700, 'italic')]
css = []
font_records = []
for filename, weight, style in faces:
    src = font_dir / filename
    shutil.copy2(src, font_assets / filename)
    shutil.copy2(src, private_fonts / filename)
    linux_fonts = root / 'deckcraft-home/.local/share/fonts'
    linux_fonts.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, linux_fonts / filename)
    css.append(f"@font-face {{ font-family: '{family}'; font-weight: {weight}; font-style: {style}; src: url('{filename}'); }}")
    font_records.append({'file': filename, 'sha256': hashlib.sha256(src.read_bytes()).hexdigest()})
(font_assets / 'fonts.css').write_text('\n'.join(css))
(root / 'font.json').write_text(json.dumps({'requested_family': family, 'faces': font_records, 'font_version': font['name'].getDebugName(5), 'note': 'Open DejaVu faces from LibreOffice: embedded in Chrome HTML, isolated HOME/Library/Fonts for DeckCraft, bundled faces in LibreOffice. No fonts installed globally or committed.'}, indent=2))
(lib / 'flavors/probe/flavor.toml').write_text(f'''name = "probe"
font_imports = ["assets/fonts.css"]
[colors]
primary = "#174b70"
secondary = "#3b5260"
background = "#ffffff"
text = "#17232b"
text_dim = "#425563"
accent = "#007f70"
[typography]
heading_font = "'{family}'"
body_font = "'{family}'"
code_font = "'{family}'"
heading_weight = "700"
body_weight = "400"
[decoration]
intensity = 0.0
''')
slides = {
'01-bullets': '''---
title: Synthetic parity probe
subtitle: Bullets, emphasis and German text
layout: framed
footer: Synthetic fixture - no external data
source: Original test content
---
- First native bullet stays editable.
- **Bold text** and italic *text* share a paragraph.
- Grüne Bäume, Größe und zuverlässige Prüfung.
- This deliberately longer line tests wrapping at the same text frame width without truncation or automatic rewriting.

''',
'02-columns': '''---
title: Two independent columns
subtitle: Geometry and line breaks
layout: framed-cols
footer: Synthetic fixture - columns
---
::left::
## Input

A short statement with stable wording.

- Left item alpha
- Left item beta

::right::
## Output

This longer paragraph deliberately uses several words so that independent layout engines must break lines within the right-hand frame.

- Right item gamma
- Right item delta
''',
'03-picture': '''---
title: Original diagram and text
subtitle: Native picture plus editable text
layout: framed-image
footer: Synthetic fixture - original raster image
source: Generated geometric test image
---
::content::
- Picture geometry should stay fixed.
- Text remains native and editable.
- Blue, green and orange circles.

::image::
![Three original colored circles](media/circles.png)
'''}
for name, text in slides.items():
    (lib / f'slides/{name}.md').write_text(text)
(lib / 'playlists/probe.toml').write_text('name = "probe"\nflavor = "probe"\nslides = ["01-bullets", "02-columns", "03-picture"]\n[render]\ntransition = "none"\naspect_ratio = "16/9"\naspect_lock = true\n')
im = Image.new('RGB', (640, 360), 'white')
d = ImageDraw.Draw(im)
for x, color in [(110, '#174b70'), (320, '#007f70'), (530, '#d67b1c')]:
    d.ellipse((x-75, 105, x+75, 255), fill=color)
im.save(lib / 'slides/media/circles.png')
cfg = root / 'config/sldr'
cfg.mkdir(parents=True, exist_ok=True)
(cfg / 'config.toml').write_text(f'''[config]
library = "{lib}"
flavor_dir = "{lib / 'flavors'}"
layout_dir = "{lib / 'layouts'}"
scaffold_dir = "{lib / 'scaffolds'}"
default_flavor = "probe"
[presentations]
slide_dir = "{lib / 'slides'}"
playlist_dir = "{lib / 'playlists'}"
output_dir = "{root / 'out'}"
''')
print(f'Fixture: {lib}; requested font family: {family}')
