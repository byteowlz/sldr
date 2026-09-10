---
title: Custom HTML
subtitle: when no layout fits, build the figure in an ```html fence
layout: framed
type_scale: 0.85
footer: "© sldr"
source: "Escape hatch | Reference"
tags: [reference, guide]
---
```html
<style>
.demo { display: grid; grid-template-columns: 1fr auto 1fr; grid-auto-flow: column; align-items: center; gap: calc(var(--sldr-u) * 2); font-size: calc(var(--sldr-u) * 1.6); }
.demo .box { padding: calc(var(--sldr-uh) * 3) calc(var(--sldr-u) * 2); text-align: center; font-weight: 700; }
.demo .stack { display: flex; flex-direction: column; align-items: center; gap: calc(var(--sldr-uh) * 1.5); }
.demo .stack p { margin: 0; }
</style>
<div class="demo">
  <div class="stack">
    <div class="box sldr-shape">Input</div>
    <span class="sldr-arrow is-down"></span>
    <div class="box sldr-shape is-hi">Highlighted</div>
    <span class="sldr-caption">shape, arrow, highlight</span>
  </div>
  <div class="sldr-block-arrow">transform</div>
  <div class="stack">
    <div class="box sldr-shape">Output</div>
    <p>Boxes, arrows and captions come from the shared helper classes, so one html slide looks native in every flavor.</p>
    <span class="sldr-caption">block arrow, caption</span>
  </div>
</div>
```

- Size type once on the wrapper: `p` and `li` inside a framed body inherit it, and a `.panel h3` rule wins over the framed defaults.
- Explicit grids need `grid-auto-flow: column` (or `grid-template-areas`) when panels hold several children, or they auto-place by row.
- Give every SVG `<marker id>` a slide-unique name; all slides share one document.
- `type_scale: 0.85` in the frontmatter (used here) shrinks all type on one slide without touching the flavor.
