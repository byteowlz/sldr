"""Original, isolated library for browser/export regressions; no user documents."""

from pathlib import Path
import json
import os
import shutil
import subprocess

SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="240" height="120" viewBox="0 0 240 120"><rect width="240" height="120" fill="#00b060"/><circle cx="180" cy="60" r="40" fill="#174bff"/></svg>'
LONG = (
    "Complete and predictable rendering preserves every word in this deliberately oversized synthetic declaration. "
    * 5
)
MARKERS = """<!-- sldr:chrome none -->
<!-- sldr:zone name=content rep=bake x=0 y=0 w=100 h=100 -->
<style>.sldr-slide[data-layout="markers"]{padding:0}.qa{position:absolute;width:5%;height:5%;z-index:100}</style>
<div class="sldr-content">{{content}}</div>
<div class="qa" style="left:0;top:0;background:#ff0000"></div>
<div class="qa" style="right:0;top:0;background:#00ff00"></div>
<div class="qa" style="left:0;bottom:0;background:#0000ff"></div>
<div class="qa" style="right:0;bottom:0;background:#ffff00"></div>
"""


def prepare(root: Path, font_dir: Path):
    lib = root / "library"
    for sub in [
        "slides/media",
        "playlists",
        "layouts",
        "flavors/probe/assets",
        "scaffolds",
    ]:
        (lib / sub).mkdir(parents=True)
    for name in ["DejaVuSans.ttf", "DejaVuSans-Bold.ttf"]:
        shutil.copy2(font_dir / name, lib / "flavors/probe/assets" / name)
    (lib / "flavors/probe/assets/fonts.css").write_text(
        "@font-face{font-family:Probe;src:url('DejaVuSans.ttf')}\n"
        "@font-face{font-family:Probe;font-weight:700;src:url('DejaVuSans-Bold.ttf')}"
    )
    (lib / "slides/media/diagram.svg").write_text(SVG)
    (lib / "flavors/probe/assets/logo.svg").write_text(SVG)
    (lib / "layouts/markers.html").write_text(MARKERS)
    (lib / "flavors/probe/flavor.toml").write_text("""name="probe"
font_imports=["assets/fonts.css"]
[colors]
background="#ffffff"
text="#17232b"
accent="#1750aa"
[typography]
heading_font="Probe"
body_font="Probe"
code_font="Probe"
[decoration]
intensity=0.0
[[logos]]
file="logo.svg"
position="top-right"
width="8%"
layouts=["framed", "framed-cols"]
""")
    slides = {
        "markers": "---\nlayout: markers\n---\nMarker fixture",
        "statement": f"---\nlayout: statement\n---\nEyebrow sentinel\n\n# {LONG}",
        "frame": "---\ntitle: Frame title\nlayout: framed\n---\nFrame body sentinel",
        "cols": "---\ntitle: Column title\nlayout: framed-cols\n---\n::left::\nLeft sentinel\n::right::\nRight sentinel",
        "lang": "---\ntitle: Language fixture\nlayout: framed-freeform\n---\n::lang:en::\n::block x=6 y=24 w=42 h=60::\nEnglish sentinel\n::lang:de::\n::block x=6 y=24 w=42 h=60::\nDeutsche Sprachprüfung",
        "svg": "---\ntitle: SVG fixture\nlayout: framed-freeform\n---\n::block x=6 y=24 w=42 h=60::\n![](media/diagram.svg)",
    }
    for name, text in slides.items():
        (lib / f"slides/{name}.md").write_text(text + "\n")
    for name, entries in [("all", list(slides)), *[(n, [n]) for n in slides]]:
        (lib / f"playlists/{name}.toml").write_text(
            f'name="{name}"\nflavor="probe"\nslides={json.dumps(entries)}\n'
            '[render]\ntransition="none"\naspect_ratio="16/9"\n'
        )
    cfg = root / "config/sldr"
    cfg.mkdir(parents=True)
    (cfg / "config.toml").write_text(f"""[config]
library={json.dumps(str(lib))}
flavor_dir={json.dumps(str(lib / "flavors"))}
layout_dir={json.dumps(str(lib / "layouts"))}
scaffold_dir={json.dumps(str(lib / "scaffolds"))}
default_flavor="probe"
[presentations]
slide_dir={json.dumps(str(lib / "slides"))}
playlist_dir={json.dumps(str(lib / "playlists"))}
output_dir={json.dumps(str(root / "out"))}
""")


class Fixture:
    def __init__(self, root: Path, binary: Path, browser: Path):
        self.root, self.binary, self.browser = root, binary, browser
        self.env = {
            **os.environ,
            **{
                f"XDG_{key}_HOME": str(root / value)
                for key, value in [
                    ("CONFIG", "config"),
                    ("DATA", "data"),
                    ("STATE", "state"),
                    ("CACHE", "cache"),
                ]
            },
            "CHROME_BIN": str(browser),
        }

    def run(self, name, *args, expected=0):
        result = subprocess.run(
            [str(self.binary), *map(str, args)],
            env=self.env,
            capture_output=True,
            text=True,
            timeout=120,
        )
        (self.root / f"{name}.log").write_text(result.stdout + result.stderr)
        assert result.returncode == expected, (name, result.returncode, result.stderr)
        return result
