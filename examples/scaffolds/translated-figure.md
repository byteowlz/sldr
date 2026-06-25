---
title: "{{title}}"
subtitle: "One-line context"
source: "Source name"
source_url: "https://example.com/article"
layout: framed-image
tags: [image, visual, multilingual]
# Chrome (headline/subtitle/source) per language. The body below carries the
# per-language text; this block translates the framed chrome. Add more
# languages as needed; the deck default needs no block here.
translations:
  de:
    title: "{{title}}"
    subtitle: "Kontext in einem Satz"
    source: "Quellenname"
---
::image::

![Describe the image](media/{{name}}.png)

::lang:en::
::content::

English explanation of the figure. The image above is **shared** across all
languages — declare it once, above the `::lang::` blocks.

- One point
- Another point

::lang:de::
::content::

Deutsche Erklärung der Abbildung. Das Bild oben wird für **alle Sprachen**
gemeinsam genutzt — einmal oberhalb der `::lang::`-Blöcke deklariert.

- Ein Punkt
- Noch ein Punkt
